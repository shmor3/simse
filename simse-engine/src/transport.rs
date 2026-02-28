use std::io::{self, Write};

use crate::protocol::{JsonRpcError, JsonRpcNotification, JsonRpcResponse};

/// NDJSON transport over stdin/stdout for JSON-RPC 2.0 communication.
///
/// Reads one JSON object per line from stdin, writes one per line to stdout.
/// Matches the transport in simse-code/acp-ollama-bridge.ts.
pub struct NdjsonTransport;

impl Default for NdjsonTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl NdjsonTransport {
    /// Create a new transport.
    pub fn new() -> Self {
        Self
    }

    /// Write a successful JSON-RPC response.
    pub fn write_response(&self, id: u64, result: serde_json::Value) {
        let msg = JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        };
        self.write_line(&msg);
    }

    /// Write a JSON-RPC error response.
    pub fn write_error(&self, id: u64, code: i32, message: impl Into<String>) {
        let msg = JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        };
        self.write_line(&msg);
    }

    /// Write a JSON-RPC notification (no id — fire and forget).
    pub fn write_notification(&self, method: &str, params: serde_json::Value) {
        let msg = JsonRpcNotification {
            jsonrpc: "2.0",
            method: method.to_string(),
            params: Some(params),
        };
        self.write_line(&msg);
    }

    fn write_line(&self, value: &impl serde::Serialize) {
        let mut stdout = io::stdout().lock();
        if let Err(e) = serde_json::to_writer(&mut stdout, value) {
            tracing::error!("Failed to serialize response: {}", e);
            return;
        }
        if let Err(e) = writeln!(stdout) {
            tracing::error!("Failed to write newline: {}", e);
        }
        if let Err(e) = stdout.flush() {
            tracing::error!("Failed to flush stdout: {}", e);
        }
    }
}
