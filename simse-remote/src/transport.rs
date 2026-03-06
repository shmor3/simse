use std::io::{self, Write};

use serde::Serialize;

#[derive(Serialize)]
struct JsonRpcResponse<'a> {
    jsonrpc: &'a str,
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcErrorBody>,
}

#[derive(Serialize)]
struct JsonRpcErrorBody {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct JsonRpcNotification<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

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
        self.write_line(&JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        });
    }

    pub fn write_error(
        &self,
        id: u64,
        code: i32,
        message: impl Into<String>,
        data: Option<serde_json::Value>,
    ) {
        self.write_line(&JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcErrorBody {
                code,
                message: message.into(),
                data,
            }),
        });
    }

    pub fn write_notification(&self, method: &str, params: serde_json::Value) {
        self.write_line(&JsonRpcNotification {
            jsonrpc: "2.0",
            method,
            params: Some(params),
        });
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
