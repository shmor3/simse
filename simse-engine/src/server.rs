use std::collections::{HashMap, HashSet};

use anyhow::Result;
use uuid::Uuid;

use crate::inference;
use crate::models::{ModelRegistry, SamplingParams};
use crate::protocol::*;
use crate::transport::NdjsonTransport;

/// Maximum allowed temperature value for sampling.
const MAX_TEMPERATURE: f64 = 10.0;

/// Maximum allowed `max_tokens` value for generation.
const MAX_TOKENS_LIMIT: u64 = 1_000_000;

// ── Conversation history ─────────────────────────────────────────────────

#[derive(Clone)]
struct HistoryMessage {
    role: String,
    content: String,
}

// ── Server configuration ──────────────────────────────────────────────────

/// Configuration for the ACP server instance.
pub struct ServerConfig {
    /// Server name reported in the ACP `initialize` response.
    pub server_name: String,
    /// Server version reported in the ACP `initialize` response.
    pub server_version: String,
    /// Default text generation model ID.
    pub default_model: String,
    /// Default embedding model ID.
    pub embedding_model: String,
    /// Optional TEI server URL for remote embeddings.
    pub tei_url: Option<String>,
    /// Whether to stream token-by-token via `session/update` notifications.
    pub streaming: bool,
    /// Default sampling parameters for generation requests.
    pub default_sampling: SamplingParams,
}

// ── ACP server ────────────────────────────────────────────────────────────

/// ACP-compatible inference server that handles JSON-RPC messages over stdin/stdout.
pub struct AcpServer {
    config: ServerConfig,
    registry: ModelRegistry,
    transport: NdjsonTransport,
    sessions: HashSet<String>,
    session_history: HashMap<String, Vec<HistoryMessage>>,
    current_model: String,
}

impl AcpServer {
    /// Create a new ACP server with the given configuration, model registry, and transport.
    pub fn new(config: ServerConfig, registry: ModelRegistry, transport: NdjsonTransport) -> Self {
        let current_model = config.default_model.clone();
        Self {
            config,
            registry,
            transport,
            sessions: HashSet::new(),
            session_history: HashMap::new(),
            current_model,
        }
    }

    /// Main loop: read messages from stdin, dispatch to handlers.
    pub fn run(&mut self) -> Result<()> {
        // Read all messages — this blocks on stdin until EOF
        let stdin = std::io::stdin();
        let reader = std::io::BufRead::lines(stdin.lock());

        for line_result in reader {
            let line = match line_result {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!("Failed to read stdin: {}", e);
                    break;
                }
            };

            let trimmed = line.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }

