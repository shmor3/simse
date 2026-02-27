use std::path::Path;

use anyhow::Result;
use candle_core::{Device, Tensor, DType};
use candle_nn::VarBuilder;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

use super::{EmbedResult, Embedder};

/// Pooling strategy for extracting a single embedding from token-level outputs.
#[derive(Debug, Clone, Copy)]
pub enum PoolingStrategy {
    /// Average all token embeddings (weighted by attention mask).
    Mean,
    /// Use the [CLS] token embedding (index 0).
    Cls,
}

/// BERT/NomicBERT embedding model via Candle.
pub struct BertEmbedder {
    model: BertModelVariant,
    tokenizer: Tokenizer,
    device: Device,
    normalize: bool,
    pooling: PoolingStrategy,
}

/// Wraps both standard BERT and NomicBERT model variants.
enum BertModelVariant {
    Bert(candle_transformers::models::bert::BertModel),
}

impl BertEmbedder {
    /// Load from HuggingFace Hub.
    pub fn from_hub(model_id: &str, device: &Device) -> Result<Self> {
        #[cfg(not(target_family = "wasm"))]
        {
            let api = hf_hub::api::sync::Api::new()?;
            let repo = api.model(model_id.to_string());

            // Download config, tokenizer, and weights
            let config_path = repo.get("config.json")?;
            let tokenizer_path = repo.get("tokenizer.json")?;
            let weights_path = repo.get("model.safetensors")?;

            Self::from_files(&config_path, &tokenizer_path, &weights_path, device)
        }

        #[cfg(target_family = "wasm")]
        {
            anyhow::bail!(
                "HuggingFace Hub downloads not supported in WASM. \
                 Use BertEmbedder::from_files() with local paths for model: {}",
                model_id
            )
        }
    }

    /// Load from local file paths.
    pub fn from_files(
        config_path: &Path,
        tokenizer_path: &Path,
        weights_path: &Path,
        device: &Device,
    ) -> Result<Self> {
        tracing::info!(
            config = %config_path.display(),
            weights = %weights_path.display(),
            "Loading BERT embedding model"
        );

        // Load config
        let config_str = std::fs::read_to_string(config_path)?;
        let config: candle_transformers::models::bert::Config = serde_json::from_str(&config_str)?;

        // Load tokenizer with padding enabled
        let mut tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        // Enable padding for batch processing
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            ..Default::default()
        }));
        tokenizer.with_truncation(Some(TruncationParams {
            max_length: 512,
            ..Default::default()
        })).map_err(|e| anyhow::anyhow!("Failed to set truncation: {}", e))?;

        // Load weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights_path.to_path_buf()], DType::F32, device)?
        };
        let model = candle_transformers::models::bert::BertModel::load(vb, &config)?;

        Ok(Self {
            model: BertModelVariant::Bert(model),
            tokenizer,
            device: device.clone(),
            normalize: true,
            pooling: PoolingStrategy::Mean,
        })
    }
}

impl Embedder for BertEmbedder {
    fn embed(&self, texts: &[String]) -> Result<EmbedResult> {
        if texts.is_empty() {
            return Ok(EmbedResult {
                embeddings: vec![],
                prompt_tokens: 0,
            });
        }

        tracing::debug!(batch_size = texts.len(), "Encoding batch for embeddings");

        // Tokenize all texts
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("Tokenizer batch encode error: {}", e))?;

        let mut total_tokens: u64 = 0;
        let batch_size = encodings.len();

        // Build input tensors
        let mut all_token_ids: Vec<Vec<u32>> = Vec::with_capacity(batch_size);
        let mut all_type_ids: Vec<Vec<u32>> = Vec::with_capacity(batch_size);
        let mut all_attention_masks: Vec<Vec<f32>> = Vec::with_capacity(batch_size);

        for encoding in &encodings {
            let ids = encoding.get_ids().to_vec();
            let type_ids = encoding.get_type_ids().to_vec();
            let attention_mask: Vec<f32> = encoding
                .get_attention_mask()
                .iter()
                .map(|&m| m as f32)
                .collect();

            total_tokens += ids.len() as u64;
            all_token_ids.push(ids);
            all_type_ids.push(type_ids);
            all_attention_masks.push(attention_mask);
        }

        // Pad to same length and flatten
        let seq_len = all_token_ids.iter().map(|ids| ids.len()).max().unwrap_or(0);

        let flat_token_ids: Vec<u32> = all_token_ids
            .iter()
            .flat_map(|ids| {
                let mut padded = ids.clone();
                padded.resize(seq_len, 0);
                padded
            })
            .collect();

        let flat_type_ids: Vec<u32> = all_type_ids
            .iter()
            .flat_map(|ids| {
                let mut padded = ids.clone();
                padded.resize(seq_len, 0);
                padded
            })
            .collect();

        let flat_attention_mask: Vec<f32> = all_attention_masks
            .iter()
            .flat_map(|mask| {
                let mut padded = mask.clone();
                padded.resize(seq_len, 0.0);
                padded
            })
            .collect();

        // Create tensors
        let token_ids = Tensor::from_vec(flat_token_ids, (batch_size, seq_len), &self.device)?;
        let type_ids = Tensor::from_vec(flat_type_ids, (batch_size, seq_len), &self.device)?;
        let attention_mask = Tensor::from_vec(flat_attention_mask, (batch_size, seq_len), &self.device)?;

        // Forward pass
        let output = match &self.model {
            BertModelVariant::Bert(model) => {
                model.forward(&token_ids, &type_ids, Some(&attention_mask))?
            }
        };

        // Apply pooling
        let pooled = match self.pooling {
            PoolingStrategy::Mean => {
                // Mean pooling with attention mask
                let mask_expanded = attention_mask
                    .unsqueeze(2)?
                    .broadcast_as(output.shape())?;
                let masked = output.mul(&mask_expanded)?;
                let summed = masked.sum(1)?;
                let mask_sum = attention_mask.sum(1)?.unsqueeze(1)?;
                summed.broadcast_div(&mask_sum)?
            }
            PoolingStrategy::Cls => {
                // Take the [CLS] token (index 0) from each sequence
                output.narrow(1, 0, 1)?.squeeze(1)?
            }
        };

        // Apply L2 normalization if enabled
        let final_embeddings = if self.normalize {
            let norms = pooled
                .sqr()?
                .sum_keepdim(1)?
                .sqrt()?;
            pooled.broadcast_div(&norms)?
        } else {
            pooled
        };

        // Convert to Vec<Vec<f32>>
        let embeddings: Vec<Vec<f32>> = final_embeddings
            .to_dtype(DType::F32)?
            .to_device(&Device::Cpu)?
            .to_vec2()?;

        tracing::debug!(
            batch_size,
            total_tokens,
            embedding_dim = embeddings.first().map_or(0, |e| e.len()),
            "Embeddings generated"
        );

        Ok(EmbedResult {
            embeddings,
            prompt_tokens: total_tokens,
        })
    }
}
