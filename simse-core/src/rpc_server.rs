use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use tokio::io::AsyncBufReadExt;

use crate::config::AppConfig;
use crate::context::CoreContext;
use crate::hooks::*;
use crate::rpc_protocol::*;
use crate::rpc_transport::NdjsonTransport;
use crate::server::session::{SessionInfo, SessionStatus};
use crate::tasks::{TaskCreateInput, TaskStatus, TaskUpdateInput};
use crate::tools::types::{ToolCallRequest, ToolCallResult};

pub struct CoreRpcServer {
	transport: NdjsonTransport,
	context: Option<CoreContext>,
	event_unsubscribers: Arc<Mutex<HashMap<String, Box<dyn Fn() + Send>>>>,
	next_subscription_id: Arc<Mutex<u64>>,
	pending_hook_calls:
		Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
	hook_unsubscribers: Arc<Mutex<HashMap<String, Box<dyn Fn() + Send>>>>,
	next_hook_id: Arc<Mutex<u64>>,
}

impl CoreRpcServer {
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			context: None,
			event_unsubscribers: Arc::new(Mutex::new(HashMap::new())),
			next_subscription_id: Arc::new(Mutex::new(0)),
			pending_hook_calls: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
			hook_unsubscribers: Arc::new(Mutex::new(HashMap::new())),
			next_hook_id: Arc::new(Mutex::new(0)),
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

			// -- Tasks ---------------------------------------------------
			"task/create" => self.handle_task_create(req).await,
			"task/get" => self.handle_task_get(req).await,
			"task/list" => self.handle_task_list(req).await,
			"task/listAvailable" => self.handle_task_list_available(req).await,
			"task/update" => self.handle_task_update(req).await,
			"task/delete" => self.handle_task_delete(req).await,

			// -- Events --------------------------------------------------
			"event/subscribe" => self.handle_event_subscribe(req).await,
			"event/unsubscribe" => self.handle_event_unsubscribe(req).await,
			"event/publish" => self.handle_event_publish(req).await,

			// -- Hooks ---------------------------------------------------
			"hook/registerBefore" => self.handle_hook_register_before(req).await,
			"hook/registerAfter" => self.handle_hook_register_after(req).await,
			"hook/registerValidate" => self.handle_hook_register_validate(req).await,
			"hook/registerTransform" => self.handle_hook_register_transform(req).await,
			"hook/unregister" => self.handle_hook_unregister(req).await,
			"hook/result" => self.handle_hook_result(req).await,

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

