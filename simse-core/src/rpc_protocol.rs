use serde::{Deserialize, Serialize};

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const CORE_ERROR: i32 = -32000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
	pub id: u64,
	pub method: String,
	#[serde(default)]
	pub params: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
	pub jsonrpc: &'static str,
	pub id: u64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub result: Option<serde_json::Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
	pub fn success(id: u64, result: serde_json::Value) -> Self {
		Self {
			jsonrpc: "2.0",
			id,
			result: Some(result),
			error: None,
		}
	}

	pub fn error(id: u64, code: i32, message: impl Into<String>) -> Self {
		Self {
			jsonrpc: "2.0",
			id,
			result: None,
			error: Some(JsonRpcError {
				code,
				message: message.into(),
				data: None,
			}),
		}
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
	pub code: i32,
	pub message: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
	pub jsonrpc: &'static str,
	pub method: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
	pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
		Self {
			jsonrpc: "2.0",
			method: method.into(),
			params,
		}
	}
}
