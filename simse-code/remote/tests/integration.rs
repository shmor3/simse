// ---------------------------------------------------------------------------
// Integration tests for simse-remote-engine
//
// Each test spawns the binary, communicates over JSON-RPC 2.0 / NDJSON stdio,
// and verifies responses.
// ---------------------------------------------------------------------------

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

struct RemoteProcess {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    next_id: AtomicU64,
}

#[derive(Debug)]
enum RpcResponse {
    Ok(Value),
    Error(Value),
}

impl RemoteProcess {
    fn spawn() -> Self {
        let bin = env!("CARGO_BIN_EXE_simse-remote-engine");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn simse-remote-engine");

        let stdout = child.stdout.take().expect("no stdout");
        let reader = BufReader::new(stdout);

        Self {
            child,
            reader,
            next_id: AtomicU64::new(1),
        }
    }

    fn send(&mut self, method: &str, params: Value) -> RpcResponse {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let stdin = self.child.stdin.as_mut().expect("no stdin");
        let mut line = serde_json::to_string(&request).unwrap();
        line.push('\n');
        stdin.write_all(line.as_bytes()).unwrap();
        stdin.flush().unwrap();

        loop {
            let mut buf = String::new();
            let n = self
                .reader
                .read_line(&mut buf)
                .expect("failed to read from stdout");
            if n == 0 {
                panic!("unexpected EOF while waiting for response to id={}", id);
            }
            let buf = buf.trim();
            if buf.is_empty() {
                continue;
            }
            let parsed: Value = serde_json::from_str(buf)
                .unwrap_or_else(|e| panic!("invalid JSON: {e}\nline: {buf}"));

            if parsed.get("id").is_none() {
                continue;
            }

            let resp_id = parsed["id"].as_u64().expect("response id is not u64");
            assert_eq!(resp_id, id, "response id mismatch");

            if let Some(error) = parsed.get("error") {
                return RpcResponse::Error(error.clone());
            }
            return RpcResponse::Ok(parsed.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    fn call(&mut self, method: &str, params: Value) -> Value {
        match self.send(method, params) {
            RpcResponse::Ok(v) => v,
            RpcResponse::Error(e) => panic!("expected success, got error: {e}"),
        }
    }

    fn call_err(&mut self, method: &str, params: Value) -> Value {
        match self.send(method, params) {
            RpcResponse::Error(e) => e,
            RpcResponse::Ok(v) => panic!("expected error, got success: {v}"),
        }
    }
}

impl Drop for RemoteProcess {
    fn drop(&mut self) {
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn health_returns_ok() {
    let mut proc = RemoteProcess::spawn();
    let result = proc.call("remote/health", json!({}));
    assert_eq!(result["ok"], true);
    assert_eq!(result["authenticated"], false);
    assert_eq!(result["tunnelConnected"], false);
}

#[test]
fn auth_status_when_not_logged_in() {
    let mut proc = RemoteProcess::spawn();
    let result = proc.call("auth/status", json!({}));
    assert_eq!(result["authenticated"], false);
    assert!(result["userId"].is_null());
}

#[test]
fn logout_succeeds_when_not_logged_in() {
    let mut proc = RemoteProcess::spawn();
    let result = proc.call("auth/logout", json!({}));
    assert_eq!(result["ok"], true);
}

#[test]
fn tunnel_connect_fails_without_auth() {
    let mut proc = RemoteProcess::spawn();
    let err = proc.call_err("tunnel/connect", json!({}));
    assert_eq!(err["data"]["remoteCode"], "REMOTE_NOT_AUTHENTICATED");
}

#[test]
fn tunnel_disconnect_fails_when_not_connected() {
    let mut proc = RemoteProcess::spawn();
    let err = proc.call_err("tunnel/disconnect", json!({}));
    assert_eq!(err["data"]["remoteCode"], "REMOTE_TUNNEL_NOT_CONNECTED");
}

#[test]
fn tunnel_status_when_not_connected() {
    let mut proc = RemoteProcess::spawn();
    let result = proc.call("tunnel/status", json!({}));
    assert_eq!(result["connected"], false);
    assert!(result["tunnelId"].is_null());
    assert_eq!(result["reconnectCount"], 0);
}

#[test]
fn unknown_method_returns_error() {
    let mut proc = RemoteProcess::spawn();
    let err = proc.call_err("nonexistent/method", json!({}));
    assert_eq!(err["code"], -32601);
}

#[test]
fn login_with_missing_params_returns_error() {
    let mut proc = RemoteProcess::spawn();
    // No email, no apiKey — should fail with invalid params or connection error
    let err = proc.call_err("auth/login", json!({}));
    assert!(err["data"]["remoteCode"].as_str().unwrap().starts_with("REMOTE_"));
}
