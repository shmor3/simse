use serde::Deserialize;
use tokio::io::AsyncBufReadExt;

use crate::config::AppConfig;
use crate::context::CoreContext;
use crate::rpc_protocol::*;
use crate::rpc_transport::NdjsonTransport;
use crate::server::session::{SessionInfo, SessionStatus};

pub struct CoreRpcServer {
	transport: NdjsonTransport,
	context: Option<CoreContext>,
}

impl CoreRpcServer {
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			context: None,
		}
	}

	pub async fn run(&mut self) -> Result<(), crate::error::SimseError> {
		let stdin = tokio::io::stdin();
		let reader = tokio::io::BufReader::new(stdin);
		let mut lines = reader.lines();

		while let Ok(Some(line)) = lines.next_line().await {
			let line = line.trim().to_string();
			if line.is_empty() {
				continue;
			}
			match serde_json::from_str::<JsonRpcRequest>(&line) {
				Ok(request) => self.dispatch(request).await,
				Err(e) => tracing::warn!("Invalid JSON-RPC request: {}", e),
			}
		}
		Ok(())
	}

	/// Dispatch a JSON-RPC request to the appropriate handler.
	///
	/// Public so that integration tests can call individual methods without
	/// going through the stdin transport.
	pub async fn dispatch(&mut self, req: JsonRpcRequest) {
		match req.method.as_str() {
			// -- Lifecycle -----------------------------------------------
			"core/initialize" => self.handle_initialize(req).await,
			"core/dispose" => self.handle_dispose(req).await,
			"core/health" => self.handle_health(req).await,

			// -- Session management --------------------------------------
			"session/create" => self.handle_session_create(req).await,
			"session/get" => self.handle_session_get(req).await,
			"session/list" => self.handle_session_list(req).await,
			"session/delete" => self.handle_session_delete(req).await,
			"session/updateStatus" => self.handle_session_update_status(req).await,
			"session/fork" => self.handle_session_fork(req).await,

			// -- Conversation --------------------------------------------
			"conversation/addUser" => self.handle_conv_add_user(req).await,
			"conversation/addAssistant" => self.handle_conv_add_assistant(req).await,
			"conversation/addToolResult" => self.handle_conv_add_tool_result(req).await,
			"conversation/setSystemPrompt" => self.handle_conv_set_system_prompt(req).await,
			"conversation/getMessages" => self.handle_conv_get_messages(req).await,
			"conversation/compact" => self.handle_conv_compact(req).await,
			"conversation/clear" => self.handle_conv_clear(req).await,
			"conversation/stats" => self.handle_conv_stats(req).await,
			"conversation/toJson" => self.handle_conv_to_json(req).await,
			"conversation/fromJson" => self.handle_conv_from_json(req).await,

			// -- Unknown -------------------------------------------------
			_ => self.transport.write_error(
				req.id,
				METHOD_NOT_FOUND,
				format!("Unknown method: {}", req.method),
				None,
			),
		}
	}

	fn require_context(&self) -> Option<&CoreContext> {
		self.context.as_ref()
	}

	#[allow(dead_code)]
	fn require_context_mut(&mut self) -> Option<&mut CoreContext> {
		self.context.as_mut()
	}

	fn write_not_initialized(&self, req_id: u64) {
		self.transport.write_error(
			req_id,
			CORE_ERROR,
			"Not initialized. Call core/initialize first.",
			Some(serde_json::json!({ "coreCode": "NOT_INITIALIZED" })),
		);
	}

	fn write_session_not_found(&self, req_id: u64, session_id: &str) {
		self.transport.write_error(
			req_id,
			CORE_ERROR,
			format!("Session not found: {}", session_id),
			Some(serde_json::json!({ "coreCode": "SESSION_NOT_FOUND" })),
		);
	}

	async fn handle_initialize(&mut self, req: JsonRpcRequest) {
		let config: AppConfig =
			if req.params.is_null() || req.params == serde_json::Value::Object(Default::default()) {
				AppConfig::default()
			} else {
				match serde_json::from_value::<AppConfig>(req.params.clone()) {
					Ok(c) => c,
					Err(e) => {
						self.transport.write_error(
							req.id,
							INVALID_PARAMS,
							format!("Invalid config: {}", e),
							None,
						);
						return;
					}
				}
			};

		self.context = Some(CoreContext::new(config));
		tracing::info!("CoreContext initialized");
		self.transport
			.write_response(req.id, serde_json::json!({ "initialized": true }));
	}

	async fn handle_dispose(&mut self, req: JsonRpcRequest) {
		self.context = None;
		tracing::info!("CoreContext disposed");
		self.transport
			.write_response(req.id, serde_json::json!({}));
	}

	async fn handle_health(&self, req: JsonRpcRequest) {
		let initialized = self.context.is_some();
		self.transport
			.write_response(req.id, serde_json::json!({ "initialized": initialized }));
	}

	// ── Session handlers ─────────────────────────────────────────────────

	async fn handle_session_create(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};
		let id = ctx.session_manager.create();
		self.transport
			.write_response(req.id, serde_json::json!({ "id": id }));
	}

	async fn handle_session_get(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: SessionIdParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.get_info(&params.id) {
			Some(info) => {
				self.transport
					.write_response(req.id, session_info_to_json(&info));
			}
			None => {
				self.transport.write_error(
					req.id,
					CORE_ERROR,
					format!("Session not found: {}", params.id),
					Some(serde_json::json!({ "coreCode": "SESSION_NOT_FOUND" })),
				);
			}
		}
	}

	async fn handle_session_list(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let sessions: Vec<serde_json::Value> = ctx
			.session_manager
			.list()
			.iter()
			.map(session_info_to_json)
			.collect();

		self.transport
			.write_response(req.id, serde_json::json!({ "sessions": sessions }));
	}

	async fn handle_session_delete(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: SessionIdParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		let deleted = ctx.session_manager.delete(&params.id);
		self.transport
			.write_response(req.id, serde_json::json!({ "deleted": deleted }));
	}

	async fn handle_session_update_status(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: SessionUpdateStatusParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		let status = match parse_session_status(&params.status) {
			Some(s) => s,
			None => {
				self.transport.write_error(
					req.id,
					INVALID_PARAMS,
					format!(
						"Invalid status: '{}'. Expected 'active', 'completed', or 'aborted'",
						params.status
					),
					None,
				);
				return;
			}
		};

		let updated = ctx.session_manager.update_status(&params.id, status);
		self.transport
			.write_response(req.id, serde_json::json!({ "updated": updated }));
	}

	async fn handle_session_fork(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: SessionIdParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.fork(&params.id) {
			Some(new_id) => {
				self.transport
					.write_response(req.id, serde_json::json!({ "id": new_id }));
			}
			None => {
				self.transport.write_error(
					req.id,
					CORE_ERROR,
					format!("Session not found: {}", params.id),
					Some(serde_json::json!({ "coreCode": "SESSION_NOT_FOUND" })),
				);
			}
		}
	}

	// ── Conversation handlers ────────────────────────────────────────────

	async fn handle_conv_add_user(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvAddParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.with_session(&params.session_id, |session| {
			session.conversation.add_user(&params.content);
		}) {
			Some(()) => self.transport.write_response(req.id, serde_json::json!({})),
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}

	async fn handle_conv_add_assistant(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvAddParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.with_session(&params.session_id, |session| {
			session.conversation.add_assistant(&params.content);
		}) {
			Some(()) => self.transport.write_response(req.id, serde_json::json!({})),
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}

	async fn handle_conv_add_tool_result(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvToolResultParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		let tool_name = params
			.tool_name
			.as_deref()
			.unwrap_or(&params.tool_call_id);

		match ctx.session_manager.with_session(&params.session_id, |session| {
			session
				.conversation
				.add_tool_result(&params.tool_call_id, tool_name, &params.content);
		}) {
			Some(()) => self.transport.write_response(req.id, serde_json::json!({})),
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}

	async fn handle_conv_set_system_prompt(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvSetPromptParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.with_session(&params.session_id, |session| {
			session.conversation.set_system_prompt(params.prompt.clone());
		}) {
			Some(()) => self.transport.write_response(req.id, serde_json::json!({})),
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}

	async fn handle_conv_get_messages(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvSessionParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.with_session(&params.session_id, |session| {
			let messages = serde_json::to_value(session.conversation.messages())
				.unwrap_or(serde_json::json!([]));
			let system_prompt = session.conversation.system_prompt().map(|s| s.to_string());
			(messages, system_prompt)
		}) {
			Some((messages, system_prompt)) => {
				let mut result = serde_json::json!({ "messages": messages });
				if let Some(prompt) = system_prompt {
					result["systemPrompt"] = serde_json::json!(prompt);
				}
				self.transport.write_response(req.id, result);
			}
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}

	async fn handle_conv_compact(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvCompactParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.with_session(&params.session_id, |session| {
			session.conversation.compact(&params.summary);
		}) {
			Some(()) => self.transport.write_response(req.id, serde_json::json!({})),
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}

	async fn handle_conv_clear(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvSessionParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.with_session(&params.session_id, |session| {
			session.conversation.clear();
		}) {
			Some(()) => self.transport.write_response(req.id, serde_json::json!({})),
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}

	async fn handle_conv_stats(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvSessionParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.with_session(&params.session_id, |session| {
			serde_json::json!({
				"estimatedChars": session.conversation.estimated_chars(),
				"estimatedTokens": session.conversation.estimated_tokens(),
				"needsCompaction": session.conversation.needs_compaction(),
				"contextUsagePercent": session.conversation.context_usage_percent(),
			})
		}) {
			Some(stats) => self.transport.write_response(req.id, stats),
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}

	async fn handle_conv_to_json(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvSessionParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.with_session(&params.session_id, |session| {
			session.conversation.to_json()
		}) {
			Some(json) => {
				self.transport
					.write_response(req.id, serde_json::json!({ "json": json }));
			}
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}

	async fn handle_conv_from_json(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: ConvFromJsonParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.session_manager.with_session(&params.session_id, |session| {
			session.conversation.from_json(&params.json);
		}) {
			Some(()) => self.transport.write_response(req.id, serde_json::json!({})),
			None => self.write_session_not_found(req.id, &params.session_id),
		}
	}
}

// ---------------------------------------------------------------------------
// Param types
// ---------------------------------------------------------------------------

fn parse_params<T: serde::de::DeserializeOwned>(
	params: serde_json::Value,
) -> Result<T, String> {
	serde_json::from_value(params).map_err(|e| format!("Invalid params: {}", e))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionIdParams {
	id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionUpdateStatusParams {
	id: String,
	status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvSessionParams {
	session_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvAddParams {
	session_id: String,
	content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvToolResultParams {
	session_id: String,
	tool_call_id: String,
	#[serde(default)]
	tool_name: Option<String>,
	content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvSetPromptParams {
	session_id: String,
	prompt: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvCompactParams {
	session_id: String,
	summary: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvFromJsonParams {
	session_id: String,
	json: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a `SessionInfo` to a JSON value with camelCase field names.
fn session_info_to_json(info: &SessionInfo) -> serde_json::Value {
	serde_json::json!({
		"id": info.id,
		"status": info.status,
		"createdAt": info.created_at,
		"updatedAt": info.updated_at,
		"messageCount": info.message_count,
	})
}

/// Parse a status string into a `SessionStatus`.
fn parse_session_status(s: &str) -> Option<SessionStatus> {
	match s {
		"active" => Some(SessionStatus::Active),
		"completed" => Some(SessionStatus::Completed),
		"aborted" => Some(SessionStatus::Aborted),
		_ => None,
	}
}
