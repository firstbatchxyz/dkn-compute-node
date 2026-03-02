use std::ops::ControlFlow;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::NodeError;
use crate::inference::{GenerateParams, InferenceEngine, InferenceResult};
use crate::models::template::{ChatMessage, apply_chat_template};
use crate::network::protocol::{
    Capacity, NodeMessage, RejectReason, TaskStats, ValidationRequest,
};

/// A completed inference task ready to be sent back.
pub struct CompletedTask {
    pub task_id: Uuid,
    pub result: Result<NodeMessage, NodeError>,
}

/// Executes inference tasks with backpressure via capacity tracking.
pub struct Worker {
    engine: Arc<InferenceEngine>,
    /// Chat template name for prompt formatting.
    chat_template: String,
    /// Number of available inference slots (CAS-based).
    capacity: Arc<AtomicUsize>,
    /// Maximum concurrent slots.
    max_capacity: usize,
    /// Models this worker serves.
    model_names: Vec<String>,
    /// In-flight tasks tracked via FuturesUnordered.
    in_flight: FuturesUnordered<JoinHandle<CompletedTask>>,
}

impl Worker {
    /// Create a new worker wrapping an inference engine.
    pub fn new(
        engine: InferenceEngine,
        chat_template: String,
        model_names: Vec<String>,
        max_concurrent: usize,
    ) -> Self {
        Worker {
            engine: Arc::new(engine),
            chat_template,
            capacity: Arc::new(AtomicUsize::new(max_concurrent)),
            max_capacity: max_concurrent,
            model_names,
            in_flight: FuturesUnordered::new(),
        }
    }

    /// Try to accept a task. Returns `Err(RejectReason)` if the task cannot be accepted.
    ///
    /// On success, spawns inference in a blocking thread and returns immediately.
    pub fn try_accept(
        &self,
        task_id: Uuid,
        model: &str,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        temperature: f32,
        validation: Option<ValidationRequest>,
    ) -> Result<(), RejectReason> {
        // Check model
        if !self.model_names.iter().any(|m| m == model) {
            return Err(RejectReason::ModelNotLoaded);
        }

        // Try to decrement capacity (CAS loop)
        loop {
            let current = self.capacity.load(Ordering::Acquire);
            if current == 0 {
                return Err(RejectReason::AtCapacity);
            }
            if self
                .capacity
                .compare_exchange_weak(current, current - 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }

        // Build generate params
        let params = GenerateParams {
            max_tokens,
            temperature,
            top_p: 0.9,
            seed: None,
            logprob_positions: validation
                .as_ref()
                .map(|v| v.logprob_positions.clone())
                .unwrap_or_default(),
            logprob_top_k: validation.as_ref().map(|v| v.logprob_top_k).unwrap_or(5),
        };

        let engine = Arc::clone(&self.engine);
        let capacity = Arc::clone(&self.capacity);
        let template = self.chat_template.clone();

        let handle = tokio::task::spawn_blocking(move || {
            let result = run_inference(&engine, &template, messages, &params, task_id);
            // Release capacity slot regardless of outcome
            capacity.fetch_add(1, Ordering::Release);
            result
        });

        self.in_flight.push(handle);
        Ok(())
    }

    /// Poll for the next completed task.
    ///
    /// Returns `None` when no tasks are in-flight. When used in `tokio::select!`,
    /// the branch will be skipped when there's nothing to poll.
    pub async fn next_completed(&mut self) -> Option<CompletedTask> {
        let join_result = self.in_flight.next().await?;
        match join_result {
            Ok(completed) => Some(completed),
            Err(e) => {
                tracing::error!(%e, "task panicked");
                None
            }
        }
    }

    /// Current capacity snapshot.
    pub fn capacity(&self) -> Capacity {
        Capacity {
            free: self.capacity.load(Ordering::Acquire),
            max: self.max_capacity,
        }
    }

    /// Model names this worker serves.
    pub fn model_names(&self) -> &[String] {
        &self.model_names
    }

    /// Whether there are any in-flight tasks.
    pub fn has_in_flight(&self) -> bool {
        !self.in_flight.is_empty()
    }
}

/// Run inference synchronously (called from `spawn_blocking`).
fn run_inference(
    engine: &InferenceEngine,
    template: &str,
    messages: Vec<ChatMessage>,
    params: &GenerateParams,
    task_id: Uuid,
) -> CompletedTask {
    let prompt = apply_chat_template(template, &messages);

    match engine.generate(&prompt, params, |_| ControlFlow::Continue(())) {
        Ok(result) => CompletedTask {
            task_id,
            result: Ok(build_task_result(task_id, result)),
        },
        Err(e) => CompletedTask {
            task_id,
            result: Err(e),
        },
    }
}

fn build_task_result(task_id: Uuid, result: InferenceResult) -> NodeMessage {
    NodeMessage::TaskResult {
        task_id,
        text: result.text,
        stats: TaskStats {
            tokens_generated: result.tokens_generated,
            prompt_tokens: result.prompt_tokens,
            generation_time_ms: result.generation_time_ms,
            tokens_per_second: result.tokens_per_second,
        },
        proof: result.proof,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a worker with no real engine (tests that don't need inference)
    // We can't easily mock InferenceEngine, so we test capacity logic directly.

    #[test]
    fn test_capacity_tracking() {
        let cap = Arc::new(AtomicUsize::new(3));

        // Decrement
        assert_eq!(cap.fetch_sub(1, Ordering::AcqRel), 3);
        assert_eq!(cap.load(Ordering::Acquire), 2);

        // Increment back
        cap.fetch_add(1, Ordering::Release);
        assert_eq!(cap.load(Ordering::Acquire), 3);
    }

    #[test]
    fn test_capacity_struct() {
        let c = Capacity { free: 2, max: 4 };
        assert_eq!(c.free, 2);
        assert_eq!(c.max, 4);
    }

    #[test]
    fn test_reject_reason_model_not_loaded() {
        let reason = RejectReason::ModelNotLoaded;
        let packed = rmp_serde::to_vec(&reason).unwrap();
        let roundtrip: RejectReason = rmp_serde::from_slice(&packed).unwrap();
        assert!(matches!(roundtrip, RejectReason::ModelNotLoaded));
    }

    #[test]
    fn test_reject_reason_at_capacity() {
        let reason = RejectReason::AtCapacity;
        let packed = rmp_serde::to_vec(&reason).unwrap();
        let roundtrip: RejectReason = rmp_serde::from_slice(&packed).unwrap();
        assert!(matches!(roundtrip, RejectReason::AtCapacity));
    }

    #[test]
    fn test_completed_task_success() {
        let msg = NodeMessage::TaskResult {
            task_id: Uuid::nil(),
            text: "Hello".into(),
            stats: TaskStats {
                tokens_generated: 5,
                prompt_tokens: 3,
                generation_time_ms: 50,
                tokens_per_second: 100.0,
            },
            proof: None,
        };
        let completed = CompletedTask {
            task_id: Uuid::nil(),
            result: Ok(msg),
        };
        assert!(completed.result.is_ok());
    }

    #[test]
    fn test_completed_task_error() {
        let completed = CompletedTask {
            task_id: Uuid::nil(),
            result: Err(NodeError::Inference("test error".into())),
        };
        assert!(completed.result.is_err());
    }
}
