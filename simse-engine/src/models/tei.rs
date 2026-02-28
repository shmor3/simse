//! TEI (Text Embeddings Inference) bridge.
//!
//! Implements the `Embedder` trait by proxying requests to an external
//! Hugging Face Text Embeddings Inference server via HTTP.

use anyhow::Result;
use serde::Serialize;
use ureq::Agent;

use super::{EmbedResult, Embedder};

/// Configuration for the TEI HTTP bridge.
#[derive(Debug, Clone)]
pub struct TeiConfig {
    /// Base URL of the TEI server (e.g., `http://localhost:8080`).
    pub base_url: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Whether to request normalized embeddings.
    pub normalize: bool,
    /// Whether to truncate inputs exceeding the model's max length.
    pub truncate: bool,
}

impl Default for TeiConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".to_string(),
            timeout_secs: 30,
            normalize: true,
            truncate: false,
        }
    }
}

/// Request body for the TEI `/embed` endpoint.
#[derive(Serialize)]
struct TeiEmbedRequest<'a> {
    inputs: &'a [String],
    normalize: bool,
    truncate: bool,
}

/// TEI embedder that proxies to an external TEI server.
pub struct TeiEmbedder {
    url: String,
    agent: Agent,
    config: TeiConfig,
}

impl TeiEmbedder {
    /// Create a new TEI embedder with the given configuration.
    pub fn new(config: TeiConfig) -> Self {
        let url = format!("{}/embed", config.base_url.trim_end_matches('/'));
        let agent_config = Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(config.timeout_secs)))
            .build();
        let agent = Agent::new_with_config(agent_config);
        Self { url, agent, config }
    }
}

impl Embedder for TeiEmbedder {
    fn embed(&self, texts: &[String]) -> Result<EmbedResult> {
        if texts.is_empty() {
            return Ok(EmbedResult {
                embeddings: vec![],
                prompt_tokens: 0,
            });
        }

        tracing::debug!(
            batch_size = texts.len(),
            url = %self.url,
            "Sending embedding request to TEI server"
        );

        let body = TeiEmbedRequest {
            inputs: texts,
            normalize: self.config.normalize,
            truncate: self.config.truncate,
        };

        let embeddings: Vec<Vec<f32>> = self
            .agent
            .post(&self.url)
            .send_json(&body)
            .map_err(|e| anyhow::anyhow!("TEI request failed: {}", e))?
            .body_mut()
            .read_json()
            .map_err(|e| anyhow::anyhow!("TEI response parse error: {}", e))?;

        // Estimate prompt tokens from input character count (rough approximation)
        let prompt_tokens: u64 = texts.iter().map(|t| (t.len() / 4) as u64).sum();

        tracing::debug!(
            batch_size = texts.len(),
            embedding_dim = embeddings.first().map_or(0, |e| e.len()),
            "TEI embeddings received"
        );

        Ok(EmbedResult {
            embeddings,
            prompt_tokens,
        })
    }
}
