// ---------------------------------------------------------------------------
// Integration tests for simse-vnet-engine
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

/// Manages a running `simse-vnet-engine` child process and provides methods for
/// sending JSON-RPC requests and reading responses.
struct VnetProcess {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    next_id: AtomicU64,
}

#[derive(Debug)]
enum RpcResponse {
    Ok(Value),
    Error(Value),
}

impl VnetProcess {
    fn spawn() -> Self {
        let bin = env!("CARGO_BIN_EXE_simse-vnet-engine");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to spawn simse-vnet-engine");

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

        // Read lines until we find the response matching our id.
        loop {
            let mut buf = String::new();
            let n = self
                .reader
                .read_line(&mut buf)
                .expect("failed to read from stdout");
            if n == 0 {
                panic!(
                    "unexpected EOF from simse-vnet-engine while waiting for response to id={}",
                    id
                );
            }
            let buf = buf.trim();
            if buf.is_empty() {
                continue;
            }
            let parsed: Value = serde_json::from_str(buf)
                .unwrap_or_else(|e| panic!("invalid JSON from engine: {e}\nline: {buf}"));

            // Notifications have no `id` field -- skip them.
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

    /// Convenience: send a request, expect success, return the `result` value.
    fn call(&mut self, method: &str, params: Value) -> Value {
        match self.send(method, params) {
            RpcResponse::Ok(v) => v,
            RpcResponse::Error(e) => panic!("expected success, got error: {e}"),
        }
    }

    /// Convenience: send a request, expect an error, return the `error` object.
    fn call_err(&mut self, method: &str, params: Value) -> Value {
        match self.send(method, params) {
            RpcResponse::Error(e) => e,
            RpcResponse::Ok(v) => panic!("expected error, got success: {v}"),
        }
    }

    /// Initialize with default settings.
    fn initialize(&mut self) {
        self.call("initialize", json!({}));
    }
}

impl Drop for VnetProcess {
    fn drop(&mut self) {
        // Close stdin to let the child exit gracefully.
        drop(self.child.stdin.take());
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Test 1: initialize returns ok
// ---------------------------------------------------------------------------

#[test]
fn initialize_returns_ok() {
    let mut proc = VnetProcess::spawn();
    let result = proc.call("initialize", json!({}));
    assert_eq!(result["ok"], true);
}

// ---------------------------------------------------------------------------
// Test 2: method before init returns error
// ---------------------------------------------------------------------------

#[test]
fn method_before_init_returns_error() {
    let mut proc = VnetProcess::spawn();
    let err = proc.call_err("net/metrics", json!({}));
    assert_eq!(err["data"]["vnetCode"], "VNET_NOT_INITIALIZED");
}

// ---------------------------------------------------------------------------
// Test 3: unknown method returns error
// ---------------------------------------------------------------------------

#[test]
fn unknown_method_returns_error() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();
    let err = proc.call_err("nonexistent/method", json!({}));
    assert_eq!(err["code"], -32601);
}

// ---------------------------------------------------------------------------
// Test 4: mock register and HTTP request
// ---------------------------------------------------------------------------

#[test]
fn mock_register_and_http_request() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let reg = proc.call(
        "mock/register",
        json!({
            "urlPattern": "mock://api.example.com/users",
            "method": "GET",
            "response": {
                "status": 200,
                "headers": {"Content-Type": "application/json"},
                "body": "[{\"id\":1}]",
                "bodyType": "text"
            }
        }),
    );
    assert!(reg["id"].is_string());

    let resp = proc.call(
        "net/httpRequest",
        json!({
            "url": "mock://api.example.com/users",
            "method": "GET"
        }),
    );
    assert_eq!(resp["status"], 200);
    assert_eq!(resp["body"], "[{\"id\":1}]");
}

// ---------------------------------------------------------------------------
// Test 5: mock glob pattern
// ---------------------------------------------------------------------------

#[test]
fn mock_glob_pattern() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    proc.call(
        "mock/register",
        json!({
            "urlPattern": "mock://api/*",
            "response": {"status": 200, "body": "ok", "bodyType": "text"}
        }),
    );

    let resp = proc.call(
        "net/httpRequest",
        json!({"url": "mock://api/anything/here"}),
    );
    assert_eq!(resp["status"], 200);
}

// ---------------------------------------------------------------------------
// Test 6: mock no match returns error
// ---------------------------------------------------------------------------

#[test]
fn mock_no_match_returns_error() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let err = proc.call_err("net/httpRequest", json!({"url": "mock://nothing"}));
    assert_eq!(err["data"]["vnetCode"], "VNET_NO_MOCK_MATCH");
}

// ---------------------------------------------------------------------------
// Test 7: mock times limit
// ---------------------------------------------------------------------------

#[test]
fn mock_times_limit() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    proc.call(
        "mock/register",
        json!({
            "urlPattern": "mock://once",
            "response": {"status": 200, "body": "x", "bodyType": "text"},
            "times": 1
        }),
    );

    // First request succeeds
    proc.call("net/httpRequest", json!({"url": "mock://once"}));

    // Second request fails (mock consumed)
    let err = proc.call_err("net/httpRequest", json!({"url": "mock://once"}));
    assert_eq!(err["data"]["vnetCode"], "VNET_NO_MOCK_MATCH");
}

