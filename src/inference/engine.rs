use std::ops::ControlFlow;
use std::path::Path;
use std::time::Instant;

use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
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
    /// Token positions at which to extract logprobs.
    pub logprob_positions: Vec<usize>,
    /// Top-k alternatives to collect at each logprob position.
    pub logprob_top_k: usize,
}

impl Default for GenerateParams {
    fn default() -> Self {
        Self {
            max_tokens: 512,
            temperature: 0.7,
            top_p: 0.9,
            seed: None,
            logprob_positions: vec![],
            logprob_top_k: 5,
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
    backend: LlamaBackend,
    model: LlamaModel,
    mtmd_ctx: Option<MtmdContext>,
    #[allow(dead_code)]
    gpu_layers: i32,
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
    pub fn load(
        path: &Path,
        gpu_layers: i32,
        mmproj_path: Option<&Path>,
    ) -> Result<Self, NodeError> {
        let backend = LlamaBackend::init()
            .map_err(|e| NodeError::Inference(format!("failed to init llama backend: {e}")))?;

        let model_params = if gpu_layers != 0 {
            let layers = if gpu_layers < 0 { 1000 } else { gpu_layers as u32 };
            LlamaModelParams::default().with_n_gpu_layers(layers)
        } else {
            LlamaModelParams::default()
        };

        let model = LlamaModel::load_from_file(&backend, path, &model_params)
            .map_err(|e| NodeError::Inference(format!("failed to load model: {e}")))?;

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
        let ctx_size = std::num::NonZeroU32::new(2048);
        let ctx_params = LlamaContextParams::default().with_n_ctx(ctx_size);

        let mut ctx = self
            .model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| NodeError::Inference(format!("failed to create context: {e}")))?;

        // Tokenize prompt
        let tokens = self
            .model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| NodeError::Inference(format!("tokenization failed: {e}")))?;
        let prompt_token_count = tokens.len() as u32;

        // Evaluate prompt
        let prompt_start = Instant::now();
        let mut batch = LlamaBatch::new(tokens.len().max(1), 1);
        for (i, &token) in tokens.iter().enumerate() {
            let is_last = i == tokens.len() - 1;
            batch
                .add(token, i as i32, &[0], is_last)
                .map_err(|e| NodeError::Inference(format!("batch add failed: {e}")))?;
        }
        ctx.decode(&mut batch)
            .map_err(|e| NodeError::Inference(format!("prompt decode failed: {e}")))?;
        let prompt_eval_time_ms = prompt_start.elapsed().as_millis() as u64;

        // Build sampler chain (seed is passed via the dist sampler)
        let mut samplers = vec![];
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

        for _ in 0..params.max_tokens {
            let new_token = sampler.sample(&ctx, -1);
            sampler.accept(new_token);

            if self.model.is_eog_token(new_token) {
                break;
            }

            // Extract logprobs if this position was requested
            let gen_index = generated_count as usize;
            if params.logprob_positions.contains(&gen_index) {
                if let Some(lp) =
                    self.extract_logprob(&ctx, -1, gen_index, new_token, params.logprob_top_k)
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

        // Create context with larger size for multimodal
        let ctx_size = std::num::NonZeroU32::new(4096);
        let ctx_params = LlamaContextParams::default().with_n_ctx(ctx_size);

        let mut ctx = self
            .model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| NodeError::Inference(format!("failed to create context: {e}")))?;

        // Evaluate all chunks (text + media embeddings)
        let prompt_start = Instant::now();
        let n_past = chunks
            .eval_chunks(mtmd_ctx, &ctx, 0, 0, 512, true)
            .map_err(|e| NodeError::Inference(format!("mtmd eval_chunks failed: {e}")))?;
        let prompt_eval_time_ms = prompt_start.elapsed().as_millis() as u64;

        // Build sampler chain
        let mut samplers = vec![];
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
        let mut logprobs: Vec<TokenLogprob> = Vec::new();
        let mut current_pos = n_past;
        let mut decoder = encoding_rs::UTF_8.new_decoder();
        let mut batch = LlamaBatch::new(1, 1);

        for _ in 0..params.max_tokens {
            let new_token = sampler.sample(&ctx, -1);
            sampler.accept(new_token);

            if self.model.is_eog_token(new_token) {
                break;
            }

            // Extract logprobs if this position was requested
            let gen_index = generated_count as usize;
            if params.logprob_positions.contains(&gen_index) {
                if let Some(lp) =
                    self.extract_logprob(&ctx, -1, gen_index, new_token, params.logprob_top_k)
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
