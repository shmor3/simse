use serde::{Deserialize, Serialize};

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const ACP_ERROR: i32 = -32000;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
	pub id: u64,
	pub method: String,
	pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
	pub jsonrpc: String,
	pub id: u64,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub result: Option<serde_json::Value>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
	pub fn success(id: u64, result: serde_json::Value) -> Self {
		Self {
			jsonrpc: "2.0".into(),
			id,
			result: Some(result),
			error: None,
		}
	}

	pub fn error(id: u64, code: i32, message: impl Into<String>) -> Self {
		Self {
			jsonrpc: "2.0".into(),
			id,
			result: None,
			error: Some(JsonRpcError {
				code,
				message: message.into(),
				data: None,
			}),
		}
	}

	pub fn error_with_data(
		id: u64,
		code: i32,
		message: impl Into<String>,
		data: serde_json::Value,
	) -> Self {
		Self {
			jsonrpc: "2.0".into(),
			id,
			result: None,
			error: Some(JsonRpcError {
				code,
				message: message.into(),
				data: Some(data),
			}),
		}
	}
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
	pub code: i32,
	pub message: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
	pub jsonrpc: String,
	pub method: String,
	pub params: serde_json::Value,
}

impl JsonRpcNotification {
	pub fn new(method: impl Into<String>, params: serde_json::Value) -> Self {
		Self {
			jsonrpc: "2.0".into(),
			method: method.into(),
			params,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn response_success_omits_error() {
		let r = JsonRpcResponse::success(1, serde_json::json!({"ok": true}));
		let json = serde_json::to_value(&r).unwrap();
		assert!(json.get("error").is_none());
		assert_eq!(json["id"], 1);
		assert_eq!(json["jsonrpc"], "2.0");
	}

	#[test]
	fn response_error_omits_result() {
		let r = JsonRpcResponse::error(1, -32600, "bad request");
		let json = serde_json::to_value(&r).unwrap();
		assert!(json.get("result").is_none());
		assert_eq!(json["error"]["code"], -32600);
	}

	#[test]
	fn response_error_with_data() {
		let r = JsonRpcResponse::error_with_data(1, -32000, "err", serde_json::json!({"x": 1}));
		let json = serde_json::to_value(&r).unwrap();
		assert_eq!(json["error"]["data"]["x"], 1);
	}

	#[test]
	fn notification_serializes() {
		let n = JsonRpcNotification::new("stream/delta", serde_json::json!({"text": "hi"}));
		let json = serde_json::to_value(&n).unwrap();
		assert_eq!(json["method"], "stream/delta");
		assert_eq!(json["jsonrpc"], "2.0");
	}

	#[test]
	fn request_deserializes() {
		let json = r#"{"id":42,"method":"acp/initialize","params":{"config":{}}}"#;
		let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
		assert_eq!(req.id, 42);
		assert_eq!(req.method, "acp/initialize");
		assert!(req.params.is_some());
	}

	#[test]
	fn request_without_params() {
		let json = r#"{"id":1,"method":"acp/health"}"#;
		let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
		assert!(req.params.is_none());
	}
}
