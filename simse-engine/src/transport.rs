use std::io::{self, BufRead, Write};

use crate::protocol::{JsonRpcError, JsonRpcNotification, JsonRpcResponse};

/// NDJSON transport over stdin/stdout for JSON-RPC 2.0 communication.
///
/// Reads one JSON object per line from stdin, writes one per line to stdout.
/// Matches the transport in simse-code/acp-ollama-bridge.ts.
pub struct NdjsonTransport {
    stdout: io::StdoutLock<'static>,
}

impl NdjsonTransport {
    /// Create a new transport that locks stdout for the process lifetime.
    pub fn new() -> Self {
        // Leak stdout handle to get a 'static lock — this is fine because
        // the transport lives for the entire process lifetime.
        let stdout = Box::leak(Box::new(io::stdout())).lock();
        Self { stdout }
    }

    /// Write a successful JSON-RPC response.
    pub fn write_response(&mut self, id: u64, result: serde_json::Value) {
        let msg = JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        };
        self.write_line(&msg);
    }

    /// Write a JSON-RPC error response.
    pub fn write_error(&mut self, id: u64, code: i32, message: impl Into<String>) {
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
    pub fn write_notification(&mut self, method: &str, params: serde_json::Value) {
        let msg = JsonRpcNotification {
            jsonrpc: "2.0",
            method: method.to_string(),
            params: Some(params),
        };
        self.write_line(&msg);
    }

    /// Read all incoming messages from stdin (blocks until EOF).
    /// Yields parsed messages, skipping blank lines and logging parse errors.
    pub fn read_lines(&self) -> impl Iterator<Item = crate::protocol::JsonRpcIncoming> {
        let stdin = io::stdin();
        let reader = stdin.lock();
        reader
            .lines()
            .filter_map(|line_result| {
                let line = line_result.ok()?;
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    return None;
                }
                match serde_json::from_str(&trimmed) {
                    Ok(msg) => Some(msg),
                    Err(e) => {
                        tracing::warn!("Failed to parse JSON-RPC message: {}", e);
                        None
                    }
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn write_line(&mut self, msg: &impl serde::Serialize) {
        if let Ok(json) = serde_json::to_string(msg) {
            let _ = writeln!(self.stdout, "{}", json);
            let _ = self.stdout.flush();
        }
    }
}
