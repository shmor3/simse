pub mod bert;
pub mod llama;
pub mod nomic_bert;
pub mod sampling;
pub mod tei;
pub mod tokenizer;
pub mod weights;

use std::collections::HashMap;

use anyhow::Result;

use crate::protocol::AcpModelInfo;

use self::tei::{TeiConfig, TeiEmbedder};

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

/// Result of a text generation request.
pub struct GenerationResult {
    /// The full generated text.
    pub full_text: String,
    /// Number of tokens in the input prompt.
    pub prompt_tokens: u64,
    /// Number of tokens generated.
    pub completion_tokens: u64,
    /// Why generation stopped (e.g. "end_turn", "max_tokens", "stop_sequence", "timeout").
    pub stop_reason: String,
}

/// Result of an embedding request.
pub struct EmbedResult {
    /// One embedding vector per input text.
    pub embeddings: Vec<Vec<f32>>,
    /// Total tokens consumed across all inputs.
    pub prompt_tokens: u64,
}

// ── Sampling parameters ───────────────────────────────────────────────────

/// Parameters controlling token sampling during generation.
#[derive(Debug, Clone)]
pub struct SamplingParams {
    /// Softmax temperature (0.0 = greedy, higher = more random).
    pub temperature: f64,
    /// Nucleus sampling threshold (0.0..1.0).
    pub top_p: Option<f64>,
    /// Top-k sampling: only consider the k most likely tokens.
    pub top_k: Option<usize>,
    /// Maximum number of tokens to generate.
    pub max_tokens: usize,
    /// Penalty applied to repeated tokens (1.0 = no penalty).
    pub repeat_penalty: f32,
    /// Number of recent tokens to consider for repeat penalty.
    pub repeat_last_n: usize,
    /// Stop generation when any of these sequences is emitted.
    pub stop_sequences: Vec<String>,
    /// Wall-clock timeout for generation in seconds (default: 300 = 5 minutes).
    pub generation_timeout_secs: u64,
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
            generation_timeout_secs: 300,
        }
    }
}

// ── Model configuration ──────────────────────────────────────────────────

/// Configuration for locating a model on HuggingFace Hub.
#[derive(Debug, Clone, Default)]
pub struct ModelConfig {
    /// Specific filename within a HuggingFace repo (e.g., "model-Q4_K_M.gguf").
    pub filename: Option<String>,
    /// HuggingFace revision/branch.
    pub revision: Option<String>,
    /// Explicit tokenizer source (HF repo ID or local path).
    pub tokenizer: Option<String>,
}

// ── Model registry ───────────────────────────────────────────────────────

/// Registry managing loaded text generation and embedding models.
pub struct ModelRegistry {
    generators: HashMap<String, Box<dyn TextGenerator>>,
    embedders: HashMap<String, Box<dyn Embedder>>,
    device: candle_core::Device,
}

impl ModelRegistry {
    /// Create a new empty registry on the given compute device.
    pub fn new(device: candle_core::Device) -> Self {
        Self {
            generators: HashMap::new(),
            embedders: HashMap::new(),
            device,
        }
    }

    /// Return a reference to the compute device used by this registry.
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

    /// Load a TEI bridge embedder.
    pub fn load_tei_embedder(&mut self, key: &str, config: TeiConfig) -> Result<()> {
        tracing::info!(key, url = %config.base_url, "Loading TEI bridge embedder");
        let embedder = TeiEmbedder::new(config);
        self.embedders.insert(key.to_string(), Box::new(embedder));
        Ok(())
    }

    /// Get a mutable reference to a text generator.
    pub fn get_generator(&mut self, model_id: &str) -> Option<&mut dyn TextGenerator> {
        self.generators.get_mut(model_id).map(move |g| &mut **g as &mut dyn TextGenerator)
    }

