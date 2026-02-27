use anyhow::Result;

use crate::models::{ModelRegistry, SamplingParams};
use crate::protocol::*;
use crate::transport::NdjsonTransport;

/// Run text generation with streaming notifications.
///
/// Sends `session/update` notifications for each decoded token chunk,
/// then returns the final `AcpSessionPromptResult` with full text + usage.
pub fn run_generation(
    registry: &mut ModelRegistry,
    model_id: &str,
    prompt: &str,
    system: Option<&str>,
    params: &SamplingParams,
    transport: &mut NdjsonTransport,
    session_id: &str,
    streaming: bool,
) -> Result<AcpSessionPromptResult> {
    let generator = registry
        .get_generator(model_id)
        .ok_or_else(|| anyhow::anyhow!("Model not loaded: {}", model_id))?;

    // Reset model state for a fresh generation
    generator.reset();

    let session_id_owned = session_id.to_string();

    let result = if streaming {
        generator.generate(prompt, system, params, &mut |chunk: &str| {
            let update = AcpSessionUpdateParams {
                session_id: session_id_owned.clone(),
                update: AcpSessionUpdate {
                    session_update: "agent_message_chunk".to_string(),
                    content: Some(vec![AcpContentBlock::Text {
                        text: chunk.to_string(),
                    }]),
                },
            };
            if let Ok(params_value) = serde_json::to_value(&update) {
                transport.write_notification("session/update", params_value);
            }
        })?
    } else {
        // Non-streaming: generate silently, return full result
        generator.generate(prompt, system, params, &mut |_| {})?
    };

    let usage = TokenUsage::new(result.prompt_tokens, result.completion_tokens);

    Ok(AcpSessionPromptResult {
        content: vec![AcpContentBlock::Text {
            text: result.full_text,
        }],
        stop_reason: "end_turn".to_string(),
        metadata: Some(serde_json::json!({
            "usage": serde_json::to_value(usage)?
        })),
    })
}
