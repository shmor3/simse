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

/// Parse a line as either a response or notification.
pub fn parse_message(line: &str) -> Result<RpcMessage, serde_json::Error> {
    // Try response first (has id)
    if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(line) {
        if resp.id.is_some() {
            return Ok(RpcMessage::Response(resp));
        }
    }
    // Try notification
    if let Ok(notif) = serde_json::from_str::<JsonRpcNotification>(line) {
        return Ok(RpcMessage::Notification(notif));
    }
    // Fallback: treat as response
    serde_json::from_str::<JsonRpcResponse>(line).map(RpcMessage::Response)
}

/// A parsed RPC message.
#[derive(Debug)]
pub enum RpcMessage {
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
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
}