// ---------------------------------------------------------------------------
// Test 8: mock list and unregister
// ---------------------------------------------------------------------------

#[test]
fn mock_list_and_unregister() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let reg = proc.call(
        "mock/register",
        json!({
            "urlPattern": "mock://a",
            "response": {"status": 200, "body": "", "bodyType": "text"}
        }),
    );
    let id = reg["id"].as_str().unwrap().to_string();

    let list = proc.call("mock/list", json!({}));
    assert_eq!(list.as_array().unwrap().len(), 1);

    proc.call("mock/unregister", json!({"id": id}));

    let list = proc.call("mock/list", json!({}));
    assert!(list.as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Test 9: mock clear and history
// ---------------------------------------------------------------------------

#[test]
fn mock_clear_and_history() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    proc.call(
        "mock/register",
        json!({
            "urlPattern": "mock://test/*",
            "response": {"status": 200, "body": "ok", "bodyType": "text"}
        }),
    );

    proc.call("net/httpRequest", json!({"url": "mock://test/1"}));
    proc.call("net/httpRequest", json!({"url": "mock://test/2"}));

    let history = proc.call("mock/history", json!({}));
    assert_eq!(history.as_array().unwrap().len(), 2);

    proc.call("mock/clear", json!({}));

    let list = proc.call("mock/list", json!({}));
    assert!(list.as_array().unwrap().is_empty());
}

// ---------------------------------------------------------------------------
// Test 10: WebSocket connect, send, close
// ---------------------------------------------------------------------------

#[test]
fn ws_connect_send_close() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let conn = proc.call(
        "net/wsConnect",
        json!({"url": "mock://ws.example.com/chat"}),
    );
    let sid = conn["sessionId"].as_str().unwrap().to_string();
    assert_eq!(conn["status"], "connected");

    proc.call(
        "net/wsMessage",
        json!({"sessionId": sid, "data": "hello"}),
    );

    proc.call("net/wsClose", json!({"sessionId": sid}));

    // After close, session should not be found
    let err = proc.call_err("session/get", json!({"sessionId": sid}));
    assert_eq!(err["data"]["vnetCode"], "VNET_SESSION_NOT_FOUND");
}

// ---------------------------------------------------------------------------
// Test 11: session list and get
// ---------------------------------------------------------------------------

#[test]
fn session_list_and_get() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let conn = proc.call(
        "net/wsConnect",
        json!({"url": "mock://ws.example.com/test"}),
    );
    let sid = conn["sessionId"].as_str().unwrap().to_string();

    let list = proc.call("session/list", json!({}));
    assert_eq!(list.as_array().unwrap().len(), 1);

    let info = proc.call("session/get", json!({"sessionId": sid}));
    assert_eq!(info["sessionType"], "ws");
    assert_eq!(info["scheme"], "mock");
}

// ---------------------------------------------------------------------------
// Test 12: metrics track activity
// ---------------------------------------------------------------------------

#[test]
fn metrics_track_activity() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    proc.call(
        "mock/register",
        json!({
            "urlPattern": "mock://m/*",
            "response": {"status": 200, "body": "ok", "bodyType": "text"}
        }),
    );

    proc.call("net/httpRequest", json!({"url": "mock://m/1"}));
    proc.call("net/httpRequest", json!({"url": "mock://m/2"}));

    let m = proc.call("net/metrics", json!({}));
    assert_eq!(m["totalRequests"], 2);
    assert_eq!(m["totalErrors"], 0);
}

// ---------------------------------------------------------------------------
// Test 13: invalid URL scheme rejected
// ---------------------------------------------------------------------------

#[test]
fn invalid_url_scheme_rejected() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let err = proc.call_err(
        "net/httpRequest",
        json!({"url": "https://example.com"}),
    );
    assert_eq!(err["data"]["vnetCode"], "VNET_INVALID_PARAMS");
}

// ---------------------------------------------------------------------------
// Test 14: TCP connect and send
// ---------------------------------------------------------------------------

#[test]
fn tcp_connect_and_send() {
    let mut proc = VnetProcess::spawn();
    proc.initialize();

    let conn = proc.call(
        "net/tcpConnect",
        json!({"host": "db.example.com", "port": 5432}),
    );
    let sid = conn["sessionId"].as_str().unwrap().to_string();
    assert_eq!(conn["status"], "connected");

    let result = proc.call(
        "net/tcpSend",
        json!({"sessionId": sid, "data": "SELECT 1"}),
    );
    assert!(result["bytesWritten"].as_u64().unwrap() > 0);

    proc.call("net/tcpClose", json!({"sessionId": sid}));

    // After close, session should not be found
    let err = proc.call_err("session/get", json!({"sessionId": sid}));
    assert_eq!(err["data"]["vnetCode"], "VNET_SESSION_NOT_FOUND");
}
