use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── JSON-RPC framing ──

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const VNET_ERROR: i32 = -32000;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

// ── Initialize ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub sandbox: Option<SandboxParams>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxParams {
    pub allowed_hosts: Option<Vec<String>>,
    pub allowed_ports: Option<Vec<PortRangeParam>>,
    pub allowed_protocols: Option<Vec<String>>,
    pub default_timeout_ms: Option<u64>,
    pub max_response_bytes: Option<u64>,
    pub max_connections: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortRangeParam {
    pub start: u16,
    pub end: u16,
}

// ── Network methods ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestParams {
    pub url: String,
    pub method: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponseResult {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_type: String,
    pub duration_ms: u64,
    pub bytes_received: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsConnectParams {
    pub url: String,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsMessageParams {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdParam {
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpConnectParams {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TcpSendParams {
    pub session_id: String,
    pub data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UdpSendParams {
    pub host: String,
    pub port: u16,
    pub data: String,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveParams {
    pub hostname: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveResult {
    pub addresses: Vec<String>,
    pub ttl: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsResult {
    pub total_requests: u64,
    pub total_errors: u64,
    pub active_sessions: usize,
    pub bytes_total: u64,
}

// ── Mock methods ──

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockRegisterParams {
    pub method: Option<String>,
    pub url_pattern: String,
    pub response: MockResponseParam,
    pub times: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockResponseParam {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub body: String,
    #[serde(default = "default_body_type")]
    pub body_type: String,
    pub delay_ms: Option<u64>,
}

fn default_body_type() -> String {
    "text".into()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockIdParam {
    pub id: String,
}

// ── Mock response types ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockDefinitionInfo {
    pub id: String,
    pub method: Option<String>,
    pub url_pattern: String,
    pub status: u16,
    pub times: Option<usize>,
    pub remaining: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockHitInfo {
    pub mock_id: String,
    pub url: String,
    pub method: Option<String>,
    pub timestamp: u64,
}

// ── Session response types ──

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub id: String,
    pub session_type: String,
    pub target: String,
    pub scheme: String,
    pub created_at: u64,
    pub last_active_at: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_params_deserializes() {
        let json = serde_json::json!({
            "sandbox": {
                "allowedHosts": ["*.example.com", "10.0.0.0/8"],
                "allowedPorts": [{"start": 80, "end": 80}, {"start": 443, "end": 443}],
                "allowedProtocols": ["http", "https"],
                "defaultTimeoutMs": 5000,
                "maxResponseBytes": 1048576,
                "maxConnections": 10
            }
        });
        let params: InitializeParams = serde_json::from_value(json).unwrap();
        let sandbox = params.sandbox.unwrap();
        assert_eq!(sandbox.allowed_hosts.unwrap().len(), 2);
        assert_eq!(sandbox.allowed_ports.unwrap()[0].start, 80);
    }

    #[test]
    fn http_request_params_deserializes() {
        let json = serde_json::json!({
            "url": "mock://api.example.com/users",
            "method": "GET",
            "headers": {"Authorization": "Bearer token"}
        });
        let params: HttpRequestParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.url, "mock://api.example.com/users");
        assert_eq!(params.method.unwrap(), "GET");
    }

    #[test]
    fn mock_register_params_deserializes() {
        let json = serde_json::json!({
            "urlPattern": "mock://api.example.com/*",
            "method": "GET",
            "response": {
                "status": 200,
                "headers": {"Content-Type": "application/json"},
                "body": "{\"ok\":true}",
                "bodyType": "text"
            },
            "times": 3
        });
        let params: MockRegisterParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.url_pattern, "mock://api.example.com/*");
        assert_eq!(params.times, Some(3));
        assert_eq!(params.response.status, 200);
    }

    #[test]
    fn http_response_result_serializes_camel_case() {
        let result = HttpResponseResult {
            status: 200,
            headers: std::collections::HashMap::new(),
            body: "hello".into(),
            body_type: "text".into(),
            duration_ms: 42,
            bytes_received: 5,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["durationMs"], 42);
        assert_eq!(json["bytesReceived"], 5);
        assert_eq!(json["bodyType"], "text");
    }

    #[test]
    fn session_info_serializes_camel_case() {
        let info = SessionInfo {
            id: "abc".into(),
            session_type: "ws".into(),
            target: "example.com:443".into(),
            scheme: "mock".into(),
            created_at: 1000,
            last_active_at: 2000,
            bytes_sent: 100,
            bytes_received: 200,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["sessionType"], "ws");
        assert_eq!(json["createdAt"], 1000);
        assert_eq!(json["lastActiveAt"], 2000);
        assert_eq!(json["bytesSent"], 100);
        assert_eq!(json["bytesReceived"], 200);
    }
}
