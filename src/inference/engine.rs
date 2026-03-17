use std::ops::ControlFlow;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use llama_cpp_2::context::params::{KvCacheType, LlamaContextParams};
use llama_cpp_2::llama_backend::LlamaBackend;

/// Global singleton — llama.cpp backend can only be initialized once per process.
static LLAMA_BACKEND: OnceLock<LlamaBackend> = OnceLock::new();

fn get_backend() -> Result<&'static LlamaBackend, NodeError> {
    // OnceLock guarantees the closure runs exactly once, so BackendAlreadyInitialized
    // cannot happen here. If init() somehow fails, it's a fatal environment issue.
    Ok(LLAMA_BACKEND.get_or_init(|| {
        LlamaBackend::init().expect("failed to init llama backend")
    }))
}
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaModel};
use llama_cpp_2::mtmd::{MtmdBitmap, MtmdContext, MtmdContextParams, MtmdInputText};
use llama_cpp_2::sampling::LlamaSampler;
use llama_cpp_2::token::LlamaToken;

use dkn_protocol::ChatMessage;

use crate::error::NodeError;
use crate::identity::sha256hash;
use dkn_protocol::{InferenceProof, TokenLogprob};
use crate::inference::stream::StreamToken;

/// Parameters controlling text generation.
#[derive(Debug, Clone)]
pub struct GenerateParams {
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub seed: Option<u32>,
    /// Extract logprobs every N tokens (0 = disabled).
    /// E.g. 32 → positions [0, 32, 64, ...].
    pub logprob_every_n: usize,
    /// Top-k alternatives to collect at each logprob position.
    pub logprob_top_k: usize,
    /// Optional GBNF grammar string for constrained output.
    pub grammar: Option<String>,
}

impl Default for GenerateParams {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            temperature: 0.7,
            top_p: 0.9,
            seed: None,
            logprob_every_n: 0,
            logprob_top_k: 5,
            grammar: None,
        }
    }
}

/// Result of an inference run.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    pub text: String,
    pub tokens_generated: u32,
    pub prompt_tokens: u32,
    pub generation_time_ms: u64,
    pub prompt_eval_time_ms: u64,
    pub tokens_per_second: f64,
    pub proof: Option<InferenceProof>,
}

/// Wraps llama-cpp-2 for model loading and inference.
///
/// NOTE: `LlamaContext` is not Send/Sync. All inference must happen
/// via `tokio::task::spawn_blocking` with the engine moved into the closure.
pub struct InferenceEngine {
    backend: &'static LlamaBackend,
    model: LlamaModel,
    mtmd_ctx: Option<MtmdContext>,
    #[allow(dead_code)]
    gpu_layers: i32,
    /// Effective context window size (tokens), auto-detected from model metadata.
    ctx_limit: u32,
    /// KV cache quantization type (default Q8_0 to save memory).
    kv_cache_type: KvCacheType,
}

/// Helper to convert a token to a string piece using the new token_to_piece API.
fn token_to_string(model: &LlamaModel, token: LlamaToken) -> String {
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    model
        .token_to_piece(token, &mut decoder, true, None)
        .unwrap_or_default()
}

impl InferenceEngine {
    /// Load a GGUF model from disk, optionally with a multimodal projector.
    ///
    /// `max_context` optionally caps the context window (e.g. for limited VRAM).
    /// When `None`, the model's full native context window is used.
    pub fn load(
        path: &Path,
        gpu_layers: i32,
        mmproj_path: Option<&Path>,
        max_context: Option<u32>,
        kv_cache_type: Option<KvCacheType>,
    ) -> Result<Self, NodeError> {
        let kv_cache_type = kv_cache_type.unwrap_or(KvCacheType::Q8_0);
        let backend = get_backend()?;

        let model_params = if gpu_layers != 0 {
            let layers = if gpu_layers < 0 { 1000 } else { gpu_layers as u32 };
            LlamaModelParams::default().with_n_gpu_layers(layers)
        } else {
            LlamaModelParams::default()
        };

        let model = LlamaModel::load_from_file(backend, path, &model_params)
            .map_err(|e| NodeError::Inference(format!("failed to load model: {e}")))?;

        let n_ctx_train = model.n_ctx_train();
        let ctx_limit = match max_context {
            Some(cap) => n_ctx_train.min(cap),
            None => n_ctx_train,
        };
        tracing::info!(model_ctx = n_ctx_train, effective_ctx = ctx_limit, kv_type = ?kv_cache_type, "context window");

        let mtmd_ctx = match mmproj_path {
            Some(p) => {
                let params = MtmdContextParams::default();
                let ctx = MtmdContext::init_from_file(
                    p.to_str()
                        .ok_or_else(|| NodeError::Inference("invalid mmproj path".into()))?,
                    &model,
                    &params,
                )
                .map_err(|e| NodeError::Inference(format!("failed to init mtmd context: {e}")))?;
                tracing::info!(
                    path = %p.display(),
                    vision = ctx.support_vision(),
                    audio = ctx.support_audio(),
                    "multimodal projector loaded"
                );
                Some(ctx)
            }
            None => None,
        };

        Ok(InferenceEngine {
            backend,
            model,
            mtmd_ctx,
            gpu_layers,
            ctx_limit,
            kv_cache_type,
        })
    }

