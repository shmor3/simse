use anyhow::Result;

use crate::models::ModelRegistry;
use crate::protocol::*;

/// Run embedding inference and return an ACP response with embedding vectors.
///
/// Returns embeddings in a `data` content block matching the format expected
/// by the simse ACP client (`extractEmbeddings` in acp-results.ts).
pub fn run_embedding(
    registry: &ModelRegistry,
    model_id: &str,
    texts: &[String],
) -> Result<AcpSessionPromptResult> {
    let embedder = registry
        .get_embedder(model_id)
        .ok_or_else(|| anyhow::anyhow!("Embedding model not loaded: {}", model_id))?;

    let result = embedder.embed(texts)?;

    let usage = TokenUsage::new(result.prompt_tokens, 0);

    Ok(AcpSessionPromptResult {
        content: vec![AcpContentBlock::Data {
            data: serde_json::json!({ "embeddings": result.embeddings }),
            mime_type: Some("application/json".to_string()),
        }],
        stop_reason: "end_turn".to_string(),
        metadata: Some(serde_json::json!({
            "usage": serde_json::to_value(usage)?
        })),
    })
}
