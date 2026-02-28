use std::path::Path;

use anyhow::Result;
use candle_core::{Device, Tensor};
use candle_transformers::models::quantized_llama::ModelWeights;
use tokenizers::Tokenizer;

use super::sampling::{self, Sampling};
use super::tokenizer::TokenOutputStream;
use super::{GenerationResult, ModelConfig, SamplingParams, TextGenerator};
use super::weights;

/// Maximum model file size (10 GB).
const MAX_MODEL_FILE_SIZE: u64 = 10 * 1024 * 1024 * 1024;

/// Llama text generation model using quantized GGUF weights via Candle.
pub struct LlamaGenerator {
    model: ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    eos_token_id: Option<u32>,
}

impl LlamaGenerator {
    /// Load from a local GGUF file.
    pub fn from_gguf(model_path: &Path, tokenizer: Tokenizer, device: &Device) -> Result<Self> {
        tracing::info!(path = %model_path.display(), "Loading GGUF model");

        let metadata = std::fs::metadata(model_path)
            .map_err(|e| anyhow::anyhow!("Cannot stat model file '{}': {}", model_path.display(), e))?;
        if metadata.len() > MAX_MODEL_FILE_SIZE {
            anyhow::bail!(
                "Model file too large: {} MB (max {} MB). Use a smaller quantization.",
                metadata.len() / (1024 * 1024),
                MAX_MODEL_FILE_SIZE / (1024 * 1024)
            );
        }

        let mut file = std::fs::File::open(model_path)?;
        let model_content = candle_core::quantized::gguf_file::Content::read(&mut file)?;
        let model = ModelWeights::from_gguf(model_content, &mut file, device)?;

        let eos_token_id = tokenizer
            .token_to_id("</s>")
            .or_else(|| tokenizer.token_to_id("<|eot_id|>"))
            .or_else(|| tokenizer.token_to_id("<|end_of_text|>"));

        Ok(Self {
            model,
            tokenizer,
            device: device.clone(),
            eos_token_id,
        })
    }

    /// Load from HuggingFace Hub (downloads GGUF model + tokenizer).
    pub fn from_hub(model_id: &str, config: &ModelConfig, device: &Device) -> Result<Self> {
        let source = weights::resolve_source(model_id, config.filename.as_deref(), config.revision.as_deref());
        let model_path = source.resolve()?;

        // Resolve tokenizer — tries explicit source, model repo, inferred base repo
        let tokenizer_path = weights::resolve_tokenizer(model_id, config.tokenizer.as_deref())?;
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer from {}: {}", tokenizer_path.display(), e))?;

        Self::from_gguf(&model_path, tokenizer, device)
    }

    /// Load from raw bytes (for embedded weights).
    #[cfg(feature = "embed-weights")]
    pub fn from_bytes(
        model_bytes: &[u8],
        tokenizer: Tokenizer,
        device: &Device,
    ) -> Result<Self> {
        let mut cursor = std::io::Cursor::new(model_bytes);
        let model_content = candle_core::quantized::gguf_file::Content::read(&mut cursor)?;
        let model = ModelWeights::from_gguf(model_content, &mut cursor, device)?;

        let eos_token_id = tokenizer
            .token_to_id("</s>")
            .or_else(|| tokenizer.token_to_id("<|eot_id|>"))
            .or_else(|| tokenizer.token_to_id("<|end_of_text|>"));

        Ok(Self {
            model,
            tokenizer,
            device: device.clone(),
            eos_token_id,
        })
    }
}

