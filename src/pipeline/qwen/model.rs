use luminal::{
    dtype::DType,
    graph::Graph,
    prelude::{F32Pow, GraphTensor},
    shape::Expression,
};
use luminal_nn::LayerNorm;

/// Qwen3 model configuration. Parameterized so a small draft (0.6B) and a bigger target
/// (1.7B/4B) can both load from this one module — and so the GLM-5.2 port can follow the
/// same shape with its own config.
#[derive(Clone, Copy, Debug)]
pub struct QwenConfig {
    pub layers: usize,
    pub hidden: usize,
    pub intermediate: usize,
    pub head_dim: usize,
    pub n_heads: usize,
    pub n_kv_heads: usize,
    pub vocab_size: usize,
    pub rms_norm_eps: f32,
    pub rope_theta: f32,
}

impl QwenConfig {
    pub fn q_dim(&self) -> usize {
        self.n_heads * self.head_dim
    }
    pub fn kv_dim(&self) -> usize {
        self.n_kv_heads * self.head_dim
    }
    pub fn kv_groups(&self) -> usize {
        self.n_heads / self.n_kv_heads
    }

    pub const QWEN3_0_6B: Self = Self {
        layers: 28,
        hidden: 1024,
        intermediate: 3072,
        head_dim: 128,
        n_heads: 16,
        n_kv_heads: 8,
        vocab_size: 151936,
        rms_norm_eps: 1e-6,
        rope_theta: 1e6,
    };
    pub const QWEN3_1_7B: Self = Self {
        layers: 28,
        hidden: 2048,
        intermediate: 6144,
        head_dim: 128,
        n_heads: 16,
        n_kv_heads: 8,
        vocab_size: 151936,
        rms_norm_eps: 1e-6,
        rope_theta: 1e6,
    };
    pub const QWEN3_4B: Self = Self {
        layers: 36,
        hidden: 2560,
        intermediate: 9728,
        head_dim: 128,
        n_heads: 32,
        n_kv_heads: 8,
        vocab_size: 151936,
        rms_norm_eps: 1e-6,
        rope_theta: 1e6,
    };
}

pub struct KVCache {
    pub k_caches: Vec<GraphTensor>,
    pub v_caches: Vec<GraphTensor>,
    pub max_seq: usize,
}

impl KVCache {
    pub fn new(cx: &mut Graph, cfg: &QwenConfig, max_seq: usize) -> Self {
        let mut k_caches = Vec::with_capacity(cfg.layers);
        let mut v_caches = Vec::with_capacity(cfg.layers);
        for l in 0..cfg.layers {
            // KV cache stored F16 (2 bytes/elem); the scatter writes F16 k_rope/v rows directly.
            k_caches.push(
                persist(cx, format!("kv_cache.{l}.k"), (cfg.n_kv_heads, max_seq, cfg.head_dim))
                    .as_dtype(DType::F16),
            );
            v_caches.push(
                persist(cx, format!("kv_cache.{l}.v"), (cfg.n_kv_heads, max_seq, cfg.head_dim))
                    .as_dtype(DType::F16),
            );
        }
        Self { k_caches, v_caches, max_seq }
    }
}

pub struct Qwen {
    cfg: QwenConfig,
    pub embedding: GraphTensor,
    layers: Vec<QwenLayer>,
    lm_norm: LayerNorm,
}

impl Qwen {
    pub fn init(cx: &mut Graph, cfg: QwenConfig) -> Self {
        Self {
            embedding: persist(cx, "model.embed_tokens.weight", (cfg.vocab_size, cfg.hidden))
                .as_dtype(DType::F16),
            layers: (0..cfg.layers).map(|l| QwenLayer::init(cx, cfg, l)).collect(),
            lm_norm: rms_norm(cx, cfg, "model.norm.weight"),
            cfg,
        }
    }

    #[tracing::instrument(skip_all)]
    pub fn forward(
        &self,
        token_ids: GraphTensor,
        pos_ids: GraphTensor,
        kv_cache: &KVCache,
    ) -> (GraphTensor, Vec<(GraphTensor, GraphTensor)>) {
        let mut x = token_embedding(self.embedding, token_ids, &self.cfg);
        let mut cache_outputs = Vec::with_capacity(self.layers.len());
        for (i, layer) in self.layers.iter().enumerate() {
            let (x_new, k_out, v_out) =
                layer.forward(x, pos_ids, kv_cache.k_caches[i], kv_cache.v_caches[i], kv_cache.max_seq);
            x = x_new;
            cache_outputs.push((k_out, v_out));
        }
        // Tied embeddings: lm_head = embedding.t(). Logits cast to F32 for the host argmax path.
        let normed = norm_in_f32(&self.lm_norm, x);
        let mut logits = normed.matmul(self.embedding.t());
        if logits.dtype != DType::F32 {
            logits = logits.cast(DType::F32);
        }
        (logits, cache_outputs)
    }
}