    /// Get a reference to an embedder.
    /// Supports `tei://` prefix to select TEI embedder (maps to `tei://default` key).
    pub fn get_embedder(&self, model_id: &str) -> Option<&dyn Embedder> {
        if model_id.starts_with("tei://") {
            let key = if model_id == "tei://" { "tei://default" } else { model_id };
            return self.embedders.get(key).map(|e| &**e as &dyn Embedder);
        }
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

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ModelRegistry::new() ─────────────────────────────────────────────

    #[test]
    fn new_registry_is_empty() {
        let registry = ModelRegistry::new(candle_core::Device::Cpu);
        assert!(registry.available_models().is_empty());
    }

    #[test]
    fn new_registry_has_no_default_generator() {
        let registry = ModelRegistry::new(candle_core::Device::Cpu);
        assert!(registry.default_generator_id().is_none());
    }

    // ── get_embedder() ──────────────────────────────────────────────────

    #[test]
    fn get_embedder_unknown_model_returns_none() {
        let registry = ModelRegistry::new(candle_core::Device::Cpu);
        assert!(registry.get_embedder("nonexistent-model").is_none());
    }

    #[test]
    fn get_embedder_tei_prefix_returns_none_when_not_loaded() {
        let registry = ModelRegistry::new(candle_core::Device::Cpu);
        assert!(registry.get_embedder("tei://some-model").is_none());
    }

    #[test]
    fn get_embedder_bare_tei_prefix_maps_to_default() {
        let mut registry = ModelRegistry::new(candle_core::Device::Cpu);
        // Load a TEI embedder with the default key
        let config = TeiConfig {
            base_url: "http://localhost:8080".to_string(),
            ..Default::default()
        };
        registry.load_tei_embedder("tei://default", config).unwrap();

        // "tei://" should map to "tei://default"
        assert!(registry.get_embedder("tei://").is_some());
    }

    #[test]
    fn get_embedder_tei_prefix_with_name() {
        let mut registry = ModelRegistry::new(candle_core::Device::Cpu);
        let config = TeiConfig {
            base_url: "http://localhost:9090".to_string(),
            ..Default::default()
        };
        registry.load_tei_embedder("tei://my-model", config).unwrap();

        assert!(registry.get_embedder("tei://my-model").is_some());
        assert!(registry.get_embedder("tei://other-model").is_none());
    }

    // ── available_models() ──────────────────────────────────────────────

    #[test]
    fn available_models_empty_for_fresh_registry() {
        let registry = ModelRegistry::new(candle_core::Device::Cpu);
        assert!(registry.available_models().is_empty());
    }

    #[test]
    fn available_models_includes_tei_embedders() {
        let mut registry = ModelRegistry::new(candle_core::Device::Cpu);
        let config = TeiConfig::default();
        registry.load_tei_embedder("tei://test", config).unwrap();

        let models = registry.available_models();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model_id, "tei://test");
        assert_eq!(models[0].description, Some("Embedding model".to_string()));
    }

    // ── load_tei_embedder + get_embedder roundtrip ──────────────────────

    #[test]
    fn load_tei_embedder_and_retrieve() {
        let mut registry = ModelRegistry::new(candle_core::Device::Cpu);
        let config = TeiConfig {
            base_url: "http://gpu-server:3000".to_string(),
            timeout_secs: 60,
            normalize: true,
            truncate: false,
        };
        let result = registry.load_tei_embedder("tei://roundtrip", config);
        assert!(result.is_ok());

        // Should be retrievable by exact key
        assert!(registry.get_embedder("tei://roundtrip").is_some());

        // Should appear in available models
        let models = registry.available_models();
        assert!(models.iter().any(|m| m.model_id == "tei://roundtrip"));
    }

    #[test]
    fn load_multiple_tei_embedders() {
        let mut registry = ModelRegistry::new(candle_core::Device::Cpu);

        registry.load_tei_embedder("tei://a", TeiConfig::default()).unwrap();
        registry.load_tei_embedder("tei://b", TeiConfig::default()).unwrap();

        assert!(registry.get_embedder("tei://a").is_some());
        assert!(registry.get_embedder("tei://b").is_some());
        assert_eq!(registry.available_models().len(), 2);
    }

    // ── device() ─────────────────────────────────────────────────────────

    #[test]
    fn registry_device_is_cpu() {
        let registry = ModelRegistry::new(candle_core::Device::Cpu);
        assert!(matches!(registry.device(), candle_core::Device::Cpu));
    }
}
