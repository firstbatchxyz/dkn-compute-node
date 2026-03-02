use std::ops::ControlFlow;
use std::time::Instant;

use crate::error::NodeError;
use crate::inference::engine::{GenerateParams, InferenceEngine};

/// Result of a TPS benchmark run.
#[derive(Debug, Clone)]
pub struct TpsResult {
    pub model_name: String,
    pub prompt_eval_tps: f64,
    pub generation_tps: f64,
    pub total_time_ms: u64,
    pub tokens_generated: u32,
}

impl std::fmt::Display for TpsResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {:.1} tok/s generation, {:.1} tok/s prompt eval ({} tokens in {}ms)",
            self.model_name,
            self.generation_tps,
            self.prompt_eval_tps,
            self.tokens_generated,
            self.total_time_ms,
        )
    }
}

const WARMUP_PROMPT: &str = "Write a short poem about hedgehogs and squirrels.";
const BENCHMARK_PROMPT: &str = "Please write a poem about Kapadokya.";
const BENCHMARK_MAX_TOKENS: u32 = 128;

impl InferenceEngine {
    /// Run a TPS benchmark: warmup generation, then timed generation.
    pub fn benchmark(&self, model_name: &str) -> Result<TpsResult, NodeError> {
        // Warmup: short generation to prime caches
        let warmup_params = GenerateParams {
            max_tokens: 16,
            temperature: 0.0,
            ..Default::default()
        };
        let _ = self.generate(WARMUP_PROMPT, &warmup_params, |_| ControlFlow::Continue(()));

        // Timed benchmark
        let bench_params = GenerateParams {
            max_tokens: BENCHMARK_MAX_TOKENS,
            temperature: 0.0,
            ..Default::default()
        };

        let start = Instant::now();
        let result = self.generate(BENCHMARK_PROMPT, &bench_params, |_| ControlFlow::Continue(()))?;
        let total_time_ms = start.elapsed().as_millis() as u64;

        let prompt_eval_tps = if result.prompt_eval_time_ms > 0 {
            (result.prompt_tokens as f64) / (result.prompt_eval_time_ms as f64 / 1000.0)
        } else {
            0.0
        };

        Ok(TpsResult {
            model_name: model_name.to_string(),
            prompt_eval_tps,
            generation_tps: result.tokens_per_second,
            total_time_ms,
            tokens_generated: result.tokens_generated,
        })
    }
}