    /// Whether this engine has a multimodal projector loaded.
    pub fn has_multimodal(&self) -> bool {
        self.mtmd_ctx.is_some()
    }

    /// Return the number of GPU layers configured.
    #[allow(dead_code)]
    pub fn gpu_layers(&self) -> i32 {
        self.gpu_layers
    }

    /// The model's native training context length.
    #[allow(dead_code)]
    pub fn n_ctx_train(&self) -> u32 {
        self.model.n_ctx_train()
    }

    /// The effective context limit (possibly capped by --context-size).
    pub fn ctx_limit(&self) -> u32 {
        self.ctx_limit
    }

    /// Count prompt tokens without creating a context (LlamaModel is Send+Sync).
    pub fn tokenize_count(&self, messages: &[ChatMessage]) -> Result<u32, NodeError> {
        let prompt = self.apply_template(messages)?;
        let tokens = self.model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| NodeError::Inference(format!("tokenization failed: {e}")))?;
        Ok(tokens.len() as u32)
    }

    /// Apply the GGUF-embedded chat template to produce a formatted prompt string.
    pub fn apply_template(&self, messages: &[ChatMessage]) -> Result<String, NodeError> {
        let template = self
            .model
            .chat_template(None)
            .map_err(|e| NodeError::Inference(format!("no chat template in model: {e}")))?;
        let llama_messages: Vec<LlamaChatMessage> = messages
            .iter()
            .map(|m| LlamaChatMessage::new(m.role.clone(), m.content.to_string()))
            .collect::<Result<_, _>>()
            .map_err(|e| NodeError::Inference(format!("invalid chat message: {e}")))?;
        self.model
            .apply_chat_template(&template, &llama_messages, true)
            .map_err(|e| NodeError::Inference(format!("failed to apply chat template: {e}")))
    }

    /// Apply the GGUF-embedded chat template with media parts replaced by the given marker.
    fn apply_template_with_marker(
        &self,
        messages: &[ChatMessage],
        marker: &str,
    ) -> Result<String, NodeError> {
        let template = self
            .model
            .chat_template(None)
            .map_err(|e| NodeError::Inference(format!("no chat template in model: {e}")))?;
        let llama_messages: Vec<LlamaChatMessage> = messages
            .iter()
            .map(|m| {
                LlamaChatMessage::new(m.role.clone(), m.content.text_with_markers(marker))
            })
            .collect::<Result<_, _>>()
            .map_err(|e| NodeError::Inference(format!("invalid chat message: {e}")))?;
        self.model
            .apply_chat_template(&template, &llama_messages, true)
            .map_err(|e| NodeError::Inference(format!("failed to apply chat template: {e}")))
    }

    /// Generate text from a prompt.
    ///
    /// `on_token` is called for each generated token. Return `ControlFlow::Break(())`
    /// to stop generation early.
    pub fn generate<F>(
        &self,
        prompt: &str,
        params: &GenerateParams,
        mut on_token: F,
    ) -> Result<InferenceResult, NodeError>
    where
        F: FnMut(StreamToken) -> ControlFlow<()>,
    {
        // Tokenize prompt
        let tokens = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| NodeError::Inference(format!("tokenization failed: {e}")))?;
        let prompt_token_count = tokens.len() as u32;

        // Pre-flight: check that prompt + max_tokens fits in context
        let needed = prompt_token_count + params.max_tokens;
        if needed > self.ctx_limit {
            return Err(NodeError::Inference(format!(
                "prompt ({prompt_token_count}) + max_tokens ({}) = {needed} exceeds context ({})",
                params.max_tokens, self.ctx_limit
            )));
        }

        // Allocate only what this request needs (saves RAM vs full ctx_limit)
        let ctx_size = std::num::NonZeroU32::new(needed);
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(ctx_size)
            .with_type_k(self.kv_cache_type)
            .with_type_v(self.kv_cache_type);

        let mut ctx = self
            .model
            .new_context(self.backend, ctx_params)
            .map_err(|e| NodeError::Inference(format!("failed to create context: {e}")))?;

        // Evaluate prompt in chunks (n_batch = 2048 default in llama.cpp)
        let prompt_start = Instant::now();
        let n_batch = 2048usize;
        let mut batch = LlamaBatch::new(n_batch.min(tokens.len()).max(1), 1);
        let mut prompt_pos = 0;
        while prompt_pos < tokens.len() {
            batch.clear();
            let chunk_end = (prompt_pos + n_batch).min(tokens.len());
            for i in prompt_pos..chunk_end {
                let is_last = i == tokens.len() - 1;
                batch
                    .add(tokens[i], i as i32, &[0], is_last)
                    .map_err(|e| NodeError::Inference(format!("batch add failed: {e}")))?;
            }
            ctx.decode(&mut batch)
                .map_err(|e| NodeError::Inference(format!("prompt decode failed: {e}")))?;
            prompt_pos = chunk_end;
        }
        let prompt_eval_time_ms = prompt_start.elapsed().as_millis() as u64;

        // Build sampler chain (grammar first to mask invalid tokens, then sampling)
        let mut samplers = vec![];
        if let Some(ref grammar_str) = params.grammar {
            samplers.push(
                LlamaSampler::grammar(&self.model, grammar_str, "root")
                    .map_err(|e| NodeError::Inference(format!("grammar error: {e}")))?,
            );
        }
        if params.temperature > 0.0 {
            samplers.push(LlamaSampler::top_p(params.top_p, 1));
            samplers.push(LlamaSampler::temp(params.temperature));
            samplers.push(LlamaSampler::dist(params.seed.unwrap_or(0)));
        } else {
            samplers.push(LlamaSampler::greedy());
        }
        let mut sampler = LlamaSampler::chain_simple(samplers);

        // Generation loop
        let gen_start = Instant::now();
        let mut generated_text = String::new();
        let mut generated_count: u32 = 0;
        let mut logprobs: Vec<TokenLogprob> = Vec::new();
        let mut current_pos = tokens.len() as i32;
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        // Batch index where logits are available:
        // after chunked prompt eval → last token's position in last chunk; after single-token decode → 0
        let mut logit_batch_idx: i32 = ((tokens.len() - 1) % n_batch) as i32;

        for _ in 0..params.max_tokens {
            // sample() internally calls apply + select + accept
            let new_token = sampler.sample(&ctx, -1);

            if self.model.is_eog_token(new_token) {
                break;
            }

            // Extract logprobs at stride positions
            let gen_index = generated_count as usize;
            if params.logprob_every_n > 0 && gen_index.is_multiple_of(params.logprob_every_n) {
                if let Some(lp) =
                    self.extract_logprob(&ctx, logit_batch_idx, gen_index, new_token, params.logprob_top_k)
                {
                    logprobs.push(lp);
                }
            }

            // Decode token to text
            let piece = self
                .model
                .token_to_piece(new_token, &mut decoder, true, None)
                .unwrap_or_default();
            generated_text.push_str(&piece);
            generated_count += 1;

            // Stream callback
            let stream_token = StreamToken {
                text: piece,
                index: gen_index,
            };
            if let ControlFlow::Break(()) = on_token(stream_token) {
                break;
            }

            // Prepare next batch
            batch.clear();
            batch
                .add(new_token, current_pos, &[0], true)
                .map_err(|e| NodeError::Inference(format!("batch add failed: {e}")))?;
            ctx.decode(&mut batch)
                .map_err(|e| NodeError::Inference(format!("decode failed: {e}")))?;
            logit_batch_idx = 0; // single-token batch → logits at batch index 0
            current_pos += 1;
        }

        let generation_time_ms = gen_start.elapsed().as_millis() as u64;
        let tokens_per_second = if generation_time_ms > 0 {
            (generated_count as f64) / (generation_time_ms as f64 / 1000.0)
        } else {
            0.0
        };

        let proof = if logprobs.is_empty() {
            None
        } else {
            Some(InferenceProof {
                logprobs,
                kv_cache_hash: None,
            })
        };

        Ok(InferenceResult {
            text: generated_text,
            tokens_generated: generated_count,
            prompt_tokens: prompt_token_count,
            generation_time_ms,
            prompt_eval_time_ms,
            tokens_per_second,
            proof,
        })
    }

    /// Generate text from multimodal messages containing image/audio parts.
    ///
    /// Uses the mtmd context to process media, then runs the standard sampling loop.
    pub fn generate_multimodal<F>(
        &self,
        messages: &[ChatMessage],
        params: &GenerateParams,
        mut on_token: F,
    ) -> Result<InferenceResult, NodeError>
    where
        F: FnMut(StreamToken) -> ControlFlow<()>,
    {
        let mtmd_ctx = self
            .mtmd_ctx
            .as_ref()
            .ok_or_else(|| NodeError::Inference("no multimodal context loaded".into()))?;

        // Get the default media marker used by the mtmd tokenizer
        let marker = llama_cpp_2::mtmd::mtmd_default_marker();

        // Apply chat template with media parts replaced by the marker
        let prompt = self.apply_template_with_marker(messages, marker)?;

        // Collect all media byte slices in order across all messages
        let mut media_blobs: Vec<&[u8]> = Vec::new();
        for msg in messages {
            media_blobs.extend(msg.content.media_data());
        }

        // Create bitmaps from media blobs
        let bitmaps: Vec<MtmdBitmap> = media_blobs
            .iter()
            .map(|data| {
                MtmdBitmap::from_buffer(mtmd_ctx, data)
                    .map_err(|e| NodeError::Inference(format!("failed to create bitmap: {e}")))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let bitmap_refs: Vec<&MtmdBitmap> = bitmaps.iter().collect();

        // Tokenize the prompt with media markers resolved to bitmap embeddings
        let input_text = MtmdInputText {
            text: prompt,
            add_special: false, // chat template already includes BOS
            parse_special: true,
        };
        let chunks = mtmd_ctx
            .tokenize(input_text, &bitmap_refs)
            .map_err(|e| NodeError::Inference(format!("mtmd tokenize failed: {e}")))?;

        let prompt_token_count = chunks.total_tokens() as u32;

        // Allocate only what this request needs (saves RAM vs full ctx_limit)
        let needed = prompt_token_count + params.max_tokens;
        let ctx_size = std::num::NonZeroU32::new(needed);
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(ctx_size)
            .with_type_k(self.kv_cache_type)
            .with_type_v(self.kv_cache_type);

        let mut ctx = self
            .model
            .new_context(self.backend, ctx_params)
            .map_err(|e| NodeError::Inference(format!("failed to create context: {e}")))?;

        // Evaluate all chunks (text + media embeddings)
        let prompt_start = Instant::now();
        let n_past = chunks
            .eval_chunks(mtmd_ctx, &ctx, 0, 0, 512, true)
            .map_err(|e| NodeError::Inference(format!("mtmd eval_chunks failed: {e}")))?;
        let prompt_eval_time_ms = prompt_start.elapsed().as_millis() as u64;

        // Build sampler chain (grammar first to mask invalid tokens, then sampling)
        let mut samplers = vec![];
        if let Some(ref grammar_str) = params.grammar {
            samplers.push(
                LlamaSampler::grammar(&self.model, grammar_str, "root")
                    .map_err(|e| NodeError::Inference(format!("grammar error: {e}")))?,
            );
        }
        if params.temperature > 0.0 {
            samplers.push(LlamaSampler::top_p(params.top_p, 1));
            samplers.push(LlamaSampler::temp(params.temperature));
            samplers.push(LlamaSampler::dist(params.seed.unwrap_or(0)));
        } else {
            samplers.push(LlamaSampler::greedy());
        }
        let mut sampler = LlamaSampler::chain_simple(samplers);

        // Generation loop (same as text-only but starting from n_past)
        let gen_start = Instant::now();
        let mut generated_text = String::new();
        let mut generated_count: u32 = 0;
        let logprobs: Vec<TokenLogprob> = Vec::new();
        let mut current_pos = n_past;
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut batch = LlamaBatch::new(1, 1);
        // Always use -1 (C API sentinel for "last logits") for sampling.
        // After single-token decode, batch output index is 0, but -1 always works.
        // Multimodal tasks skip validation so logprob extraction is not needed.

        for _ in 0..params.max_tokens {
            // sample() internally calls apply + select + accept
            let new_token = sampler.sample(&ctx, -1);

            if self.model.is_eog_token(new_token) {
                break;
            }

            let gen_index = generated_count as usize;

            // Decode token to text
            let piece = self
                .model
                .token_to_piece(new_token, &mut decoder, true, None)
                .unwrap_or_default();
            generated_text.push_str(&piece);
            generated_count += 1;

            // Stream callback
            let stream_token = StreamToken {
                text: piece,
                index: gen_index,
            };
            if let ControlFlow::Break(()) = on_token(stream_token) {
                break;
            }

            // Prepare next batch
            batch.clear();
            batch
                .add(new_token, current_pos, &[0], true)
                .map_err(|e| NodeError::Inference(format!("batch add failed: {e}")))?;
            ctx.decode(&mut batch)
                .map_err(|e| NodeError::Inference(format!("decode failed: {e}")))?;
            current_pos += 1;
        }

        let generation_time_ms = gen_start.elapsed().as_millis() as u64;
        let tokens_per_second = if generation_time_ms > 0 {
            (generated_count as f64) / (generation_time_ms as f64 / 1000.0)
        } else {
            0.0
        };

        let proof = if logprobs.is_empty() {
            None
        } else {
            Some(InferenceProof {
                logprobs,
                kv_cache_hash: None,
            })
        };

        Ok(InferenceResult {
            text: generated_text,
            tokens_generated: generated_count,
            prompt_tokens: prompt_token_count,
            generation_time_ms,
            prompt_eval_time_ms,
            tokens_per_second,
            proof,
        })
    }

    /// Prefill-only validation: tokenize prompt+output, run a single forward pass,
    /// and extract logprobs at the same stride positions used during generation.
    ///
    /// Returns an `InferenceProof` that can be compared against the original.
    pub fn validate_prefill(
        &self,
        prompt: &str,
        output_text: &str,
        logprob_every_n: usize,
        logprob_top_k: usize,
    ) -> Result<InferenceProof, NodeError> {
        // Tokenize prompt alone to find the split point
        let prompt_tokens = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| NodeError::Inference(format!("prompt tokenization failed: {e}")))?;
        let n_prompt = prompt_tokens.len();

        // Tokenize prompt + output together
        let full_text = format!("{}{}", prompt, output_text);
        let all_tokens = self
            .model
            .str_to_token(&full_text, AddBos::Always)
            .map_err(|e| NodeError::Inference(format!("full tokenization failed: {e}")))?;
        let n_output = all_tokens.len().saturating_sub(n_prompt);

        if n_output == 0 {
            return Ok(InferenceProof {
                logprobs: vec![],
                kv_cache_hash: None,
            });
        }

        // Compute probe positions: gen_index values [0, N, 2N, ...] where each is < n_output
        let mut probe_gen_indices: Vec<usize> = Vec::new();
        if logprob_every_n > 0 {
            let mut k = 0;
            while k < n_output {
                probe_gen_indices.push(k);
                k += logprob_every_n;
            }
        }

        if probe_gen_indices.is_empty() {
            return Ok(InferenceProof {
                logprobs: vec![],
                kv_cache_hash: None,
            });
        }

        // Create context sized to fit all tokens (+ small padding)
        let ctx_size = std::num::NonZeroU32::new((all_tokens.len() + 64) as u32);
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(ctx_size)
            .with_type_k(self.kv_cache_type)
            .with_type_v(self.kv_cache_type);

        let mut ctx = self
            .model
            .new_context(self.backend, ctx_params)
            .map_err(|e| NodeError::Inference(format!("failed to create context: {e}")))?;

        // Build batch with all tokens. Set output=true only at positions where we need logits.
        // For probe gen_index k, we need logits at sequence position (n_prompt + k - 1) for k > 0,
        // and at (n_prompt - 1) for k == 0 (last prompt token predicts first output token).
        let mut output_positions: Vec<usize> = Vec::new();
        for &k in &probe_gen_indices {
            let seq_pos = if k == 0 { n_prompt - 1 } else { n_prompt + k - 1 };
            output_positions.push(seq_pos);
        }

        // Evaluate in chunks and extract logprobs per-chunk (next decode overwrites logits)
        let n_batch = 2048usize;
        let mut batch = LlamaBatch::new(n_batch.min(all_tokens.len()).max(1), 1);
        let mut logprobs: Vec<TokenLogprob> = Vec::new();

        let mut pos = 0;
        while pos < all_tokens.len() {
            batch.clear();
            let chunk_end = (pos + n_batch).min(all_tokens.len());

            // Track which probe positions fall in this chunk
            let mut chunk_probes: Vec<(usize, usize)> = Vec::new(); // (probe_idx, batch_position)

            for (batch_pos, (i, &token)) in all_tokens.iter().enumerate().skip(pos).take(chunk_end - pos).enumerate() {
                let is_output = output_positions.contains(&i);
                batch
                    .add(token, i as i32, &[0], is_output)
                    .map_err(|e| NodeError::Inference(format!("batch add failed: {e}")))?;
                if is_output {
                    if let Some(probe_idx) = output_positions.iter().position(|&p| p == i) {
                        chunk_probes.push((probe_idx, batch_pos));
                    }
                }
            }

            ctx.decode(&mut batch)
                .map_err(|e| NodeError::Inference(format!("prefill decode failed: {e}")))?;

            // Extract logprobs for this chunk's probes before next decode
            for &(probe_idx, batch_pos) in &chunk_probes {
                let gen_index = probe_gen_indices[probe_idx];
                let target_token = all_tokens[n_prompt + gen_index];
                if let Some(lp) =
                    self.extract_logprob(&ctx, batch_pos as i32, gen_index, target_token, logprob_top_k)
                {
                    logprobs.push(lp);
                }
            }

            pos = chunk_end;
        }

        Ok(InferenceProof {
            logprobs,
            kv_cache_hash: None,
        })
    }

    /// Extract logprob data at a given batch index.
    fn extract_logprob(
        &self,
        ctx: &llama_cpp_2::context::LlamaContext,
        batch_idx: i32,
        position: usize,
        chosen_token: LlamaToken,
        top_k: usize,
    ) -> Option<TokenLogprob> {
        let logits = ctx.get_logits_ith(batch_idx);

        // Compute softmax to get log-probabilities
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_sum: f32 = logits.iter().map(|&l| (l - max_logit).exp()).sum();
        let log_sum = max_logit + exp_sum.ln();

        // Collect (token_id, logprob) for all vocab
        let mut all_logprobs: Vec<(u32, f32)> = logits
            .iter()
            .enumerate()
            .map(|(i, &l)| (i as u32, l - log_sum))
            .collect();

        // Sort by logprob descending
        all_logprobs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let chosen_id = chosen_token.0 as u32;
        let chosen_logprob = all_logprobs
            .iter()
            .find(|(id, _)| *id == chosen_id)
            .map(|(_, lp)| *lp)
            .unwrap_or(f32::NEG_INFINITY);

        let chosen_text = token_to_string(&self.model, chosen_token);

        let top_k_entries: Vec<(String, f32)> = all_logprobs
            .iter()
            .take(top_k)
            .map(|(id, lp)| {
                let text = token_to_string(&self.model, LlamaToken(*id as i32));
                (text, *lp)
            })
            .collect();

        Some(TokenLogprob {
            position,
            token_id: chosen_id,
            token_text: chosen_text,
            logprob: chosen_logprob,
            top_k: top_k_entries,
        })
    }

    /// Compute a placeholder KV-cache hash from logits at a given position.
    #[allow(dead_code)]
    fn kv_cache_hash_placeholder(
        ctx: &llama_cpp_2::context::LlamaContext,
        batch_idx: i32,
    ) -> [u8; 32] {
        let logits = ctx.get_logits_ith(batch_idx);
        let bytes: Vec<u8> = logits.iter().flat_map(|f| f.to_le_bytes()).collect();
        sha256hash(&bytes)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use dkn_protocol::{ContentPart, MessageContent};

    /// Create a minimal 64x64 BMP image with a color gradient (no external deps).
    fn create_test_bmp() -> Vec<u8> {
        let width: u32 = 64;
        let height: u32 = 64;
        let row_bytes = width * 3; // 192, already 4-byte aligned
        let pixel_data_size = row_bytes * height;
        let file_size = 54 + pixel_data_size;

        let mut data = Vec::with_capacity(file_size as usize);

        // BMP file header (14 bytes)
        data.extend_from_slice(b"BM");
        data.extend_from_slice(&file_size.to_le_bytes());
        data.extend_from_slice(&[0u8; 4]); // reserved
        data.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset

        // BITMAPINFOHEADER (40 bytes)
        data.extend_from_slice(&40u32.to_le_bytes()); // header size
        data.extend_from_slice(&width.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes()); // planes
        data.extend_from_slice(&24u16.to_le_bytes()); // bits per pixel
        data.extend_from_slice(&[0u8; 24]); // compression=0, rest zeros

        // Pixel data (bottom-up, BGR)
        for y in 0..height {
            for x in 0..width {
                let r = ((x * 255) / (width - 1)) as u8;
                let g = ((y * 255) / (height - 1)) as u8;
                let b = 128u8;
                data.push(b);
                data.push(g);
                data.push(r);
            }
        }

        data
    }

    /// Integration test: download lfm2.5-vl:1.6b + mmproj, run vision inference.
    ///
    /// Run with:
    ///   cargo test test_vision_inference -- --ignored --nocapture
    ///
    /// Optionally provide your own image:
    ///   TEST_IMAGE_PATH=/path/to/photo.jpg cargo test test_vision_inference -- --ignored --nocapture
    #[tokio::test]
    #[ignore] // requires ~1.5 GB download (model + mmproj)
    async fn test_vision_inference() {
        let registry = crate::models::default_registry();
        let spec = registry.get("lfm2.5-vl:1.6b").unwrap().clone();

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("dria-test-models");
        let cache = crate::models::ModelCache::new(cache_dir).unwrap();

        // Download / cache the GGUF model
        let model_path = if let Some(p) = cache.get_local_path(&spec) {
            println!("model found in cache: {}", p.display());
            p
        } else {
            println!("downloading model (this may take a while)...");
            let hf_path = crate::models::ModelDownloader::download(&spec).await.unwrap();
            cache.link_model(&spec, &hf_path).unwrap()
        };

        // Download / cache the mmproj
        let mmproj_path = if let Some(p) = cache.get_mmproj_path(&spec) {
            println!("mmproj found in cache: {}", p.display());
            p
        } else {
            println!("downloading mmproj (this may take a while)...");
            let hf_path = crate::models::ModelDownloader::download_mmproj(&spec)
                .await
                .unwrap();
            cache.link_mmproj(&spec, &hf_path).unwrap()
        };

        // Load engine with multimodal projector
        println!("loading model + mmproj...");
        let engine = InferenceEngine::load(&model_path, 0, Some(&mmproj_path), None, None).unwrap();
        assert!(engine.has_multimodal(), "engine should have multimodal context");

        // Get test image: from env var or generate a synthetic BMP
        let image_bytes = if let Ok(path) = std::env::var("TEST_IMAGE_PATH") {
            println!("using image: {path}");
            std::fs::read(&path).expect("failed to read TEST_IMAGE_PATH")
        } else {
            println!("using synthetic 64x64 gradient BMP");
            create_test_bmp()
        };

        // Build multimodal chat messages
        let messages = vec![ChatMessage {
            role: "user".into(),
            content: MessageContent::Parts(vec![
                ContentPart::Text {
                    text: "What do you see in this image? Describe it briefly.".into(),
                },
                ContentPart::Image {
                    data: image_bytes,
                },
            ]),
        }];

        let params = GenerateParams {
            max_tokens: 256,
            temperature: 0.0,
            ..Default::default()
        };

        // Run multimodal inference, streaming tokens to stdout
        println!("\n--- model output ---");
        let result = engine
            .generate_multimodal(&messages, &params, |token| {
                print!("{}", token.text);
                ControlFlow::Continue(())
            })
            .unwrap();
        println!("\n--- end output ---\n");

        println!(
            "tokens: {} | prompt: {} | time: {}ms | {:.1} tok/s",
            result.tokens_generated,
            result.prompt_tokens,
            result.generation_time_ms,
            result.tokens_per_second,
        );

        assert!(!result.text.is_empty(), "model should produce output");
        assert!(result.tokens_generated > 0);
    }

    /// Helper to load a model from cache (or download).
    async fn load_model(spec: crate::models::registry::ModelSpec) -> (InferenceEngine, String) {
        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("dria-test-models");
        let cache = crate::models::ModelCache::new(cache_dir).unwrap();

        let model_path = if let Some(p) = cache.get_local_path(&spec) {
            println!("model found in cache: {}", p.display());
            p
        } else {
            println!("downloading model...");
            let hf_path = crate::models::ModelDownloader::download(&spec).await.unwrap();
            cache.link_model(&spec, &hf_path).unwrap()
        };

        let name = spec.name.clone();
        let engine = InferenceEngine::load(&model_path, 0, None, None, None).unwrap();
        (engine, name)
    }

    /// Load lfm2.5:1.2b from the default registry.
    async fn load_text_model() -> (InferenceEngine, String) {
        let registry = crate::models::default_registry();
        let spec = registry.get("lfm2.5:1.2b").unwrap().clone();
        load_model(spec).await
    }

    /// Load a small Qwen 3.5 model for grammar-compatible testing.
    async fn load_qwen_model() -> (InferenceEngine, String) {
        let spec = crate::models::registry::ModelSpec {
            name: "qwen3.5:0.8b".into(),
            hf_repo: "unsloth/Qwen3.5-0.8B-GGUF".into(),
            hf_file: "Qwen3.5-0.8B-Q4_K_M.gguf".into(),
            sha256: None,
            model_type: dkn_protocol::ModelType::Text,
            hf_mmproj_file: None,
        };
        load_model(spec).await
    }

    /// End-to-end validation test:
    /// 1. Generate text with logprob_every_n=8 (greedy so output is deterministic)
    /// 2. validate_prefill() with the same prompt+output
    /// 3. compare_proofs() — should Pass
    ///
    /// Run with:
    ///   cargo test test_validate_prefill_e2e -- --ignored --nocapture
    #[tokio::test]
    #[ignore] // requires lfm2.5:1.2b model (~800 MB)
    async fn test_validate_prefill_e2e() {
        let (engine, _model_name) = load_text_model().await;

        let messages = vec![ChatMessage {
            role: "user".into(),
            content: "What is 2 + 2? Answer in one word.".into(),
        }];

        let prompt = engine.apply_template(&messages).unwrap();

        // Generate with logprobs every 8 tokens, greedy (deterministic)
        let params = GenerateParams {
            max_tokens: 64,
            temperature: 0.0,
            logprob_every_n: 8,
            logprob_top_k: 5,
            ..Default::default()
        };

        let gen_result = engine
            .generate(&prompt, &params, |_| ControlFlow::Continue(()))
            .unwrap();

        println!("generated: {:?}", gen_result.text);
        println!("tokens: {}", gen_result.tokens_generated);

        let original_proof = gen_result.proof.as_ref().expect("should have proof with logprob_every_n=8");
        println!("original proof positions: {:?}",
            original_proof.logprobs.iter().map(|lp| lp.position).collect::<Vec<_>>()
        );

        // Now validate: prefill-only forward pass
        let validator_proof = engine
            .validate_prefill(&prompt, &gen_result.text, 8, 5)
            .unwrap();

        println!("validator proof positions: {:?}",
            validator_proof.logprobs.iter().map(|lp| lp.position).collect::<Vec<_>>()
        );

        // Both proofs should have the same positions
        assert_eq!(
            original_proof.logprobs.len(),
            validator_proof.logprobs.len(),
            "proof lengths should match"
        );

        // Compare position by position
        for (orig, val) in original_proof.logprobs.iter().zip(validator_proof.logprobs.iter()) {
            assert_eq!(orig.position, val.position, "positions should match");
            assert_eq!(orig.token_id, val.token_id, "token IDs should match at position {}", orig.position);
            let diff = (orig.logprob - val.logprob).abs();
            println!(
                "pos {} | token '{}' | orig_lp={:.4} | val_lp={:.4} | diff={:.4}",
                orig.position, orig.token_text, orig.logprob, val.logprob, diff
            );
            assert!(
                diff < 0.5,
                "logprob diff too large at position {}: {diff}",
                orig.position
            );
        }

        println!("\nall positions match — validation passed!");
    }

    /// End-to-end structured output test:
    /// 1. Test a trivial GBNF grammar to verify grammar sampling works
    /// 2. Generate with json_object grammar (greedy) — output must be valid JSON
    /// 3. Generate with json_schema grammar — output must match the schema
    ///
    /// Run with:
    ///   cargo test test_structured_output_e2e -- --ignored --nocapture
    #[tokio::test]
    #[ignore] // requires qwen3.5:0.8b model (~533 MB download)
    async fn test_structured_output_e2e() {
        let (engine, _model_name) = load_qwen_model().await;

        // --- Step 1: trivial GBNF grammar to confirm grammar sampling works ---
        {
            let grammar = r#"root ::= "hello""#.to_string();
            let messages = vec![ChatMessage {
                role: "user".into(),
                content: "Say hello".into(),
            }];
            let prompt = engine.apply_template(&messages).unwrap();

            let params = GenerateParams {
                max_tokens: 16,
                temperature: 0.0,
                grammar: Some(grammar),
                ..Default::default()
            };

            println!("\n--- trivial grammar test ---");
            let result = engine
                .generate(&prompt, &params, |_| ControlFlow::Continue(()))
                .unwrap();
            println!("output: {:?}", result.text);
            assert_eq!(result.text, "hello", "trivial grammar should constrain to 'hello'");
            println!("trivial grammar OK");
        }

        // --- Step 2: json_object mode (permissive JSON) ---
        {
            let json_grammar = llama_cpp_2::json_schema_to_grammar(r#"{"type": "object"}"#)
                .expect("json_object grammar should convert");
            println!("\njson_object grammar length: {} chars", json_grammar.len());

            let messages = vec![ChatMessage {
                role: "user".into(),
                content: "Return a JSON object with a field called 'answer' set to 42.".into(),
            }];
            let prompt = engine.apply_template(&messages).unwrap();

            let params = GenerateParams {
                max_tokens: 128,
                temperature: 0.0,
                grammar: Some(json_grammar),
                ..Default::default()
            };

            print!("\n--- json_object output ---\n");
            let result = engine
                .generate(&prompt, &params, |tok| {
                    print!("{}", tok.text);
                    ControlFlow::Continue(())
                })
                .unwrap();
            println!("\n--- end ---");

            let text = result.text.trim();
            assert!(!text.is_empty(), "should produce output");

            let parsed: serde_json::Value =
                serde_json::from_str(text).expect("json_object output must be valid JSON");
            assert!(parsed.is_object(), "should be a JSON object");
            println!("parsed JSON: {parsed}");
        }

        // --- Step 3: json_schema mode (specific schema) ---
        {
            let schema = serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "age": { "type": "integer" }
                },
                "required": ["name", "age"],
                "additionalProperties": false
            });

            let schema_str = serde_json::to_string(&schema).unwrap();
            let schema_grammar = llama_cpp_2::json_schema_to_grammar(&schema_str)
                .expect("json_schema grammar should convert");
            println!("\njson_schema grammar length: {} chars", schema_grammar.len());

            let messages = vec![ChatMessage {
                role: "user".into(),
                content: "Give me a person named Alice who is 30 years old.".into(),
            }];
            let prompt = engine.apply_template(&messages).unwrap();

            let params = GenerateParams {
                max_tokens: 128,
                temperature: 0.0,
                grammar: Some(schema_grammar),
                ..Default::default()
            };

            print!("\n--- json_schema output ---\n");
            let result = engine
                .generate(&prompt, &params, |tok| {
                    print!("{}", tok.text);
                    ControlFlow::Continue(())
                })
                .unwrap();
            println!("\n--- end ---");

            let text = result.text.trim();
            assert!(!text.is_empty(), "should produce output");

            let parsed: serde_json::Value =
                serde_json::from_str(text).expect("json_schema output must be valid JSON");
            assert!(parsed.is_object(), "should be a JSON object");
            assert!(parsed.get("name").is_some(), "should have 'name' field");
            assert!(parsed.get("age").is_some(), "should have 'age' field");
            assert!(parsed["name"].is_string(), "'name' should be a string");
            assert!(parsed["age"].is_number(), "'age' should be a number");
            println!("parsed JSON: {parsed}");
        }

        println!("\nstructured output test passed!");
    }

    /// Grammar test with lfm2.5:1.2b — verify grammar sampling works across tokenizer types.
    ///
    /// Run with:
    ///   cargo test test_structured_output_lfm2 -- --ignored --nocapture
    #[tokio::test]
    #[ignore] // requires lfm2.5:1.2b model (~800 MB)
    async fn test_structured_output_lfm2() {
        let (engine, _model_name) = load_text_model().await;

        // Trivial grammar
        {
            let grammar = r#"root ::= "hello""#.to_string();
            let messages = vec![ChatMessage {
                role: "user".into(),
                content: "Say hello".into(),
            }];
            let prompt = engine.apply_template(&messages).unwrap();

            let params = GenerateParams {
                max_tokens: 16,
                temperature: 0.0,
                grammar: Some(grammar),
                ..Default::default()
            };

            println!("\n--- lfm2 trivial grammar test ---");
            let result = engine
                .generate(&prompt, &params, |_| ControlFlow::Continue(()))
                .unwrap();
            println!("output: {:?}", result.text);
            assert_eq!(result.text, "hello");
            println!("trivial grammar OK");
        }

        // Class-like schema with nested object, array, and enum
        {
            let schema = serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "age": { "type": "integer" },
                    "role": { "type": "string", "enum": ["admin", "user", "moderator"] },
                    "address": {
                        "type": "object",
                        "properties": {
                            "city": { "type": "string" },
                            "country": { "type": "string" }
                        },
                        "required": ["city", "country"],
                        "additionalProperties": false
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                },
                "required": ["name", "age", "role", "address", "tags"],
                "additionalProperties": false
            });

            let schema_str = serde_json::to_string(&schema).unwrap();
            let grammar = llama_cpp_2::json_schema_to_grammar(&schema_str)
                .expect("class schema should convert");
            println!("\nclass schema grammar length: {} chars", grammar.len());

            let messages = vec![ChatMessage {
                role: "user".into(),
                content: "Create a user profile for Alice, age 30, admin role, lives in Istanbul Turkey, tags: developer and lead.".into(),
            }];
            let prompt = engine.apply_template(&messages).unwrap();

            let params = GenerateParams {
                max_tokens: 256,
                temperature: 0.0,
                grammar: Some(grammar),
                ..Default::default()
            };

            print!("\n--- lfm2 class-like schema output ---\n");
            let result = engine
                .generate(&prompt, &params, |tok| {
                    print!("{}", tok.text);
                    ControlFlow::Continue(())
                })
                .unwrap();
            println!("\n--- end ---");

            let parsed: serde_json::Value =
                serde_json::from_str(result.text.trim()).expect("must be valid JSON");
            assert!(parsed.is_object());
            assert!(parsed["name"].is_string());
            assert!(parsed["age"].is_number());
            let role = parsed["role"].as_str().unwrap();
            assert!(["admin", "user", "moderator"].contains(&role), "role must be enum value");
            assert!(parsed["address"].is_object());
            assert!(parsed["address"]["city"].is_string());
            assert!(parsed["address"]["country"].is_string());
            assert!(parsed["tags"].is_array());
            println!("parsed: {parsed}");
        }

        println!("\nlfm2 structured output OK");
    }
}