struct QwenLayer {
    cfg: QwenConfig,
    up: GraphTensor,
    gate: GraphTensor,
    down: GraphTensor,
    q_proj: GraphTensor,
    k_proj: GraphTensor,
    v_proj: GraphTensor,
    o_proj: GraphTensor,
    q_norm: GraphTensor,
    k_norm: GraphTensor,
    attn_rms: LayerNorm,
    mlp_rms: LayerNorm,
}

impl QwenLayer {
    fn init(cx: &mut Graph, cfg: QwenConfig, l: usize) -> Self {
        Self {
            up: layer_weight(cx, l, "mlp.up_proj", (cfg.intermediate, cfg.hidden)).as_dtype(DType::F16),
            gate: layer_weight(cx, l, "mlp.gate_proj", (cfg.intermediate, cfg.hidden)).as_dtype(DType::F16),
            down: layer_weight(cx, l, "mlp.down_proj", (cfg.hidden, cfg.intermediate)).as_dtype(DType::F16),
            q_proj: layer_weight(cx, l, "self_attn.q_proj", (cfg.q_dim(), cfg.hidden)).as_dtype(DType::F16),
            k_proj: layer_weight(cx, l, "self_attn.k_proj", (cfg.kv_dim(), cfg.hidden)).as_dtype(DType::F16),
            v_proj: layer_weight(cx, l, "self_attn.v_proj", (cfg.kv_dim(), cfg.hidden)).as_dtype(DType::F16),
            o_proj: layer_weight(cx, l, "self_attn.o_proj", (cfg.hidden, cfg.q_dim())).as_dtype(DType::F16),
            q_norm: layer_weight(cx, l, "self_attn.q_norm", cfg.head_dim),
            k_norm: layer_weight(cx, l, "self_attn.k_norm", cfg.head_dim),
            attn_rms: rms_norm(cx, cfg, format!("model.layers.{l}.input_layernorm.weight")),
            mlp_rms: rms_norm(cx, cfg, format!("model.layers.{l}.post_attention_layernorm.weight")),
            cfg,
        }
    }

    pub fn forward(
        &self,
        mut x: GraphTensor,
        pos_ids: GraphTensor,
        k_cache_in: GraphTensor,
        v_cache_in: GraphTensor,
        max_seq: usize,
    ) -> (GraphTensor, GraphTensor, GraphTensor) {
        let cfg = &self.cfg;
        let x_attn = norm_in_f32(&self.attn_rms, x);
        let q = x_attn.matmul(self.q_proj.t());
        let k = x_attn.matmul(self.k_proj.t());
        let v = x_attn.matmul(self.v_proj.t());

        let q_rope = qwen_rotary_embeddings(qk_norm(q, self.q_norm, cfg, cfg.n_heads), pos_ids, cfg, cfg.n_heads);
        let k_rope = qwen_rotary_embeddings(qk_norm(k, self.k_norm, cfg, cfg.n_kv_heads), pos_ids, cfg, cfg.n_kv_heads);

        let (attn_out, k_cache_out, v_cache_out) =
            hlir_attention(q_rope, k_rope, v, k_cache_in, v_cache_in, max_seq, cfg);
        x += attn_out.matmul(self.o_proj.t());

        let x_mlp = norm_in_f32(&self.mlp_rms, x);
        let gate = x_mlp.matmul(self.gate.t());
        let up = x_mlp.matmul(self.up.t());
        let swiglu = swiglu_in_f32(gate, up);
        let mlp_out = swiglu.matmul(self.down.t());
        (x + mlp_out, k_cache_out, v_cache_out)
    }
}

fn persist(cx: &mut Graph, name: impl ToString, shape: impl luminal::prelude::ToShape) -> GraphTensor {
    cx.named_tensor(name, shape).persist()
}

fn layer_weight(cx: &mut Graph, layer: usize, suffix: &str, shape: impl luminal::prelude::ToShape) -> GraphTensor {
    persist(cx, format!("model.layers.{layer}.{suffix}.weight"), shape)
}

fn rms_norm(cx: &mut Graph, cfg: QwenConfig, weight_name: impl ToString) -> LayerNorm {
    LayerNorm::new(cfg.hidden, Some(&weight_name.to_string()), None, false, cfg.rms_norm_eps, cx)
}

/// RMS norm computed in F32 with explicit casts when the input is 16-bit.
fn norm_in_f32(norm: &LayerNorm, x: GraphTensor) -> GraphTensor {
    if x.dtype == DType::F32 {
        norm.forward(x)
    } else {
        norm.forward(x.cast(DType::F32)).cast(x.dtype)
    }
}

/// SwiGLU computed in F32 (silu(gate) * up), result cast back to the input dtype.
fn swiglu_in_f32(gate: GraphTensor, up: GraphTensor) -> GraphTensor {
    let dtype = gate.dtype;
    if dtype == DType::F32 {
        gate.swish() * up
    } else {
        let result = gate.cast(DType::F32).swish() * up.cast(DType::F32);
        result.cast(dtype)
    }
}

fn token_embedding(embedding: GraphTensor, token_ids: GraphTensor, cfg: &QwenConfig) -> GraphTensor {
    let seq = token_ids.dims1();
    embedding.gather(
        (token_ids * cfg.hidden).expand_dim(1, cfg.hidden)
            + token_ids.graph().arange(cfg.hidden).expand_dim(0, seq),
    )
}

