use std::collections::HashSet;

use anyhow::Result;
use uuid::Uuid;

use crate::inference;
use crate::models::{ModelRegistry, SamplingParams};
use crate::protocol::*;
use crate::transport::NdjsonTransport;

// ── Server configuration ──────────────────────────────────────────────────

pub struct ServerConfig {
    pub server_name: String,
    pub server_version: String,
    pub default_model: String,
    pub embedding_model: String,
    pub tei_url: Option<String>,
    pub streaming: bool,
    pub default_sampling: SamplingParams,
}

// ── ACP server ────────────────────────────────────────────────────────────

pub struct AcpServer {
    config: ServerConfig,
    registry: ModelRegistry,
    transport: NdjsonTransport,
    sessions: HashSet<String>,
    current_model: String,
}

impl AcpServer {
    pub fn new(config: ServerConfig, registry: ModelRegistry, transport: NdjsonTransport) -> Self {
        let current_model = config.default_model.clone();
        Self {
            config,
            registry,
            transport,
            sessions: HashSet::new(),
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
                    self.transport.write_error(0, PARSE_ERROR, "Parse error: invalid JSON");
                    continue;
                }
            };

            self.handle_message(msg);
        }

        Ok(())
    }

    fn handle_message(&mut self, msg: JsonRpcIncoming) {
        let id = msg.id.unwrap_or(0);
        let method = match msg.method {
            Some(m) => m,
            None => {
                // Response to something we sent (e.g., permission) — ignore
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
        self.transport.write_response(id, serde_json::to_value(result).unwrap());
    }

    fn handle_session_new(&mut self, id: u64) {
        let session_id = Uuid::new_v4().to_string();
        self.sessions.insert(session_id.clone());

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

        self.transport.write_response(id, serde_json::to_value(result).unwrap());
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

        // Detect embed vs generate
        if Self::is_embed_request(&prompt_params.prompt) {
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

            let result = inference::generation::run_generation(
                &mut self.registry,
                &self.current_model,
                &user_prompt,
                system_prompt.as_deref(),
                &sampling,
                &mut self.transport,
                &prompt_params.session_id,
                self.config.streaming,
            )?;
            self.transport.write_response(id, serde_json::to_value(result)?);
        }

        Ok(())
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

    /// Extract texts from an embed request.
    fn extract_embed_texts(content: &[AcpContentBlock]) -> Vec<String> {
        for block in content {
            let value = match block {
                AcpContentBlock::Text { text } => serde_json::from_str::<serde_json::Value>(text).ok(),
                AcpContentBlock::Data { data, .. } => Some(data.clone()),
                _ => None,
            };

            if let Some(v) = value {
                if let Some(texts) = v.get("texts").and_then(|t| t.as_array()) {
                    return texts
                        .iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect();
                }
            }
        }
        vec![]
    }

    /// Extract sampling params from session/prompt metadata.
    fn extract_sampling_params(&self, metadata: &Option<serde_json::Value>) -> SamplingParams {
        let mut params = self.config.default_sampling.clone();

        if let Some(meta) = metadata {
            if let Some(temp) = meta.get("temperature").and_then(|v| v.as_f64()) {
                params.temperature = temp;
            }
            if let Some(max) = meta.get("max_tokens").and_then(|v| v.as_u64()) {
                params.max_tokens = max as usize;
            }
            if let Some(top_p) = meta.get("top_p").and_then(|v| v.as_f64()) {
                params.top_p = Some(top_p);
            }
            if let Some(top_k) = meta.get("top_k").and_then(|v| v.as_u64()) {
                params.top_k = Some(top_k as usize);
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