impl TextGenerator for LlamaGenerator {
    fn generate(
        &mut self,
        prompt: &str,
        system: Option<&str>,
        params: &SamplingParams,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationResult> {
        // Build the full prompt with system message if provided
        let full_prompt = if let Some(sys) = system {
            format!("{}\n\n{}", sys, prompt)
        } else {
            prompt.to_string()
        };

        // Tokenize
        let encoding = self
            .tokenizer
            .encode(full_prompt.as_str(), true)
            .map_err(|e| anyhow::anyhow!("Tokenizer encode error: {}", e))?;
        let prompt_tokens = encoding.get_ids().to_vec();
        let prompt_token_count = prompt_tokens.len() as u64;

        tracing::debug!(prompt_tokens = prompt_token_count, "Starting generation");

        // Set up sampling
        let sampler = Sampling::from_params(params.temperature, params.top_p, params.top_k);

        // Set up streaming token decoder
        let mut token_stream = TokenOutputStream::new(self.tokenizer.clone());

        // Track generated tokens for repeat penalty
        let mut all_tokens = prompt_tokens.clone();
        let mut generated_count: u64 = 0;
        let mut full_text = String::new();

        // Initial forward pass with all prompt tokens
        let input = Tensor::new(prompt_tokens.as_slice(), &self.device)?.unsqueeze(0)?;
        let logits = self.model.forward(&input, 0)?;
        let logits = logits.squeeze(0)?.squeeze(0)?;

        // Apply repeat penalty
        let penalty_context: Vec<u32> = if params.repeat_last_n > 0 {
            let start = all_tokens.len().saturating_sub(params.repeat_last_n);
            all_tokens[start..].to_vec()
        } else {
            vec![]
        };
        let logits = sampling::apply_repeat_penalty(&logits, params.repeat_penalty, &penalty_context)?;

        // Sample first token
        let next_token = sampler.sample(&logits)?;
        all_tokens.push(next_token);
        generated_count += 1;

        // Emit first token
        if let Some(text) = token_stream.next_token(next_token)? {
            full_text.push_str(&text);
            on_token(&text);
        }

        // Continue generating with wall-clock timeout
        let mut eos_reached = next_token == self.eos_token_id.unwrap_or(u32::MAX);
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(params.generation_timeout_secs);
        let mut stop_reason = if eos_reached { "end_turn" } else { "max_tokens" };

        // Check stop sequences after first token
        let mut hit_stop = false;
        for seq in &params.stop_sequences {
            if full_text.ends_with(seq) {
                full_text.truncate(full_text.len() - seq.len());
                hit_stop = true;
                break;
            }
        }
        if hit_stop {
            eos_reached = true;
            stop_reason = "stop_sequence";
        }

        while !eos_reached && (generated_count as usize) < params.max_tokens {
            if start.elapsed() > timeout {
                tracing::warn!(elapsed = ?start.elapsed(), "Generation timeout reached");
                stop_reason = "timeout";
                break;
            }

            let input = Tensor::new(&[next_token], &self.device)?.unsqueeze(0)?;
            let logits = self.model.forward(&input, prompt_tokens.len() + generated_count as usize - 1)?;
            let logits = logits.squeeze(0)?.squeeze(0)?;

            // Apply repeat penalty
            let penalty_context: Vec<u32> = if params.repeat_last_n > 0 {
                let start = all_tokens.len().saturating_sub(params.repeat_last_n);
                all_tokens[start..].to_vec()
            } else {
                vec![]
            };
            let logits = sampling::apply_repeat_penalty(&logits, params.repeat_penalty, &penalty_context)?;

            let next_token = sampler.sample(&logits)?;
            all_tokens.push(next_token);
            generated_count += 1;

            // Check EOS
            if Some(next_token) == self.eos_token_id {
                stop_reason = "end_turn";
                break;
            }

            if let Some(text) = token_stream.next_token(next_token)? {
                full_text.push_str(&text);

                // Check stop sequences after each token
                let mut hit_stop = false;
                for seq in &params.stop_sequences {
                    if full_text.ends_with(seq) {
                        full_text.truncate(full_text.len() - seq.len());
                        hit_stop = true;
                        break;
                    }
                }
                if hit_stop {
                    on_token(&text);
                    stop_reason = "stop_sequence";
                    break;
                }

                on_token(&text);
            }
        }

        // Decode any remaining bytes
        if let Some(text) = token_stream.decode_rest()? {
            full_text.push_str(&text);
            on_token(&text);
        }

        tracing::debug!(
            prompt_tokens = prompt_token_count,
            completion_tokens = generated_count,
            "Generation complete"
        );

        Ok(GenerationResult {
            full_text,
            prompt_tokens: prompt_token_count,
            completion_tokens: generated_count,
            stop_reason: stop_reason.to_string(),
        })
    }

    fn reset(&mut self) {
        // Re-create model to clear KV cache
        // Note: candle's quantized_llama doesn't expose a cache clear method,
        // so we rely on the position index passed to forward() to manage state.
        // For truly stateless sessions, each session/prompt gets a fresh forward sequence.
        tracing::debug!("Model state reset requested");
    }
}
