use std::collections::HashMap;
use std::ops::ControlFlow;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use futures::stream::FuturesUnordered;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::error::NodeError;
use crate::inference::{GenerateParams, InferenceEngine, InferenceResult};
use crate::models::template::{ChatMessage, apply_chat_template};
use crate::network::protocol::{
    Capacity, ModelType, NodeMessage, RejectReason, TaskStats, ValidationRequest,
};

/// A completed inference task ready to be sent back.
pub struct CompletedTask {
    pub task_id: Uuid,
    pub result: Result<NodeMessage, NodeError>,
    /// Whether this task was streamed (tokens already forwarded inline).
    pub stream: bool,
}

/// Executes inference tasks with backpressure via capacity tracking.
///
/// Supports multiple models, each with its own engine, chat template, and modality.
pub struct Worker {
    /// Map of model name → (engine, chat_template, model_type).
    engines: HashMap<String, (Arc<InferenceEngine>, String, ModelType)>,
    /// Number of available inference slots (CAS-based).
    capacity: Arc<AtomicUsize>,
    /// Maximum concurrent slots.
    max_capacity: usize,
    /// In-flight tasks tracked via FuturesUnordered.
    in_flight: FuturesUnordered<JoinHandle<CompletedTask>>,
}

impl Worker {
    /// Create a new worker wrapping multiple inference engines.
    pub fn new(
        engines: HashMap<String, (InferenceEngine, String, ModelType)>,
        max_concurrent: usize,
    ) -> Self {
        let engines = engines
            .into_iter()
            .map(|(name, (engine, template, model_type))| {
                (name, (Arc::new(engine), template, model_type))
            })
            .collect();
        Worker {
            engines,
            capacity: Arc::new(AtomicUsize::new(max_concurrent)),
            max_capacity: max_concurrent,
            in_flight: FuturesUnordered::new(),
        }
    }

