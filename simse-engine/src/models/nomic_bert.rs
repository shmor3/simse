//! NomicBERT model implementation for nomic-ai/nomic-embed-text-v1.5.
//!
//! NomicBERT differs from standard BERT in several ways:
//!   - Rotary position embeddings (RoPE) instead of learned absolute
//!   - SwiGLU MLP instead of GELU FFN
//!   - Fused QKV projection (single weight matrix)
//!   - No bias on attention/MLP linear layers
//!   - GPT-2-style config field names (n_embd, n_head, n_layer, n_inner)

use anyhow::Result;
use candle_core::{DType, Device, IndexOp, Tensor, D};
use candle_nn::{layer_norm, linear_no_bias, LayerNorm, Linear, Module, VarBuilder};
use serde::Deserialize;

// ── Config ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct NomicBertConfig {
    pub vocab_size: usize,
    pub n_embd: usize,
    pub n_head: usize,
    pub n_layer: usize,
    pub n_inner: usize,
    pub n_positions: usize,
    pub type_vocab_size: usize,
    #[serde(default = "default_layer_norm_eps")]
    pub layer_norm_epsilon: f64,
    #[serde(default = "default_rotary_base")]
    pub rotary_emb_base: f32,
    #[serde(default = "default_rotary_fraction")]
    pub rotary_emb_fraction: f64,
    #[serde(default)]
    pub rotary_emb_interleaved: bool,
    #[serde(default)]
    pub pad_token_id: usize,
}

fn default_layer_norm_eps() -> f64 {
    1e-12
}

fn default_rotary_base() -> f32 {
    1000.0
}

fn default_rotary_fraction() -> f64 {
    1.0
}

impl NomicBertConfig {
    fn head_dim(&self) -> usize {
        self.n_embd / self.n_head
    }

    fn rotary_dim(&self) -> usize {
        (self.head_dim() as f64 * self.rotary_emb_fraction) as usize
    }
}

// ── Rotary Embeddings ────────────────────────────────────────────────────

struct RotaryEmbedding {
    cos: Tensor,
    sin: Tensor,
}

impl RotaryEmbedding {
    fn new(config: &NomicBertConfig, device: &Device) -> Result<Self> {
        let head_dim = config.head_dim();
        let rotary_dim = config.rotary_dim();
        let max_len = config.n_positions;

        // inv_freq = 1.0 / (base ^ (arange(0, rotary_dim, 2) / rotary_dim))
        let inv_freq: Vec<f32> = (0..rotary_dim)
            .step_by(2)
            .map(|i| 1.0 / config.rotary_emb_base.powf(i as f32 / head_dim as f32))
            .collect();
        let inv_freq = Tensor::new(inv_freq, device)?;

        // t = arange(0, max_len)
        let t: Vec<f32> = (0..max_len).map(|i| i as f32).collect();
        let t = Tensor::new(t, device)?;

        // freqs = outer(t, inv_freq) -> (max_len, rotary_dim/2)
        let freqs = t.unsqueeze(1)?.matmul(&inv_freq.unsqueeze(0)?)?;

        let cos = freqs.cos()?;
        let sin = freqs.sin()?;

        Ok(Self { cos, sin })
    }

    fn apply(&self, q: &Tensor, k: &Tensor, offset: usize) -> Result<(Tensor, Tensor)> {
        let (_b, _h, seq_len, _d) = q.dims4()?;
        let cos = self.cos.i(offset..offset + seq_len)?;
        let sin = self.sin.i(offset..offset + seq_len)?;
        let q_rot = candle_nn::rotary_emb::rope(q, &cos, &sin)?;
        let k_rot = candle_nn::rotary_emb::rope(k, &cos, &sin)?;
        Ok((q_rot, k_rot))
    }
}

// ── SwiGLU MLP ───────────────────────────────────────────────────────────

struct SwiGluMlp {
    fc11: Linear,
    fc12: Linear,
    fc2: Linear,
}

impl SwiGluMlp {
    fn new(vb: VarBuilder, config: &NomicBertConfig) -> Result<Self> {
        let fc11 = linear_no_bias(config.n_embd, config.n_inner, vb.pp("fc11"))?;
        let fc12 = linear_no_bias(config.n_embd, config.n_inner, vb.pp("fc12"))?;
        let fc2 = linear_no_bias(config.n_inner, config.n_embd, vb.pp("fc2"))?;
        Ok(Self { fc11, fc12, fc2 })
    }

    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        // SwiGLU: fc11(x) * silu(fc12(x))
        let value = self.fc11.forward(xs)?;
        let gate = candle_nn::Activation::Silu.forward(&self.fc12.forward(xs)?)?;
        let y = (value * gate)?;
        self.fc2.forward(&y).map_err(Into::into)
    }
}

// ── Attention ────────────────────────────────────────────────────────────

struct NomicAttention {
    wqkv: Linear,
    out_proj: Linear,
    n_head: usize,
    head_dim: usize,
}

impl NomicAttention {
    fn new(vb: VarBuilder, config: &NomicBertConfig) -> Result<Self> {
        let head_dim = config.head_dim();
        let wqkv = linear_no_bias(config.n_embd, 3 * config.n_embd, vb.pp("Wqkv"))?;
        let out_proj = linear_no_bias(config.n_embd, config.n_embd, vb.pp("out_proj"))?;
        Ok(Self {
            wqkv,
            out_proj,
            n_head: config.n_head,
            head_dim,
        })
    }

