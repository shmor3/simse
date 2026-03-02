use tokio::io::AsyncBufReadExt;

use crate::config::AppConfig;
use crate::context::CoreContext;
use crate::rpc_protocol::*;
use crate::rpc_transport::NdjsonTransport;

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

	async fn dispatch(&mut self, req: JsonRpcRequest) {
		match req.method.as_str() {
			"core/initialize" => self.handle_initialize(req).await,
			"core/dispose" => self.handle_dispose(req).await,
			"core/health" => self.handle_health(req).await,
			_ => self.transport.write_error(
				req.id,
				METHOD_NOT_FOUND,
				format!("Unknown method: {}", req.method),
				None,
			),
		}
	}

	#[allow(dead_code)]
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
}