    /// Try to accept a task. Returns `Err(RejectReason)` if the task cannot be accepted.
    ///
    /// On success, spawns inference in a blocking thread and returns immediately.
    /// When `stream` is true and `stream_tx` is provided, tokens are forwarded
    /// inline via the connection's outgoing channel.
    #[allow(clippy::too_many_arguments)]
    pub fn try_accept(
        &self,
        task_id: Uuid,
        model: &str,
        messages: Vec<ChatMessage>,
        max_tokens: u32,
        temperature: f32,
        validation: Option<ValidationRequest>,
        stream: bool,
        stream_tx: Option<mpsc::UnboundedSender<NodeMessage>>,
    ) -> Result<(), RejectReason> {
        // Look up engine + template + model_type for the requested model (fail fast before decrementing capacity)
        let (engine, template, model_type) = self
            .engines
            .get(model)
            .ok_or(RejectReason::ModelNotLoaded)?;

        // Check modality: reject if messages contain image/audio parts that the model can't handle
        let has_image = messages
            .iter()
            .any(|m| m.content.has_image());
        let has_audio = messages
            .iter()
            .any(|m| m.content.has_audio());

        if has_image && *model_type != ModelType::Vision {
            return Err(RejectReason::InvalidRequest(
                "message contains image content but model does not support vision".into(),
            ));
        }
        if has_audio && *model_type != ModelType::Audio {
            return Err(RejectReason::InvalidRequest(
                "message contains audio content but model does not support audio".into(),
            ));
        }

        let engine = Arc::clone(engine);
        let template = template.clone();

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

        let capacity = Arc::clone(&self.capacity);

        if stream {
            if let Some(conn_tx) = stream_tx {
                // Bridge from blocking thread to async: use std sync_channel
                let (sync_tx, sync_rx) = std::sync::mpsc::sync_channel::<NodeMessage>(32);

                // Async forwarder: reads from sync_rx, sends to connection channel
                tokio::spawn(async move {
                    // sync_rx.recv() blocks, so we wrap in spawn_blocking to keep async runtime happy
                    loop {
                        let rx = sync_rx.try_recv();
                        match rx {
                            Ok(msg) => {
                                if conn_tx.send(msg).is_err() {
                                    break; // connection gone
                                }
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                        }
                    }
                });

                let handle = tokio::task::spawn_blocking(move || {
                    let result =
                        run_inference_streaming(&engine, &template, messages, &params, task_id, sync_tx);
                    capacity.fetch_add(1, Ordering::Release);
                    result
                });
                self.in_flight.push(handle);
            }
        } else {
            let handle = tokio::task::spawn_blocking(move || {
                let result = run_inference(&engine, &template, messages, &params, task_id);
                capacity.fetch_add(1, Ordering::Release);
                result
            });
            self.in_flight.push(handle);
        }

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
    pub fn model_names(&self) -> Vec<String> {
        self.engines.keys().cloned().collect()
    }

    /// Whether there are any in-flight tasks.
    pub fn has_in_flight(&self) -> bool {
        !self.in_flight.is_empty()
    }

    /// Add a new model engine at runtime (for hot-swap).
    ///
    /// If a model with this name already exists, it is replaced.
    pub fn add_engine(
        &mut self,
        name: String,
        engine: InferenceEngine,
        template: String,
        model_type: ModelType,
    ) {
        self.engines
            .insert(name, (Arc::new(engine), template, model_type));
    }

    /// Remove a model engine by name. Returns true if the model was present.
    ///
    /// Safe while tasks are in-flight — running tasks hold their own Arc<InferenceEngine> clone.
    pub fn remove_engine(&mut self, name: &str) -> bool {
        self.engines.remove(name).is_some()
    }

    /// Check whether the worker has a model loaded.
    pub fn has_model(&self, name: &str) -> bool {
        self.engines.contains_key(name)
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
            stream: false,
        },
        Err(e) => CompletedTask {
            task_id,
            result: Err(e),
            stream: false,
        },
    }
}

/// Run streaming inference: sends tokens via `token_tx` as they're generated.
fn run_inference_streaming(
    engine: &InferenceEngine,
    template: &str,
    messages: Vec<ChatMessage>,
    params: &GenerateParams,
    task_id: Uuid,
    token_tx: std::sync::mpsc::SyncSender<NodeMessage>,
) -> CompletedTask {
    let prompt = apply_chat_template(template, &messages);

    let tx = token_tx.clone();
    let result = engine.generate(&prompt, params, move |stream_token| {
        let msg = NodeMessage::StreamToken {
            task_id,
            token: stream_token.text,
            index: stream_token.index as u32,
        };
        if tx.send(msg).is_err() {
            // Receiver dropped (connection lost), stop generation
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    });

    match result {
        Ok(result) => {
            // Send StreamEnd
            let end_msg = NodeMessage::StreamEnd {
                task_id,
                text: result.text.clone(),
                stats: TaskStats {
                    tokens_generated: result.tokens_generated,
                    prompt_tokens: result.prompt_tokens,
                    generation_time_ms: result.generation_time_ms,
                    tokens_per_second: result.tokens_per_second,
                },
                proof: result.proof.clone(),
            };
            let _ = token_tx.send(end_msg);
            CompletedTask {
                task_id,
                result: Ok(build_task_result(task_id, result)),
                stream: true,
            }
        }
        Err(e) => {
            // Send StreamError
            let err_msg = NodeMessage::StreamError {
                task_id,
                error: e.to_string(),
            };
            let _ = token_tx.send(err_msg);
            CompletedTask {
                task_id,
                result: Err(e),
                stream: true,
            }
        }
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
            stream: false,
        };
        assert!(completed.result.is_ok());
        assert!(!completed.stream);
    }

    #[test]
    fn test_completed_task_error() {
        let completed = CompletedTask {
            task_id: Uuid::nil(),
            result: Err(NodeError::Inference("test error".into())),
            stream: false,
        };
        assert!(completed.result.is_err());
    }

    #[test]
    fn test_completed_task_streaming() {
        let completed = CompletedTask {
            task_id: Uuid::nil(),
            result: Ok(NodeMessage::TaskResult {
                task_id: Uuid::nil(),
                text: "streamed".into(),
                stats: TaskStats {
                    tokens_generated: 3,
                    prompt_tokens: 2,
                    generation_time_ms: 30,
                    tokens_per_second: 100.0,
                },
                proof: None,
            }),
            stream: true,
        };
        assert!(completed.stream);
        assert!(completed.result.is_ok());
    }

    #[test]
    fn test_worker_has_model() {
        let worker = Worker::new(HashMap::new(), 1);
        assert!(!worker.has_model("lfm2.5:1.2b"));
    }

    #[test]
    fn test_worker_remove_engine_not_present() {
        let mut worker = Worker::new(HashMap::new(), 1);
        assert!(!worker.remove_engine("lfm2.5:1.2b"));
    }

    #[test]
    fn test_worker_model_names_empty() {
        let worker = Worker::new(HashMap::new(), 1);
        assert!(worker.model_names().is_empty());
    }

    #[test]
    fn test_modality_check_text_content() {
        use crate::models::template::MessageContent;
        // MessageContent::Text should have no image/audio
        let content = MessageContent::Text("hello".into());
        assert!(!content.has_image());
        assert!(!content.has_audio());
    }
}
