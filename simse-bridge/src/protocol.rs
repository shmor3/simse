//! JSON-RPC 2.0 protocol types for bridge communication.

use serde::{Deserialize, Serialize};

/// A JSON-RPC request.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

/// A JSON-RPC response.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC error.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// A JSON-RPC notification (no id).
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// A server-to-client JSON-RPC request (has both `id` and `method`).
///
/// This is used by ACP servers that send requests to the client, e.g.
/// `session/request_permission`. The client must respond with a matching `id`.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcServerRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// Parse a line as a response, notification, or server request.
pub fn parse_message(line: &str) -> Result<RpcMessage, serde_json::Error> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    let has_id = value.get("id").is_some_and(|v| !v.is_null());
    let has_method = value.get("method").is_some();

    if has_id && has_method {
        // Server-to-client request (has both `id` and `method`)
        serde_json::from_value(value).map(RpcMessage::ServerRequest)
    } else if has_id {
        // Response (has `id` but no `method`)
        serde_json::from_value(value).map(RpcMessage::Response)
    } else {
        // Notification (no `id`)
        serde_json::from_value(value).map(RpcMessage::Notification)
    }
}

/// A parsed RPC message.
#[derive(Debug, Clone)]
pub enum RpcMessage {
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
    ServerRequest(JsonRpcServerRequest),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_request() {
        let req = JsonRpcRequest::new(1, "initialize", None);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn parse_response() {
        let line = r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#;
        let msg = parse_message(line).unwrap();
        assert!(matches!(msg, RpcMessage::Response(r) if r.id == Some(1)));
    }

    #[test]
    fn parse_error_response() {
        let line = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Invalid Request"}}"#;
        let msg = parse_message(line).unwrap();
        if let RpcMessage::Response(r) = msg {
            assert!(r.error.is_some());
            assert_eq!(r.error.unwrap().code, -32600);
        } else {
            panic!("Expected response");
        }
    }

    #[test]
    fn parse_notification() {
        let line = r#"{"jsonrpc":"2.0","method":"stream.delta","params":{"text":"hello"}}"#;
        let msg = parse_message(line).unwrap();
        assert!(matches!(msg, RpcMessage::Notification(_)));
    }

    #[test]
    fn parse_server_request() {
        let line = r#"{"jsonrpc":"2.0","id":0,"method":"session/request_permission","params":{"options":[]}}"#;
        let msg = parse_message(line).unwrap();
        if let RpcMessage::ServerRequest(req) = msg {
            assert_eq!(req.id, 0);
            assert_eq!(req.method, "session/request_permission");
        } else {
            panic!("Expected ServerRequest, got {msg:?}");
        }
    }

    #[test]
    fn parse_server_request_not_confused_with_response() {
        // A message with both id and method should be a ServerRequest, not a Response
        let line = r#"{"jsonrpc":"2.0","id":5,"method":"some/method","params":{}}"#;
        let msg = parse_message(line).unwrap();
        assert!(matches!(msg, RpcMessage::ServerRequest(_)));
    }

    #[test]
    fn parse_response_without_method() {
        // A message with id but no method should be a Response
        let line = r#"{"jsonrpc":"2.0","id":5,"result":{"ok":true}}"#;
        let msg = parse_message(line).unwrap();
        assert!(matches!(msg, RpcMessage::Response(_)));
    }
}