            let msg: JsonRpcIncoming = match serde_json::from_str(&trimmed) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("Parse error: {}", e);
                    // For parse errors, we don't have an id at all. Use 0 as fallback
                    // per JSON-RPC 2.0 spec (id MUST be null for parse errors).
                    self.transport.write_error(0, PARSE_ERROR, "Parse error: invalid JSON");
                    continue;
                }
            };

            self.handle_message(msg);
        }

        Ok(())
    }

    fn handle_message(&mut self, msg: JsonRpcIncoming) {
        let method = match msg.method {
            Some(m) => m,
            None => {
                // Response to something we sent (e.g., permission) — ignore
                return;
            }
        };

        // JSON-RPC notifications (no id) should not receive responses.
        let id = match msg.id {
            Some(id) => id,
            None => {
                // It's a notification — process but don't respond
                tracing::debug!(method = %method, "Received notification (no id)");
                return;
            }
        };

        match method.as_str() {
            "initialize" => self.handle_initialize(id),
            "session/new" => self.handle_session_new(id),
            "session/prompt" => {
                if let Err(e) = self.handle_session_prompt(id, msg.params) {
                    tracing::error!("session/prompt error: {}", e);
                    self.transport.write_error(id, INTERNAL_ERROR, e.to_string());
                }
            }
            "session/delete" => self.handle_session_delete(id, msg.params),
            "session/set_config_option" => self.handle_set_config(id, msg.params),
            _ => {
                self.transport.write_error(id, METHOD_NOT_FOUND, format!("Method not found: {}", method));
            }
        }
    }

    fn handle_initialize(&mut self, id: u64) {
        let result = AcpInitializeResult {
            protocol_version: 1,
            agent_info: AcpAgentInfo {
                name: self.config.server_name.clone(),
                version: self.config.server_version.clone(),
            },
            agent_capabilities: Some(serde_json::json!({})),
        };
        match serde_json::to_value(result) {
            Ok(v) => self.transport.write_response(id, v),
            Err(e) => self.transport.write_error(id, INTERNAL_ERROR, format!("Serialization error: {}", e)),
        }
    }

    fn handle_session_new(&mut self, id: u64) {
        let session_id = Uuid::new_v4().to_string();
        self.sessions.insert(session_id.clone());
        self.session_history.insert(session_id.clone(), Vec::new());

        let models = AcpModelsInfo {
            available_models: self.registry.available_models(),
            current_model_id: self.current_model.clone(),
        };

        let result = AcpSessionNewResult {
            session_id,
            models: Some(models),
            modes: Some(AcpModesInfo {
                current_mode_id: "default".to_string(),
                available_modes: vec![
                    AcpModeInfo {
                        id: "default".to_string(),
                        name: "Default".to_string(),
                        description: None,
                    },
                ],
            }),
        };

        match serde_json::to_value(result) {
            Ok(v) => self.transport.write_response(id, v),
            Err(e) => self.transport.write_error(id, INTERNAL_ERROR, format!("Serialization error: {}", e)),
        }
    }

    fn handle_session_prompt(&mut self, id: u64, params: Option<serde_json::Value>) -> Result<()> {
        let params_value = params.ok_or_else(|| anyhow::anyhow!("Missing params"))?;
        let prompt_params: AcpSessionPromptParams = serde_json::from_value(params_value)?;

        // Validate session
        if !self.sessions.contains(&prompt_params.session_id) {
            self.transport.write_error(id, INVALID_PARAMS, format!("Session not found: {}", prompt_params.session_id));
            return Ok(());
        }

        // Extract sampling params from metadata if present
        let sampling = self.extract_sampling_params(&prompt_params.metadata);

        // Check metadata-based embed detection first (preferred)
        let is_embed = prompt_params.metadata
            .as_ref()
            .and_then(|m| m.get("action"))
            .and_then(|a| a.as_str())
            == Some("embed");

        // Fall back to content-sniffing for backwards compatibility
        let is_embed = is_embed || Self::is_embed_request(&prompt_params.prompt);

        // Detect embed vs generate
        if is_embed {
            let texts = Self::extract_embed_texts(&prompt_params.prompt);
            // Check if prompt requests a specific embedding model via metadata
            let embed_model = prompt_params.metadata
                .as_ref()
                .and_then(|m| m.get("model"))
                .and_then(|m| m.as_str())
                .unwrap_or(&self.config.embedding_model);

            let result = inference::embedding::run_embedding(
                &self.registry,
                embed_model,
                &texts,
            )?;
            self.transport.write_response(id, serde_json::to_value(result)?);
        } else {
            // Extract prompt text
            let (user_prompt, system_prompt) = Self::extract_text_from_content(&prompt_params.prompt);

            // Build conversation context from history for multi-turn
            let history = self.session_history.get(&prompt_params.session_id).cloned().unwrap_or_default();
            let mut context_parts: Vec<String> = Vec::new();
            for msg in &history {
                context_parts.push(format!("{}: {}", msg.role, msg.content));
            }

            // Prepend history to the user prompt if there is conversation context
            let full_user_prompt = if context_parts.is_empty() {
                user_prompt.clone()
            } else {
                context_parts.push(format!("user: {}", user_prompt));
                context_parts.join("\n")
            };

            // Record user message in history
            if let Some(hist) = self.session_history.get_mut(&prompt_params.session_id) {
                hist.push(HistoryMessage {
                    role: "user".to_string(),
                    content: user_prompt.clone(),
                });
            }

            let result = inference::generation::run_generation(
                &mut self.registry,
                &self.current_model,
                &full_user_prompt,
                system_prompt.as_deref(),
                &sampling,
                &mut self.transport,
                &prompt_params.session_id,
                self.config.streaming,
            )?;

            // Record assistant response in history
            let response_text: String = result.content.iter().filter_map(|block| {
                match block {
                    AcpContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                }
            }).collect::<Vec<_>>().join("");

            if let Some(hist) = self.session_history.get_mut(&prompt_params.session_id) {
                hist.push(HistoryMessage {
                    role: "assistant".to_string(),
                    content: response_text,
                });
            }

            self.transport.write_response(id, serde_json::to_value(result)?);
        }

        Ok(())
    }

    fn handle_session_delete(&mut self, id: u64, params: Option<serde_json::Value>) {
        if let Some(params) = params {
            if let Some(session_id) = params.get("sessionId").and_then(|v| v.as_str()) {
                self.sessions.remove(session_id);
                self.session_history.remove(session_id);
                tracing::info!(session_id, "Session deleted");
                self.transport.write_response(id, serde_json::json!({}));
                return;
            }
        }
        self.transport.write_error(id, INVALID_PARAMS, "Missing or invalid sessionId");
    }

    fn handle_set_config(&mut self, id: u64, params: Option<serde_json::Value>) {
        if let Some(params) = params {
            if let Ok(config_params) = serde_json::from_value::<AcpSetConfigParams>(params) {
                match config_params.config_option_id.as_str() {
                    "model" => {
                        tracing::info!(model = %config_params.group_id, "Switching model");
                        self.current_model = config_params.group_id;
                        self.transport.write_response(id, serde_json::json!({}));
                        return;
                    }
                    "mode" => {
                        // Acknowledge mode changes but don't act on them
                        self.transport.write_response(id, serde_json::json!({}));
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.transport.write_error(id, INVALID_PARAMS, "Unsupported config option");
    }

    // ── Content helpers ───────────────────────────────────────────────────

    /// Extract user prompt and optional system prompt from content blocks.
    /// If there are multiple text blocks, the first is the system prompt and the last is the user prompt.
    fn extract_text_from_content(content: &[AcpContentBlock]) -> (String, Option<String>) {
        let text_blocks: Vec<&str> = content
            .iter()
            .filter_map(|block| match block {
                AcpContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        match text_blocks.len() {
            0 => (String::new(), None),
            1 => (text_blocks[0].to_string(), None),
            _ => {
                let system = text_blocks[0].to_string();
                let user = text_blocks[text_blocks.len() - 1].to_string();
                (user, Some(system))
            }
        }
    }

    /// Check if the prompt is an embedding request.
    /// Looks for `{"texts":[...],"action":"embed"}` in text blocks or data blocks.
    fn is_embed_request(content: &[AcpContentBlock]) -> bool {
        for block in content {
            match block {
                AcpContentBlock::Text { text } => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
                        if v.get("action").and_then(|a| a.as_str()) == Some("embed") {
                            return true;
                        }
                    }
                }
                AcpContentBlock::Data { data, .. } => {
                    if data.get("action").and_then(|a| a.as_str()) == Some("embed") {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Extract texts from an embed request with batch size limiting.
    fn extract_embed_texts(content: &[AcpContentBlock]) -> Vec<String> {
        let mut texts = Vec::new();
        for block in content {
            let value = match block {
                AcpContentBlock::Text { text } => serde_json::from_str::<serde_json::Value>(text).ok(),
                AcpContentBlock::Data { data, .. } => Some(data.clone()),
                _ => None,
            };

            if let Some(v) = value {
                if let Some(text_array) = v.get("texts").and_then(|t| t.as_array()) {
                    texts = text_array
                        .iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect();
                    break;
                }
            }
        }

        const MAX_BATCH_SIZE: usize = 256;
        if texts.len() > MAX_BATCH_SIZE {
            tracing::warn!(count = texts.len(), max = MAX_BATCH_SIZE, "Truncating embed batch");
            texts.truncate(MAX_BATCH_SIZE);
        }

        texts
    }

    /// Extract sampling params from session/prompt metadata with bounds validation.
    pub(crate) fn extract_sampling_params(&self, metadata: &Option<serde_json::Value>) -> SamplingParams {
        let mut params = self.config.default_sampling.clone();

        if let Some(meta) = metadata {
            if let Some(temp) = meta.get("temperature").and_then(|v| v.as_f64()) {
                if temp.is_finite() && (0.0..=MAX_TEMPERATURE).contains(&temp) {
                    params.temperature = temp;
                } else {
                    tracing::warn!(temperature = temp, "Invalid temperature, using default");
                }
            }
            if let Some(max) = meta.get("max_tokens").and_then(|v| v.as_u64()) {
                if max > 0 && max <= MAX_TOKENS_LIMIT {
                    params.max_tokens = max as usize;
                } else {
                    tracing::warn!(max_tokens = max, "Invalid max_tokens, using default");
                }
            }
            if let Some(top_p) = meta.get("top_p").and_then(|v| v.as_f64()) {
                if top_p.is_finite() && (0.0..=1.0).contains(&top_p) {
                    params.top_p = Some(top_p);
                } else {
                    tracing::warn!(top_p = top_p, "Invalid top_p, using default");
                }
            }
            if let Some(top_k) = meta.get("top_k").and_then(|v| v.as_u64()) {
                if top_k > 0 {
                    params.top_k = Some(top_k as usize);
                } else {
                    tracing::warn!(top_k = top_k, "Invalid top_k, using default");
                }
            }
            if let Some(stops) = meta.get("stop_sequences").and_then(|v| v.as_array()) {
                params.stop_sequences = stops
                    .iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect();
            }
        }

        params
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to build an AcpServer for testing extract_sampling_params.
    fn test_server() -> AcpServer {
        let config = ServerConfig {
            server_name: "test".to_string(),
            server_version: "0.0.1".to_string(),
            default_model: "test-model".to_string(),
            embedding_model: "test-embed".to_string(),
            tei_url: None,
            streaming: false,
            default_sampling: SamplingParams::default(),
        };
        let registry = ModelRegistry::new(candle_core::Device::Cpu);
        let transport = NdjsonTransport::new();
        AcpServer::new(config, registry, transport)
    }

    // ── is_embed_request ─────────────────────────────────────────────────

    #[test]
    fn is_embed_request_with_embed_json_in_text_block() {
        let content = vec![AcpContentBlock::Text {
            text: r#"{"texts":["hello"],"action":"embed"}"#.to_string(),
        }];
        assert!(AcpServer::is_embed_request(&content));
    }

    #[test]
    fn is_embed_request_with_normal_text() {
        let content = vec![AcpContentBlock::Text {
            text: "Hello, how are you?".to_string(),
        }];
        assert!(!AcpServer::is_embed_request(&content));
    }

    #[test]
    fn is_embed_request_with_empty_content() {
        let content: Vec<AcpContentBlock> = vec![];
        assert!(!AcpServer::is_embed_request(&content));
    }

    #[test]
    fn is_embed_request_with_data_block() {
        let content = vec![AcpContentBlock::Data {
            data: serde_json::json!({"texts": ["foo"], "action": "embed"}),
            mime_type: None,
        }];
        assert!(AcpServer::is_embed_request(&content));
    }

    #[test]
    fn is_embed_request_with_data_block_no_embed_action() {
        let content = vec![AcpContentBlock::Data {
            data: serde_json::json!({"texts": ["foo"], "action": "generate"}),
            mime_type: None,
        }];
        assert!(!AcpServer::is_embed_request(&content));
    }

    #[test]
    fn is_embed_request_json_without_action_field() {
        let content = vec![AcpContentBlock::Text {
            text: r#"{"texts":["hello"]}"#.to_string(),
        }];
        assert!(!AcpServer::is_embed_request(&content));
    }

    // ── extract_embed_texts ──────────────────────────────────────────────

    #[test]
    fn extract_embed_texts_from_text_block() {
        let content = vec![AcpContentBlock::Text {
            text: r#"{"texts":["hello","world"],"action":"embed"}"#.to_string(),
        }];
        let texts = AcpServer::extract_embed_texts(&content);
        assert_eq!(texts, vec!["hello", "world"]);
    }

    #[test]
    fn extract_embed_texts_from_data_block() {
        let content = vec![AcpContentBlock::Data {
            data: serde_json::json!({"texts": ["alpha", "beta", "gamma"], "action": "embed"}),
            mime_type: None,
        }];
        let texts = AcpServer::extract_embed_texts(&content);
        assert_eq!(texts, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn extract_embed_texts_empty_content() {
        let content: Vec<AcpContentBlock> = vec![];
        let texts = AcpServer::extract_embed_texts(&content);
        assert!(texts.is_empty());
    }

    #[test]
    fn extract_embed_texts_no_texts_field() {
        let content = vec![AcpContentBlock::Text {
            text: r#"{"action":"embed"}"#.to_string(),
        }];
        let texts = AcpServer::extract_embed_texts(&content);
        assert!(texts.is_empty());
    }

    #[test]
    fn extract_embed_texts_non_string_texts_filtered() {
        let content = vec![AcpContentBlock::Text {
            text: r#"{"texts":["valid", 42, null, "also_valid"],"action":"embed"}"#.to_string(),
        }];
        let texts = AcpServer::extract_embed_texts(&content);
        assert_eq!(texts, vec!["valid", "also_valid"]);
    }

    // ── extract_text_from_content ────────────────────────────────────────

    #[test]
    fn extract_text_empty_content() {
        let content: Vec<AcpContentBlock> = vec![];
        let (user, system) = AcpServer::extract_text_from_content(&content);
        assert!(user.is_empty());
        assert!(system.is_none());
    }

    #[test]
    fn extract_text_single_block() {
        let content = vec![AcpContentBlock::Text {
            text: "Hello world".to_string(),
        }];
        let (user, system) = AcpServer::extract_text_from_content(&content);
        assert_eq!(user, "Hello world");
        assert!(system.is_none());
    }

    #[test]
    fn extract_text_two_blocks_system_and_user() {
        let content = vec![
            AcpContentBlock::Text { text: "You are a helpful assistant.".to_string() },
            AcpContentBlock::Text { text: "What is Rust?".to_string() },
        ];
        let (user, system) = AcpServer::extract_text_from_content(&content);
        assert_eq!(user, "What is Rust?");
        assert_eq!(system, Some("You are a helpful assistant.".to_string()));
    }

    #[test]
    fn extract_text_ignores_non_text_blocks() {
        let content = vec![
            AcpContentBlock::Data {
                data: serde_json::json!({"key": "value"}),
                mime_type: None,
            },
            AcpContentBlock::Text { text: "Only text block".to_string() },
        ];
        let (user, system) = AcpServer::extract_text_from_content(&content);
        assert_eq!(user, "Only text block");
        assert!(system.is_none());
    }

    #[test]
    fn extract_text_multiple_blocks_first_system_last_user() {
        let content = vec![
            AcpContentBlock::Text { text: "system".to_string() },
            AcpContentBlock::Text { text: "middle".to_string() },
            AcpContentBlock::Text { text: "user".to_string() },
        ];
        let (user, system) = AcpServer::extract_text_from_content(&content);
        assert_eq!(user, "user");
        assert_eq!(system, Some("system".to_string()));
    }

    // ── extract_sampling_params ──────────────────────────────────────────

    #[test]
    fn extract_sampling_params_none_metadata() {
        let server = test_server();
        let params = server.extract_sampling_params(&None);
        // Should return defaults
        assert!((params.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(params.max_tokens, 2048);
        assert!(params.top_p.is_none());
        assert!(params.top_k.is_none());
        assert!(params.stop_sequences.is_empty());
    }

    #[test]
    fn extract_sampling_params_valid_values() {
        let server = test_server();
        let meta = Some(serde_json::json!({
            "temperature": 1.5,
            "max_tokens": 4096,
            "top_p": 0.9,
            "top_k": 50,
            "stop_sequences": ["</s>", "\n"]
        }));
        let params = server.extract_sampling_params(&meta);
        assert!((params.temperature - 1.5).abs() < f64::EPSILON);
        assert_eq!(params.max_tokens, 4096);
        assert_eq!(params.top_p, Some(0.9));
        assert_eq!(params.top_k, Some(50));
        assert_eq!(params.stop_sequences, vec!["</s>", "\n"]);
    }

    #[test]
    fn extract_sampling_params_invalid_temperature_uses_default() {
        let server = test_server();
        // Temperature > MAX_TEMPERATURE (10.0) should be rejected
        let meta = Some(serde_json::json!({ "temperature": 999.0 }));
        let params = server.extract_sampling_params(&meta);
        assert!((params.temperature - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn extract_sampling_params_negative_temperature_uses_default() {
        let server = test_server();
        let meta = Some(serde_json::json!({ "temperature": -1.0 }));
        let params = server.extract_sampling_params(&meta);
        assert!((params.temperature - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn extract_sampling_params_zero_temperature_valid() {
        let server = test_server();
        let meta = Some(serde_json::json!({ "temperature": 0.0 }));
        let params = server.extract_sampling_params(&meta);
        assert!((params.temperature).abs() < f64::EPSILON);
    }

    #[test]
    fn extract_sampling_params_zero_max_tokens_uses_default() {
        let server = test_server();
        let meta = Some(serde_json::json!({ "max_tokens": 0 }));
        let params = server.extract_sampling_params(&meta);
        assert_eq!(params.max_tokens, 2048);
    }

    #[test]
    fn extract_sampling_params_exceeding_max_tokens_uses_default() {
        let server = test_server();
        let meta = Some(serde_json::json!({ "max_tokens": 2_000_000 }));
        let params = server.extract_sampling_params(&meta);
        assert_eq!(params.max_tokens, 2048);
    }

    #[test]
    fn extract_sampling_params_invalid_top_p_uses_default() {
        let server = test_server();
        let meta = Some(serde_json::json!({ "top_p": 1.5 }));
        let params = server.extract_sampling_params(&meta);
        assert!(params.top_p.is_none());
    }

    #[test]
    fn extract_sampling_params_zero_top_k_uses_default() {
        let server = test_server();
        let meta = Some(serde_json::json!({ "top_k": 0 }));
        let params = server.extract_sampling_params(&meta);
        assert!(params.top_k.is_none());
    }

    #[test]
    fn extract_sampling_params_nan_temperature_uses_default() {
        let server = test_server();
        let meta = Some(serde_json::json!({ "temperature": f64::NAN }));
        let params = server.extract_sampling_params(&meta);
        // NaN is not finite, so it should be rejected
        assert!((params.temperature - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn extract_sampling_params_empty_metadata_object() {
        let server = test_server();
        let meta = Some(serde_json::json!({}));
        let params = server.extract_sampling_params(&meta);
        assert!((params.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(params.max_tokens, 2048);
    }

    #[test]
    fn extract_sampling_params_boundary_temperature() {
        let server = test_server();
        // Exactly MAX_TEMPERATURE (10.0) should be accepted
        let meta = Some(serde_json::json!({ "temperature": 10.0 }));
        let params = server.extract_sampling_params(&meta);
        assert!((params.temperature - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn extract_sampling_params_boundary_top_p() {
        let server = test_server();
        // top_p = 0.0 and 1.0 should both be accepted
        let meta_zero = Some(serde_json::json!({ "top_p": 0.0 }));
        let params = server.extract_sampling_params(&meta_zero);
        assert_eq!(params.top_p, Some(0.0));

        let meta_one = Some(serde_json::json!({ "top_p": 1.0 }));
        let params = server.extract_sampling_params(&meta_one);
        assert_eq!(params.top_p, Some(1.0));
    }
}