    fn forward(
        &self,
        xs: &Tensor,
        attention_mask: &Tensor,
        rotary: &RotaryEmbedding,
    ) -> Result<Tensor> {
        let (b_sz, seq_len, _) = xs.dims3()?;

        // Fused QKV projection
        let qkv = self.wqkv.forward(xs)?;
        // Reshape: (batch, seq, 3*n_embd) -> (batch, seq, 3, n_head, head_dim)
        let qkv = qkv.reshape((b_sz, seq_len, 3, self.n_head, self.head_dim))?;

        // Split Q, K, V and transpose to (batch, n_head, seq, head_dim)
        let q = qkv.i((.., .., 0))?.transpose(1, 2)?.contiguous()?;
        let k = qkv.i((.., .., 1))?.transpose(1, 2)?.contiguous()?;
        let v = qkv.i((.., .., 2))?.transpose(1, 2)?;

        // Apply rotary embeddings to Q and K
        let (q, k) = rotary.apply(&q, &k, 0)?;

        // Scaled dot-product attention
        let scale = 1.0 / (self.head_dim as f64).sqrt();
        let scores = (q.matmul(&k.transpose(D::Minus2, D::Minus1)?)? * scale)?;
        let scores = scores.broadcast_add(attention_mask)?;
        let probs = candle_nn::ops::softmax_last_dim(&scores)?;
        let attn_output = probs.matmul(&v)?;

        // Reshape back: (batch, n_head, seq, head_dim) -> (batch, seq, n_embd)
        let attn_output = attn_output
            .transpose(1, 2)?
            .reshape((b_sz, seq_len, self.n_head * self.head_dim))?;

        self.out_proj.forward(&attn_output).map_err(Into::into)
    }
}

// ── Transformer Block ────────────────────────────────────────────────────

struct NomicBertBlock {
    attn: NomicAttention,
    mlp: SwiGluMlp,
    norm1: LayerNorm,
    norm2: LayerNorm,
}

impl NomicBertBlock {
    fn new(vb: VarBuilder, config: &NomicBertConfig) -> Result<Self> {
        let attn = NomicAttention::new(vb.pp("attn"), config)?;
        let mlp = SwiGluMlp::new(vb.pp("mlp"), config)?;
        let norm1 = layer_norm(config.n_embd, config.layer_norm_epsilon, vb.pp("norm1"))?;
        let norm2 = layer_norm(config.n_embd, config.layer_norm_epsilon, vb.pp("norm2"))?;
        Ok(Self {
            attn,
            mlp,
            norm1,
            norm2,
        })
    }

    fn forward(
        &self,
        xs: &Tensor,
        attention_mask: &Tensor,
        rotary: &RotaryEmbedding,
    ) -> Result<Tensor> {
        // Post-norm: residual + norm(attn(x))
        let attn_out = self.attn.forward(xs, attention_mask, rotary)?;
        let xs = self.norm1.forward(&(xs + attn_out)?)?;

        // Post-norm: residual + norm(mlp(x))
        let mlp_out = self.mlp.forward(&xs)?;
        let xs = self.norm2.forward(&(xs + mlp_out)?)?;

        Ok(xs)
    }
}

// ── NomicBERT Model ──────────────────────────────────────────────────────

pub struct NomicBertModel {
    word_embeddings: candle_nn::Embedding,
    token_type_embeddings: candle_nn::Embedding,
    emb_ln: LayerNorm,
    layers: Vec<NomicBertBlock>,
    rotary: RotaryEmbedding,
    pub device: Device,
}

impl NomicBertModel {
    pub fn load(vb: VarBuilder, config: &NomicBertConfig) -> Result<Self> {
        let word_embeddings = candle_nn::embedding(
            config.vocab_size,
            config.n_embd,
            vb.pp("embeddings").pp("word_embeddings"),
        )?;
        let token_type_embeddings = candle_nn::embedding(
            config.type_vocab_size,
            config.n_embd,
            vb.pp("embeddings").pp("token_type_embeddings"),
        )?;
        let emb_ln = layer_norm(config.n_embd, config.layer_norm_epsilon, vb.pp("emb_ln"))?;

        let mut layers = Vec::with_capacity(config.n_layer);
        for i in 0..config.n_layer {
            let layer = NomicBertBlock::new(
                vb.pp("encoder").pp("layers").pp(i.to_string()),
                config,
            )?;
            layers.push(layer);
        }

        let rotary = RotaryEmbedding::new(config, vb.device())?;

        Ok(Self {
            word_embeddings,
            token_type_embeddings,
            emb_ln,
            layers,
            rotary,
            device: vb.device().clone(),
        })
    }

    pub fn forward(
        &self,
        input_ids: &Tensor,
        token_type_ids: &Tensor,
        attention_mask: Option<&Tensor>,
    ) -> Result<Tensor> {
        let (b_sz, seq_len) = input_ids.dims2()?;

        // Embeddings: word + token_type (no position embeddings — RoPE handles that)
        let word_emb = self.word_embeddings.forward(input_ids)?;
        let type_emb = self.token_type_embeddings.forward(token_type_ids)?;
        let mut hidden_states = self.emb_ln.forward(&(word_emb + type_emb)?)?;

        // Build extended attention mask: (b, 1, 1, seq) with 0.0 / -10000.0
        let attention_mask = match attention_mask {
            Some(mask) => {
                let mask = mask.to_dtype(DType::F32)?;
                let inverted = (mask.ones_like()? - &mask)?;
                (inverted * (-10000.0f64))?.unsqueeze(1)?.unsqueeze(1)?
            }
            None => Tensor::zeros((b_sz, 1, 1, seq_len), DType::F32, &self.device)?,
        };

        // Transformer blocks
        for layer in &self.layers {
            hidden_states = layer.forward(&hidden_states, &attention_mask, &self.rotary)?;
        }

        Ok(hidden_states)
    }
}