	/// Expose pending hook calls for testing.
	#[doc(hidden)]
	pub fn pending_hook_calls(
		&self,
	) -> &Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<serde_json::Value>>>>
	{
		&self.pending_hook_calls
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

	fn write_task_not_found(&self, req_id: u64, task_id: &str) {
		self.transport.write_error(
			req_id,
			CORE_ERROR,
			format!("Task not found: {}", task_id),
			Some(serde_json::json!({ "coreCode": "TASK_NOT_FOUND" })),
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

	// ── Task handlers ────────────────────────────────────────────────

	async fn handle_task_create(&mut self, req: JsonRpcRequest) {
		let ctx = match self.context.as_mut() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: TaskCreateParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		let input = TaskCreateInput {
			subject: params.subject,
			description: params.description,
			active_form: params.active_form,
			owner: params.owner,
			metadata: params.metadata,
		};

		match ctx.task_list.create_checked(input) {
			Ok(task) => {
				let value = serde_json::to_value(&task).unwrap();
				self.transport.write_response(req.id, value);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					CORE_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	async fn handle_task_get(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: TaskIdParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		match ctx.task_list.get(&params.id) {
			Some(task) => {
				let value = serde_json::to_value(task).unwrap();
				self.transport.write_response(req.id, value);
			}
			None => {
				self.write_task_not_found(req.id, &params.id);
			}
		}
	}

	async fn handle_task_list(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let tasks: Vec<serde_json::Value> = ctx
			.task_list
			.list()
			.iter()
			.map(|t| serde_json::to_value(t).unwrap())
			.collect();

		self.transport
			.write_response(req.id, serde_json::json!({ "tasks": tasks }));
	}

	async fn handle_task_list_available(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let tasks: Vec<serde_json::Value> = ctx
			.task_list
			.list_available()
			.iter()
			.map(|t| serde_json::to_value(t).unwrap())
			.collect();

		self.transport
			.write_response(req.id, serde_json::json!({ "tasks": tasks }));
	}

	async fn handle_task_update(&mut self, req: JsonRpcRequest) {
		let params: TaskUpdateParams = match parse_params(req.params.clone()) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		let status = if let Some(ref s) = params.status {
			match parse_task_status(s) {
				Some(status) => Some(status),
				None => {
					self.transport.write_error(
						req.id,
						INVALID_PARAMS,
						format!(
							"Invalid status: '{}'. Expected 'pending', 'in_progress', 'completed', or 'deleted'",
							s
						),
						None,
					);
					return;
				}
			}
		} else {
			None
		};

		let ctx = match self.context.as_mut() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let input = TaskUpdateInput {
			status,
			subject: params.subject,
			description: params.description,
			active_form: params.active_form,
			owner: params.owner,
			metadata: params.metadata,
			add_blocks: params.add_blocks,
			add_blocked_by: params.add_blocked_by,
		};

		match ctx.task_list.update(&params.id, input) {
			Ok(Some(task)) => {
				let value = serde_json::to_value(&task).unwrap();
				self.transport.write_response(req.id, value);
			}
			Ok(None) => {
				self.write_task_not_found(req.id, &params.id);
			}
			Err(e) => {
				self.transport.write_error(
					req.id,
					CORE_ERROR,
					e.to_string(),
					Some(e.to_json_rpc_error()),
				);
			}
		}
	}

	async fn handle_task_delete(&mut self, req: JsonRpcRequest) {
		let ctx = match self.context.as_mut() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: TaskIdParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		let deleted = ctx.task_list.delete(&params.id);
		self.transport
			.write_response(req.id, serde_json::json!({ "deleted": deleted }));
	}

	// ── Event handlers ───────────────────────────────────────────────────

	async fn handle_event_subscribe(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: EventSubscribeParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		// Generate subscription ID
		let sub_id = {
			let mut next = self.next_subscription_id.lock().unwrap_or_else(|e| e.into_inner());
			let id = *next;
			*next = next.wrapping_add(1);
			format!("sub_{}", id)
		};

		// Box the unsubscribe closures so subscribe() and subscribe_all()
		// return types unify into a single `Box<dyn Fn() + Send>`.
		let unsub: Box<dyn Fn() + Send> = if params.event_type == "*" {
			let sub_id_for_closure = sub_id.clone();
			let transport = NdjsonTransport::new();
			let raw = ctx.event_bus.subscribe_all(move |event_type, payload| {
				transport.write_notification(
					"event/fired",
					serde_json::json!({
						"type": event_type,
						"payload": payload,
						"subscriptionId": sub_id_for_closure,
					}),
				);
			});
			Box::new(raw)
		} else {
			let sub_id_for_closure = sub_id.clone();
			let transport = NdjsonTransport::new();
			let event_type_for_notification = params.event_type.clone();
			let raw = ctx.event_bus.subscribe(&params.event_type, move |payload| {
				transport.write_notification(
					"event/fired",
					serde_json::json!({
						"type": event_type_for_notification,
						"payload": payload,
						"subscriptionId": sub_id_for_closure,
					}),
				);
			});
			Box::new(raw)
		};

		// Store the unsubscribe closure
		{
			let mut unsubs = self.event_unsubscribers.lock().unwrap_or_else(|e| e.into_inner());
			unsubs.insert(sub_id.clone(), Box::new(unsub));
		}

		self.transport
			.write_response(req.id, serde_json::json!({ "subscriptionId": sub_id }));
	}

	async fn handle_event_unsubscribe(&self, req: JsonRpcRequest) {
		let _ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: EventUnsubscribeParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		let unsub = {
			let mut unsubs = self.event_unsubscribers.lock().unwrap_or_else(|e| e.into_inner());
			unsubs.remove(&params.subscription_id)
		};

		if let Some(unsub_fn) = unsub {
			unsub_fn();
		}

		self.transport.write_response(req.id, serde_json::json!({}));
	}

	async fn handle_event_publish(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: EventPublishParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		ctx.event_bus.publish(&params.event_type, params.payload);
		self.transport.write_response(req.id, serde_json::json!({}));
	}

	// ── Hook handlers ───────────────────────────────────────────────────

	fn next_hook_id_string(&self) -> String {
		let mut next = self.next_hook_id.lock().unwrap_or_else(|e| e.into_inner());
		let id = *next;
		*next = next.wrapping_add(1);
		format!("hook_{}", id)
	}

	async fn handle_hook_register_before(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let hook_id = self.next_hook_id_string();
		let pending = Arc::clone(&self.pending_hook_calls);
		let hook_id_for_closure = hook_id.clone();

		let handler: BeforeHandler = Arc::new(move |request: ToolCallRequest| {
			let pending = Arc::clone(&pending);
			let _hook_id = hook_id_for_closure.clone();
			Box::pin(async move {
				let request_id = uuid::Uuid::new_v4().to_string();
				let (tx, rx) = tokio::sync::oneshot::channel();

				// Store the sender
				{
					let mut map = pending.lock().await;
					map.insert(request_id.clone(), tx);
				}

				// Send notification to TS
				let transport = NdjsonTransport::new();
				transport.write_notification(
					"hook/execute",
					serde_json::json!({
						"requestId": request_id,
						"hookType": "before",
						"toolName": &request.name,
						"args": &request.arguments,
					}),
				);

				// Wait for result with 60s timeout
				match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
					Ok(Ok(result)) => {
						if result.get("blocked").is_some() {
							let reason = result["blocked"]
								.as_str()
								.unwrap_or("Blocked by hook")
								.to_string();
							BeforeHookResult::Blocked(BlockedResult { reason })
						} else {
							// Parse possibly modified request
							let name = result
								.get("name")
								.and_then(|v| v.as_str())
								.unwrap_or(&request.name)
								.to_string();
							let arguments = result
								.get("args")
								.cloned()
								.unwrap_or_else(|| request.arguments.clone());
							BeforeHookResult::Continue(ToolCallRequest {
								id: request.id.clone(),
								name,
								arguments,
							})
						}
					}
					Ok(Err(_)) => {
						// Channel closed — continue with original request
						BeforeHookResult::Continue(request)
					}
					Err(_) => {
						// Timeout — clean up and continue
						let mut map = pending.lock().await;
						map.remove(&request_id);
						BeforeHookResult::Continue(request)
					}
				}
			})
		});

		let unsub = ctx.hook_system.register_before(handler);

		// Store unsubscribe closure
		{
			let mut unsubs = self
				.hook_unsubscribers
				.lock()
				.unwrap_or_else(|e| e.into_inner());
			unsubs.insert(hook_id.clone(), Box::new(unsub));
		}

		self.transport
			.write_response(req.id, serde_json::json!({ "hookId": hook_id }));
	}

	async fn handle_hook_register_after(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let hook_id = self.next_hook_id_string();
		let pending = Arc::clone(&self.pending_hook_calls);

		let handler: AfterHandler = Arc::new(move |context: AfterHookContext| {
			let pending = Arc::clone(&pending);
			Box::pin(async move {
				let request_id = uuid::Uuid::new_v4().to_string();
				let (tx, rx) = tokio::sync::oneshot::channel();

				{
					let mut map = pending.lock().await;
					map.insert(request_id.clone(), tx);
				}

				let transport = NdjsonTransport::new();
				transport.write_notification(
					"hook/execute",
					serde_json::json!({
						"requestId": request_id,
						"hookType": "after",
						"request": {
							"id": &context.request.id,
							"name": &context.request.name,
							"args": &context.request.arguments,
						},
						"result": {
							"id": &context.result.id,
							"name": &context.result.name,
							"output": &context.result.output,
							"isError": context.result.is_error,
						},
					}),
				);

				match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
					Ok(Ok(result)) => {
						let output = result
							.get("output")
							.and_then(|v| v.as_str())
							.unwrap_or(&context.result.output)
							.to_string();
						let is_error = result
							.get("isError")
							.and_then(|v| v.as_bool())
							.unwrap_or(context.result.is_error);
						ToolCallResult {
							id: context.result.id,
							name: context.result.name,
							output,
							is_error,
							duration_ms: context.result.duration_ms,
						}
					}
					Ok(Err(_)) => context.result,
					Err(_) => {
						let mut map = pending.lock().await;
						map.remove(&request_id);
						context.result
					}
				}
			})
		});

		let unsub = ctx.hook_system.register_after(handler);

		{
			let mut unsubs = self
				.hook_unsubscribers
				.lock()
				.unwrap_or_else(|e| e.into_inner());
			unsubs.insert(hook_id.clone(), Box::new(unsub));
		}

		self.transport
			.write_response(req.id, serde_json::json!({ "hookId": hook_id }));
	}

	async fn handle_hook_register_validate(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let hook_id = self.next_hook_id_string();
		let pending = Arc::clone(&self.pending_hook_calls);

		let handler: ValidateHandler = Arc::new(move |context: ValidateHookContext| {
			let pending = Arc::clone(&pending);
			Box::pin(async move {
				let request_id = uuid::Uuid::new_v4().to_string();
				let (tx, rx) = tokio::sync::oneshot::channel();

				{
					let mut map = pending.lock().await;
					map.insert(request_id.clone(), tx);
				}

				let transport = NdjsonTransport::new();
				transport.write_notification(
					"hook/execute",
					serde_json::json!({
						"requestId": request_id,
						"hookType": "validate",
						"request": {
							"id": &context.request.id,
							"name": &context.request.name,
							"args": &context.request.arguments,
						},
						"result": {
							"id": &context.result.id,
							"name": &context.result.name,
							"output": &context.result.output,
							"isError": context.result.is_error,
						},
					}),
				);

				match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
					Ok(Ok(result)) => {
						if let Some(messages) = result.get("messages") {
							messages
								.as_array()
								.map(|arr| {
									arr.iter()
										.filter_map(|v| v.as_str().map(|s| s.to_string()))
										.collect()
								})
								.unwrap_or_default()
						} else {
							Vec::new()
						}
					}
					Ok(Err(_)) => Vec::new(),
					Err(_) => {
						let mut map = pending.lock().await;
						map.remove(&request_id);
						Vec::new()
					}
				}
			})
		});

		let unsub = ctx.hook_system.register_validate(handler);

		{
			let mut unsubs = self
				.hook_unsubscribers
				.lock()
				.unwrap_or_else(|e| e.into_inner());
			unsubs.insert(hook_id.clone(), Box::new(unsub));
		}

		self.transport
			.write_response(req.id, serde_json::json!({ "hookId": hook_id }));
	}

	async fn handle_hook_register_transform(&self, req: JsonRpcRequest) {
		let ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let hook_id = self.next_hook_id_string();
		let pending = Arc::clone(&self.pending_hook_calls);

		let handler: PromptTransformHandler = Arc::new(move |prompt: String| {
			let pending = Arc::clone(&pending);
			Box::pin(async move {
				let request_id = uuid::Uuid::new_v4().to_string();
				let (tx, rx) = tokio::sync::oneshot::channel();

				{
					let mut map = pending.lock().await;
					map.insert(request_id.clone(), tx);
				}

				let transport = NdjsonTransport::new();
				transport.write_notification(
					"hook/execute",
					serde_json::json!({
						"requestId": request_id,
						"hookType": "prompt_transform",
						"prompt": &prompt,
					}),
				);

				match tokio::time::timeout(std::time::Duration::from_secs(60), rx).await {
					Ok(Ok(result)) => result
						.get("prompt")
						.and_then(|v| v.as_str())
						.unwrap_or(&prompt)
						.to_string(),
					Ok(Err(_)) => prompt,
					Err(_) => {
						let mut map = pending.lock().await;
						map.remove(&request_id);
						prompt
					}
				}
			})
		});

		let unsub = ctx.hook_system.register_prompt_transform(handler);

		{
			let mut unsubs = self
				.hook_unsubscribers
				.lock()
				.unwrap_or_else(|e| e.into_inner());
			unsubs.insert(hook_id.clone(), Box::new(unsub));
		}

		self.transport
			.write_response(req.id, serde_json::json!({ "hookId": hook_id }));
	}

	async fn handle_hook_unregister(&self, req: JsonRpcRequest) {
		let _ctx = match self.require_context() {
			Some(c) => c,
			None => {
				self.write_not_initialized(req.id);
				return;
			}
		};

		let params: HookUnregisterParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		let unsub = {
			let mut unsubs = self
				.hook_unsubscribers
				.lock()
				.unwrap_or_else(|e| e.into_inner());
			unsubs.remove(&params.hook_id)
		};

		if let Some(unsub_fn) = unsub {
			unsub_fn();
		}

		self.transport.write_response(req.id, serde_json::json!({}));
	}

	async fn handle_hook_result(&self, req: JsonRpcRequest) {
		// hook/result does not require context — it only resolves pending channels
		let params: HookResultParams = match parse_params(req.params) {
			Ok(p) => p,
			Err(e) => {
				self.transport
					.write_error(req.id, INVALID_PARAMS, e, None);
				return;
			}
		};

		let sender = {
			let mut map = self.pending_hook_calls.lock().await;
			map.remove(&params.request_id)
		};

		if let Some(tx) = sender {
			let _ = tx.send(params.result);
		}

		self.transport.write_response(req.id, serde_json::json!({}));
	}

	// ── Conversation handlers ────────────────────────────────────────────

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

// -- Task params -----------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskIdParams {
	id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskCreateParams {
	subject: String,
	description: String,
	#[serde(default)]
	active_form: Option<String>,
	#[serde(default)]
	owner: Option<String>,
	#[serde(default)]
	metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TaskUpdateParams {
	id: String,
	#[serde(default)]
	status: Option<String>,
	#[serde(default)]
	subject: Option<String>,
	#[serde(default)]
	description: Option<String>,
	#[serde(default)]
	active_form: Option<String>,
	#[serde(default)]
	owner: Option<String>,
	#[serde(default)]
	metadata: Option<HashMap<String, serde_json::Value>>,
	#[serde(default)]
	add_blocks: Option<Vec<String>>,
	#[serde(default)]
	add_blocked_by: Option<Vec<String>>,
}

// -- Hook params -----------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookUnregisterParams {
	hook_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookResultParams {
	request_id: String,
	result: serde_json::Value,
}

// -- Event params ----------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventSubscribeParams {
	event_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventUnsubscribeParams {
	subscription_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventPublishParams {
	#[serde(rename = "type")]
	event_type: String,
	#[serde(default)]
	payload: serde_json::Value,
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

/// Parse a status string into a `TaskStatus`.
fn parse_task_status(s: &str) -> Option<TaskStatus> {
	match s {
		"pending" => Some(TaskStatus::Pending),
		"in_progress" => Some(TaskStatus::InProgress),
		"completed" => Some(TaskStatus::Completed),
		"deleted" => Some(TaskStatus::Deleted),
		_ => None,
	}
}