/// Per-head RMS normalization for QK-norm. Input: [seq, n_heads*head_dim].
fn qk_norm(x: GraphTensor, weight: GraphTensor, cfg: &QwenConfig, n_heads: usize) -> GraphTensor {
    let dtype = x.dtype;
    let seq = x.dims()[0];
    let x = if dtype == DType::F32 { x } else { x.cast(DType::F32) };
    let reshaped = x.split_dims(1, cfg.head_dim);
    let normed = reshaped.std_norm(2, cfg.rms_norm_eps);
    let w = weight.expand_dim(0, n_heads).expand_dim(0, seq);
    let result = (normed * w).merge_dims(1, 2);
    if dtype == DType::F32 {
        result
    } else {
        result.cast(dtype)
    }
}

fn qwen_rotary_embeddings(mut input: GraphTensor, pos_ids: GraphTensor, cfg: &QwenConfig, n_heads: usize) -> GraphTensor {
    let hd = cfg.head_dim;
    input = input.split_dims(1, hd).transpose(0, 1); // [n_heads, seq, head_dim]

    let freqs = input.graph().arange_options(0, hd, 2).cast(DType::F32) / hd as f32;
    let inv_freqs = cfg.rope_theta.pow(freqs).reciprocal();
    let emb = pos_ids.cast(DType::F32).expand_dim(1, 1).matmul(inv_freqs.expand_dim(0, 1));

    let x0 = input.slice((.., .., ..hd / 2));
    let x1 = input.slice((.., .., hd / 2..));

    let mut cos = emb.cos();
    let mut sin = emb.sin();
    if x0.dtype != DType::F32 {
        cos = cos.cast(x0.dtype);
        sin = sin.cast(x0.dtype);
    }
    let cos = cos.expand_dim(0, n_heads);
    let sin = sin.expand_dim(0, n_heads);
    let x0_out = x0 * cos - x1 * sin;
    let x1_out = x1 * cos + x0 * sin;

    x0_out.concat_along(x1_out, 2).transpose(0, 1).merge_dims(1, 2)
}

/// HLIR attention with pre-allocated KV cache using scatter.
fn hlir_attention(
    q_rope: GraphTensor,
    k_rope: GraphTensor,
    v: GraphTensor,
    k_cache_in: GraphTensor,
    v_cache_in: GraphTensor,
    max_seq: usize,
    cfg: &QwenConfig,
) -> (GraphTensor, GraphTensor, GraphTensor) {
    let cx = q_rope.graph();
    let hd = cfg.head_dim;
    let nkv = cfg.n_kv_heads;
    let seq = q_rope.dims()[0];
    let prev = Expression::from('p');
    let total_seq = prev + seq;

    let k_new = k_rope.split_dims(1, hd).transpose(0, 1);
    let v_new = v.split_dims(1, hd).transpose(0, 1);

    let h_offset = cx.arange(nkv) * (max_seq * hd);
    let p_offset = (cx.arange(seq) + prev) * hd;
    let d_offset = cx.arange(hd);
    let scatter_idx = h_offset.expand_dim(1, seq).expand_dim(2, hd)
        + p_offset.expand_dim(0, nkv).expand_dim(2, hd)
        + d_offset.expand_dim(0, nkv).expand_dim(1, seq);

    let k_cache_out = k_new.scatter(scatter_idx, k_cache_in);
    let v_cache_out = v_new.scatter(scatter_idx, v_cache_in);

    let mut k_full = k_cache_out.slice((.., ..total_seq, ..));
    let mut v_full = v_cache_out.slice((.., ..total_seq, ..));
    // Normalize the visible cache axis to total_seq (the slice reports min(max_seq, p+s)).
    k_full.shape.dims[1] = total_seq;
    v_full.shape.dims[1] = total_seq;

    // GQA expand; trailing `* 1.0` forces contiguous materialization.
    let k_3d = k_full.expand_dim(1, cfg.kv_groups()).merge_dims(0, 1) * 1.0;
    let v_3d = v_full.expand_dim(1, cfg.kv_groups()).merge_dims(0, 1) * 1.0;

    let q = q_rope.split_dims(1, hd).transpose(0, 1);

    let scores = (q.matmul(k_3d.transpose(1, 2)) / (hd as f32).sqrt()).cast(DType::F32);

    let q_abs = cx.arange(seq).cast(DType::F32) + prev;
    let k_pos = cx.arange(total_seq).cast(DType::F32);
    let mask = k_pos.expand_dim(0, seq).gt(q_abs.expand_dim(1, total_seq));
    let mask_3d = mask.cast(DType::F32).expand_dim(0, cfg.n_heads);
    let masked_scores = scores + mask_3d * (-1e10f32);

    let attn_weights = masked_scores.softmax(2).cast(v_3d.dtype);
    let attn_out = attn_weights.matmul(v_3d);
    let out = attn_out.transpose(0, 1).merge_dims(1, 2);

    (out, k_cache_out, v_cache_out)
}
