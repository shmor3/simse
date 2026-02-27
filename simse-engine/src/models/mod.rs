pub mod bert;
pub mod llama;
pub mod sampling;
pub mod tokenizer;
pub mod weights;

use std::collections::HashMap;

use anyhow::Result;

use crate::protocol::AcpModelInfo;

// ── Traits ────────────────────────────────────────────────────────────────

/// Trait for text generation models.
pub trait TextGenerator: Send {
    /// Generate text from a prompt, calling `on_token` for each decoded token chunk.
    fn generate(
        &mut self,
        prompt: &str,
        system: Option<&str>,
        params: &SamplingParams,
        on_token: &mut dyn FnMut(&str),
    ) -> Result<GenerationResult>;

    /// Reset internal state (KV cache) between requests.
    fn reset(&mut self);
}

/// Trait for embedding models.
pub trait Embedder: Send {
    /// Generate embeddings for a batch of texts.
    fn embed(&self, texts: &[String]) -> Result<EmbedResult>;
}

// ── Result types ──────────────────────────────────────────────────────────

pub struct GenerationResult {
    pub full_text: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

pub struct EmbedResult {
    pub embeddings: Vec<Vec<f32>>,
    pub prompt_tokens: u64,
}

// ── Sampling parameters ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SamplingParams {
    pub temperature: f64,
    pub top_p: Option<f64>,
    pub top_k: Option<usize>,
    pub max_tokens: usize,
    pub repeat_penalty: f32,
    pub repeat_last_n: usize,
    pub stop_sequences: Vec<String>,
}

impl Default for SamplingParams {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: None,
            top_k: None,
            max_tokens: 2048,
            repeat_penalty: 1.1,
            repeat_last_n: 64,
            stop_sequences: vec![],
        }
    }
}

// ── Model configuration ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ModelConfig {
    /// Specific filename within a HuggingFace repo (e.g., "model-Q4_K_M.gguf")
    pub filename: Option<String>,
    /// HuggingFace revision/branch
    pub revision: Option<String>,
}

// ── Model registry ───────────────────────────────────────────────────────

/// Registry managing loaded text generation and embedding models.
pub struct ModelRegistry {
    generators: HashMap<String, Box<dyn TextGenerator>>,
    embedders: HashMap<String, Box<dyn Embedder>>,
    device: candle_core::Device,
}

impl ModelRegistry {
    pub fn new(device: candle_core::Device) -> Self {
        Self {
            generators: HashMap::new(),
            embedders: HashMap::new(),
            device,
        }
    }

    pub fn device(&self) -> &candle_core::Device {
        &self.device
    }

    /// Load a text generation model (GGUF quantized).
    pub fn load_generator(&mut self, model_id: &str, config: &ModelConfig) -> Result<()> {
        tracing::info!(model_id, "Loading text generation model");
        let generator = llama::LlamaGenerator::from_hub(model_id, config, &self.device)?;
        self.generators.insert(model_id.to_string(), Box::new(generator));
        Ok(())
    }

    /// Load an embedding model.
    pub fn load_embedder(&mut self, model_id: &str, _config: &ModelConfig) -> Result<()> {
        tracing::info!(model_id, "Loading embedding model");
        let embedder = bert::BertEmbedder::from_hub(model_id, &self.device)?;
        self.embedders.insert(model_id.to_string(), Box::new(embedder));
        Ok(())
    }

    /// Get a mutable reference to a text generator.
    pub fn get_generator(&mut self, model_id: &str) -> Option<&mut dyn TextGenerator> {
        self.generators.get_mut(model_id).map(move |g| &mut **g as &mut dyn TextGenerator)
    }

    /// Get a reference to an embedder.
    pub fn get_embedder(&self, model_id: &str) -> Option<&dyn Embedder> {
        self.embedders.get(model_id).map(|e| &**e as &dyn Embedder)
    }

    /// List all available models for the ACP session/new response.
    pub fn available_models(&self) -> Vec<AcpModelInfo> {
        let mut models: Vec<AcpModelInfo> = self
            .generators
            .keys()
            .map(|id| AcpModelInfo {
                model_id: id.clone(),
                name: id.clone(),
                description: Some("Text generation model".to_string()),
            })
            .collect();

        models.extend(self.embedders.keys().map(|id| AcpModelInfo {
            model_id: id.clone(),
            name: id.clone(),
            description: Some("Embedding model".to_string()),
        }));

        models
    }

    /// Get the first generator model ID (used as default).
    pub fn default_generator_id(&self) -> Option<String> {
        self.generators.keys().next().cloned()
    }
}
