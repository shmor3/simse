use std::io::{self, Write};

use serde::Serialize;

use crate::rpc_protocol::{JsonRpcNotification, JsonRpcResponse};

pub struct NdjsonTransport;

impl Default for NdjsonTransport {
	fn default() -> Self {
		Self::new()
	}
}

impl NdjsonTransport {
	pub fn new() -> Self {
		Self
	}

	pub fn write_response(&self, id: u64, result: serde_json::Value) {
		self.write_line(&JsonRpcResponse::success(id, result));
	}

	pub fn write_error(
		&self,
		id: u64,
		code: i32,
		message: impl Into<String>,
		data: Option<serde_json::Value>,
	) {
		let mut resp = JsonRpcResponse::error(id, code, message);
		if let Some(ref mut err) = resp.error {
			err.data = data;
		}
		self.write_line(&resp);
	}

	pub fn write_notification(&self, method: impl Into<String>, params: serde_json::Value) {
		self.write_line(&JsonRpcNotification::new(method, Some(params)));
	}

	fn write_line(&self, value: &impl Serialize) {
		let mut stdout = io::stdout().lock();
		if let Err(e) = serde_json::to_writer(&mut stdout, value) {
			tracing::error!("Failed to serialize: {}", e);
			return;
		}
		let _ = writeln!(stdout);
		let _ = stdout.flush();
	}
}
