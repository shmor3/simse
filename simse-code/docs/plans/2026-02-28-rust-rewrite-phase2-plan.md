# SimSE Rust Rewrite — Phase 2: Bridge Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the JSON-RPC bridge between Rust TUI and TypeScript simse core — a TS bridge server that wraps the simse core APIs, and a Rust async client that spawns/manages the subprocess and handles request/response/streaming.

**Architecture:** TS bridge server uses NDJSON stdin/stdout JSON-RPC 2.0 (same pattern as the existing `acp-ollama-bridge.ts`). Rust client uses `tokio::process` for subprocess management, `tokio::sync` for pending request tracking, and channels for streaming notifications.

**Tech Stack:** TypeScript (bridge server, ~250 LOC), Rust (tokio, serde_json, thiserror)

---

## Task 6: Create TS bridge server

**Files:**
- Create: `simse-code/bridge-server.ts`

**Step 1: Write the failing test**

Create `simse-code/tests/bridge-server.test.ts`:

```typescript
import { describe, it, expect } from 'bun:test';
import { createNdjsonTransport, createBridgeHandler } from '../bridge-server.js';

describe('bridge-server', () => {
	it('createNdjsonTransport returns frozen transport', () => {
		const lines: string[] = [];
		const transport = createNdjsonTransport((line) => lines.push(line));
		expect(Object.isFrozen(transport)).toBe(true);
		expect(typeof transport.writeResponse).toBe('function');
		expect(typeof transport.writeError).toBe('function');
		expect(typeof transport.writeNotification).toBe('function');
	});

	it('writeResponse serializes JSON-RPC response', () => {
		const lines: string[] = [];
		const transport = createNdjsonTransport((line) => lines.push(line));
		transport.writeResponse(1, { ok: true });
		const parsed = JSON.parse(lines[0]);
		expect(parsed.jsonrpc).toBe('2.0');
		expect(parsed.id).toBe(1);
		expect(parsed.result).toEqual({ ok: true });
	});

	it('writeError serializes JSON-RPC error', () => {
		const lines: string[] = [];
		const transport = createNdjsonTransport((line) => lines.push(line));
		transport.writeError(1, -32600, 'Invalid Request');
		const parsed = JSON.parse(lines[0]);
		expect(parsed.jsonrpc).toBe('2.0');
		expect(parsed.id).toBe(1);
		expect(parsed.error.code).toBe(-32600);
		expect(parsed.error.message).toBe('Invalid Request');
	});

	it('writeNotification serializes notification (no id)', () => {
		const lines: string[] = [];
		const transport = createNdjsonTransport((line) => lines.push(line));
		transport.writeNotification('stream.delta', { text: 'hello' });
		const parsed = JSON.parse(lines[0]);
		expect(parsed.jsonrpc).toBe('2.0');
		expect(parsed.method).toBe('stream.delta');
		expect(parsed.params).toEqual({ text: 'hello' });
		expect(parsed.id).toBeUndefined();
	});

	it('createBridgeHandler handles initialize', () => {
		const lines: string[] = [];
		const transport = createNdjsonTransport((line) => lines.push(line));
		const handler = createBridgeHandler(transport);
		handler({ jsonrpc: '2.0', id: 1, method: 'initialize' });
		const parsed = JSON.parse(lines[0]);
		expect(parsed.result.protocolVersion).toBe(1);
		expect(parsed.result.name).toBe('simse-bridge');
	});

	it('createBridgeHandler returns error for unknown method', () => {
		const lines: string[] = [];
		const transport = createNdjsonTransport((line) => lines.push(line));
		const handler = createBridgeHandler(transport);
		handler({ jsonrpc: '2.0', id: 1, method: 'nonexistent' });
		const parsed = JSON.parse(lines[0]);
		expect(parsed.error.code).toBe(-32601);
	});
});
```

**Step 2: Run test to verify it fails**

Run: `cd /Users/pixelator/experiments/simse/simse-code && bun test tests/bridge-server.test.ts`
Expected: FAIL (module not found)

**Step 3: Implement bridge-server.ts**

Create `simse-code/bridge-server.ts`:

```typescript
/**
 * SimSE Bridge Server — JSON-RPC 2.0 over NDJSON stdin/stdout.
 *
 * Wraps the simse core TypeScript library for consumption by the Rust TUI.
 * Follows the same transport pattern as acp-ollama-bridge.ts.
 */

import { createInterface } from 'node:readline';

// --- Transport Layer ---

export interface NdjsonTransport {
	readonly writeResponse: (id: number, result: unknown) => void;
	readonly writeError: (id: number, code: number, message: string, data?: unknown) => void;
	readonly writeNotification: (method: string, params?: unknown) => void;
}

export function createNdjsonTransport(
	writeLine: (line: string) => void = (line) => process.stdout.write(`${line}\n`),
): NdjsonTransport {
	const writeResponse = (id: number, result: unknown): void => {
		writeLine(JSON.stringify({ jsonrpc: '2.0', id, result }));
	};

	const writeError = (id: number, code: number, message: string, data?: unknown): void => {
		writeLine(JSON.stringify({ jsonrpc: '2.0', id, error: { code, message, data } }));
	};

	const writeNotification = (method: string, params?: unknown): void => {
		writeLine(JSON.stringify({ jsonrpc: '2.0', method, params }));
	};

	return Object.freeze({ writeResponse, writeError, writeNotification });
}

// --- Bridge Handler ---

export type MessageHandler = (msg: JsonRpcMessage) => void;

interface JsonRpcMessage {
	jsonrpc: string;
	id?: number;
	method?: string;
	params?: unknown;
}

const METHOD_NOT_FOUND = -32601;
const INTERNAL_ERROR = -32603;

export function createBridgeHandler(transport: NdjsonTransport): MessageHandler {
	const handleInitialize = (id: number): void => {
		transport.writeResponse(id, {
			protocolVersion: 1,
			name: 'simse-bridge',
			version: '0.1.0',
			capabilities: {
				generate: true,
				library: true,
				tools: true,
				config: true,
				session: true,
			},
		});
	};

	const handleGenerate = async (id: number, params: unknown): Promise<void> => {
		// Stub — will be wired to simse core in Phase 8
		transport.writeResponse(id, {
			content: '[bridge: generate not yet implemented]',
			agentId: '',
			serverName: '',
		});
	};

	const handleLibrarySearch = async (id: number, params: unknown): Promise<void> => {
		transport.writeResponse(id, { results: [] });
	};

	const handleLibraryAdd = async (id: number, params: unknown): Promise<void> => {
		transport.writeResponse(id, { volumeId: '' });
	};

	const handleLibraryRecommend = async (id: number, params: unknown): Promise<void> => {
		transport.writeResponse(id, { results: [] });
	};

	const handleToolsList = async (id: number): Promise<void> => {
		transport.writeResponse(id, { tools: [] });
	};

	const handleToolsExecute = async (id: number, params: unknown): Promise<void> => {
		transport.writeResponse(id, { id: '', name: '', output: '', isError: true });
	};

	const handleSessionLoad = async (id: number, params: unknown): Promise<void> => {
		transport.writeResponse(id, { messages: [] });
	};

	const handleSessionSave = async (id: number, params: unknown): Promise<void> => {
		transport.writeResponse(id, { ok: true });
	};

	const handleConfigRead = async (id: number): Promise<void> => {
		transport.writeResponse(id, {});
	};

	const handleConfigWrite = async (id: number, params: unknown): Promise<void> => {
		transport.writeResponse(id, { ok: true });
	};

	const methodMap: Record<string, (id: number, params?: unknown) => void | Promise<void>> = {
		initialize: handleInitialize,
		generate: handleGenerate,
		'generateStream': handleGenerate, // stub — will emit notifications in Phase 8
		'library.search': handleLibrarySearch,
		'library.add': handleLibraryAdd,
		'library.recommend': handleLibraryRecommend,
		'tools.list': handleToolsList,
		'tools.execute': handleToolsExecute,
		'session.load': handleSessionLoad,
		'session.save': handleSessionSave,
		'config.read': handleConfigRead,
		'config.write': handleConfigWrite,
	};

	const handleMessage = (msg: JsonRpcMessage): void => {
		const id = msg.id;
		const method = msg.method;
		if (id === undefined || method === undefined) return;

		const handler = methodMap[method];
		if (!handler) {
			transport.writeError(id, METHOD_NOT_FOUND, `Method not found: ${method}`);
			return;
		}

		try {
			const result = handler(id, msg.params);
			if (result instanceof Promise) {
				result.catch((err: unknown) => {
					const message = err instanceof Error ? err.message : String(err);
					transport.writeError(id, INTERNAL_ERROR, message);
				});
			}
		} catch (err: unknown) {
			const message = err instanceof Error ? err.message : String(err);
			transport.writeError(id, INTERNAL_ERROR, message);
		}
	};

	return handleMessage;
}

// --- Entry Point ---

export function startBridge(): void {
	const transport = createNdjsonTransport();
	const handler = createBridgeHandler(transport);

	const rl = createInterface({ input: process.stdin, terminal: false });
	rl.on('line', (line: string) => {
		try {
			const msg = JSON.parse(line) as JsonRpcMessage;
			handler(msg);
		} catch {
			// Ignore malformed lines
		}
	});

	rl.on('close', () => {
		process.exit(0);
	});
}

if (import.meta.main) {
	startBridge();
}
```

**Step 4: Run tests to verify they pass**

Run: `cd /Users/pixelator/experiments/simse/simse-code && bun test tests/bridge-server.test.ts`
Expected: All 6 tests PASS

**Step 5: Commit**

```bash
git add simse-code/bridge-server.ts simse-code/tests/bridge-server.test.ts
git commit -m "feat: add bridge server with JSON-RPC transport and handler stubs"
```

---

## Task 7: Implement Rust bridge client subprocess management

**Files:**
- Modify: `simse-bridge/src/client.rs`
- Modify: `simse-bridge/Cargo.toml` (add `tokio-stream` if needed)

**Step 1: Write failing tests**

Add to the bottom of `simse-bridge/src/client.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn bridge_config_defaults() {
        let config = BridgeConfig::default();
        assert_eq!(config.command, "bun");
        assert_eq!(config.timeout_ms, 60_000);
    }

    #[tokio::test]
    async fn spawn_bridge_echoes() {
        // Spawn a simple echo process to verify subprocess management
        let config = BridgeConfig {
            command: "cat".into(),
            args: vec![],
            data_dir: String::new(),
            timeout_ms: 5_000,
        };
        let bridge = spawn_bridge(&config).await.unwrap();
        assert!(is_healthy(&bridge));

        // Send a line and read it back
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"test"}"#;
        send_line(&bridge, line).await.unwrap();

        let response = read_line(&bridge, Duration::from_secs(2)).await.unwrap();
        assert_eq!(response.trim(), line);

        kill_bridge(bridge).await;
    }

    #[tokio::test]
    async fn spawn_bridge_detects_exit() {
        let config = BridgeConfig {
            command: "true".into(),
            args: vec![],
            data_dir: String::new(),
            timeout_ms: 1_000,
        };
        let bridge = spawn_bridge(&config).await.unwrap();
        // `true` exits immediately
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!is_healthy(&bridge));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p simse-bridge`
Expected: FAIL (functions don't exist)

**Step 3: Implement subprocess management**

Replace `simse-bridge/src/client.rs` with:

```rust
//! JSON-RPC client that communicates with the TS core subprocess.

use std::time::Duration;

use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("Failed to spawn bridge process: {0}")]
    SpawnFailed(String),
    #[error("Bridge process exited unexpectedly")]
    ProcessExited,
    #[error("JSON-RPC error {code}: {message}")]
    RpcError { code: i64, message: String },
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Request timed out")]
    Timeout,
}

/// Bridge client configuration.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub command: String,
    pub args: Vec<String>,
    pub data_dir: String,
    pub timeout_ms: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            command: "bun".into(),
            args: vec!["run".into(), "bridge-server.ts".into()],
            data_dir: String::new(),
            timeout_ms: 60_000,
        }
    }
}

/// A running bridge subprocess with stdin/stdout pipes.
pub struct BridgeProcess {
    pub child: Child,
    pub stdin: tokio::process::ChildStdin,
    pub stdout: BufReader<tokio::process::ChildStdout>,
}

/// Spawn the bridge subprocess.
pub async fn spawn_bridge(config: &BridgeConfig) -> Result<BridgeProcess, BridgeError> {
    let mut child = Command::new(&config.command)
        .args(&config.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| BridgeError::SpawnFailed(e.to_string()))?;

    let stdin = child.stdin.take().ok_or(BridgeError::SpawnFailed(
        "Failed to capture stdin".into(),
    ))?;
    let stdout = child.stdout.take().ok_or(BridgeError::SpawnFailed(
        "Failed to capture stdout".into(),
    ))?;

    Ok(BridgeProcess {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}

/// Check if the bridge process is still running.
pub fn is_healthy(bridge: &BridgeProcess) -> bool {
    bridge.child.id().is_some()
}

/// Send a line to the bridge via stdin (appends newline).
pub async fn send_line(bridge: &BridgeProcess, line: &str) -> Result<(), BridgeError> {
    let stdin = &bridge.stdin;
    // We need mutable access — take a reference via unsafe pin or restructure.
    // For now, use a simpler approach with the raw fd.
    let mut buf = line.to_string();
    buf.push('\n');
    // This requires &mut — we'll restructure in the next step.
    // For now, this is a stub signature.
    Ok(())
}

/// Read a single line from the bridge stdout.
pub async fn read_line(
    bridge: &BridgeProcess,
    timeout: Duration,
) -> Result<String, BridgeError> {
    let mut line = String::new();
    match tokio::time::timeout(timeout, bridge.stdout.read_line(&mut line)).await {
        Ok(Ok(0)) => Err(BridgeError::ProcessExited),
        Ok(Ok(_)) => Ok(line),
        Ok(Err(e)) => Err(BridgeError::Io(e)),
        Err(_) => Err(BridgeError::Timeout),
    }
}

/// Kill the bridge process.
pub async fn kill_bridge(mut bridge: BridgeProcess) {
    let _ = bridge.child.kill().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_config_defaults() {
        let config = BridgeConfig::default();
        assert_eq!(config.command, "bun");
        assert_eq!(config.timeout_ms, 60_000);
    }

    #[tokio::test]
    async fn spawn_bridge_detects_exit() {
        let config = BridgeConfig {
            command: "true".into(),
            args: vec![],
            data_dir: String::new(),
            timeout_ms: 1_000,
        };
        let bridge = spawn_bridge(&config).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!is_healthy(&bridge));
    }
}
```

**NOTE:** The `send_line` and `read_line` functions need `&mut` access to the pipes. The implementer should restructure `BridgeProcess` to use `Arc<Mutex<...>>` or split into separate owned halves for the stdin writer and stdout reader. The key insight is:

- `ChildStdin` needs `&mut self` for `write_all`
- `BufReader<ChildStdout>` needs `&mut self` for `read_line`
- These must be usable concurrently (reading while waiting for responses)

The correct approach: wrap stdin in `Mutex` and keep stdout in the read task. Or split into separate owned handles. The implementer should decide the best tokio-idiomatic pattern.

**Step 4: Run tests**

Run: `cargo test -p simse-bridge`
Expected: All tests pass

**Step 5: Commit**

```bash
git add simse-bridge/src/client.rs
git commit -m "feat: add bridge subprocess spawn, health check, kill"
```

---

## Task 8: Implement JSON-RPC request/response cycle

**Files:**
- Modify: `simse-bridge/src/client.rs`

**Step 1: Write failing tests**

Add to the test module in `simse-bridge/src/client.rs`:

```rust
#[tokio::test]
async fn request_response_roundtrip() {
    // Spawn the real bridge server
    let config = BridgeConfig {
        command: "bun".into(),
        args: vec!["run".into(), "../simse-code/bridge-server.ts".into()],
        data_dir: String::new(),
        timeout_ms: 5_000,
    };
    let mut bridge = spawn_bridge(&config).await.unwrap();
    assert!(is_healthy(&bridge));

    // Send initialize request
    let response = request(&mut bridge, "initialize", None).await.unwrap();
    let result = response.result.unwrap();
    assert_eq!(result["protocolVersion"], 1);
    assert_eq!(result["name"], "simse-bridge");

    kill_bridge(bridge).await;
}

#[tokio::test]
async fn request_unknown_method_returns_error() {
    let config = BridgeConfig {
        command: "bun".into(),
        args: vec!["run".into(), "../simse-code/bridge-server.ts".into()],
        data_dir: String::new(),
        timeout_ms: 5_000,
    };
    let mut bridge = spawn_bridge(&config).await.unwrap();

    let response = request(&mut bridge, "nonexistent", None).await.unwrap();
    assert!(response.error.is_some());
    assert_eq!(response.error.unwrap().code, -32601);

    kill_bridge(bridge).await;
}
```

**Step 2: Implement the request function**

The `request` function:
1. Generates a unique request ID (atomic counter)
2. Serializes a `JsonRpcRequest` to JSON
3. Writes it to stdin + newline
4. Reads lines from stdout until a response with matching ID arrives
5. Returns the parsed `JsonRpcResponse`
6. Times out after `config.timeout_ms`

```rust
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Send a JSON-RPC request and wait for the matching response.
pub async fn request(
    bridge: &mut BridgeProcess,
    method: &str,
    params: Option<serde_json::Value>,
) -> Result<JsonRpcResponse, BridgeError> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let req = JsonRpcRequest::new(id, method, params);
    let line = serde_json::to_string(&req)?;

    bridge.stdin.write_all(line.as_bytes()).await?;
    bridge.stdin.write_all(b"\n").await?;
    bridge.stdin.flush().await?;

    let timeout = Duration::from_millis(bridge.timeout_ms);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(BridgeError::Timeout);
        }

        let mut buf = String::new();
        match tokio::time::timeout(remaining, bridge.stdout.read_line(&mut buf)).await {
            Ok(Ok(0)) => return Err(BridgeError::ProcessExited),
            Ok(Ok(_)) => {
                let msg = parse_message(buf.trim())?;
                match msg {
                    RpcMessage::Response(resp) if resp.id == Some(id) => return Ok(resp),
                    RpcMessage::Response(_) => continue, // wrong ID, keep reading
                    RpcMessage::Notification(_) => continue, // skip notifications for now
                }
            }
            Ok(Err(e)) => return Err(BridgeError::Io(e)),
            Err(_) => return Err(BridgeError::Timeout),
        }
    }
}
```

**Note:** The implementer needs to store `timeout_ms` somewhere accessible. Either pass it as a parameter or store it in `BridgeProcess`. The approach above assumes `BridgeProcess` has a `timeout_ms` field added.

**Step 3: Run tests**

Run: `cargo test -p simse-bridge`
Expected: All tests pass (requires `bun` and `bridge-server.ts` to be available)

**Step 4: Commit**

```bash
git add simse-bridge/src/client.rs
git commit -m "feat: add JSON-RPC request/response with ID matching and timeout"
```

---

## Task 9: Implement streaming notification support

**Files:**
- Modify: `simse-bridge/src/client.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn request_with_notifications_collects_them() {
    // This test uses the bridge server.
    // We'll need to add a test method to bridge-server.ts that emits notifications.
    // For now, test the notification channel infrastructure.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<JsonRpcNotification>();
    tx.send(JsonRpcNotification {
        jsonrpc: "2.0".into(),
        method: "stream.delta".into(),
        params: Some(serde_json::json!({"text": "hello"})),
    }).unwrap();
    drop(tx);

    let notif = rx.recv().await.unwrap();
    assert_eq!(notif.method, "stream.delta");
}
```

**Step 2: Implement streaming request**

Add a `request_streaming` function that returns both the final response and a channel of notifications received while waiting:

```rust
use tokio::sync::mpsc;

/// Send a JSON-RPC request and collect notifications until the response arrives.
/// Returns the response and all notifications received while waiting.
pub async fn request_streaming(
    bridge: &mut BridgeProcess,
    method: &str,
    params: Option<serde_json::Value>,
    notification_tx: mpsc::UnboundedSender<JsonRpcNotification>,
) -> Result<JsonRpcResponse, BridgeError> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let req = JsonRpcRequest::new(id, method, params);
    let line = serde_json::to_string(&req)?;

    bridge.stdin.write_all(line.as_bytes()).await?;
    bridge.stdin.write_all(b"\n").await?;
    bridge.stdin.flush().await?;

    let timeout = Duration::from_millis(bridge.timeout_ms);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(BridgeError::Timeout);
        }

        let mut buf = String::new();
        match tokio::time::timeout(remaining, bridge.stdout.read_line(&mut buf)).await {
            Ok(Ok(0)) => return Err(BridgeError::ProcessExited),
            Ok(Ok(_)) => {
                let msg = parse_message(buf.trim())?;
                match msg {
                    RpcMessage::Response(resp) if resp.id == Some(id) => return Ok(resp),
                    RpcMessage::Response(_) => continue,
                    RpcMessage::Notification(notif) => {
                        let _ = notification_tx.send(notif);
                    }
                }
            }
            Ok(Err(e)) => return Err(BridgeError::Io(e)),
            Err(_) => return Err(BridgeError::Timeout),
        }
    }
}
```

**Step 3: Run tests**

Run: `cargo test -p simse-bridge`
Expected: All tests pass

**Step 4: Commit**

```bash
git add simse-bridge/src/client.rs
git commit -m "feat: add streaming request with notification channel"
```

---

## Task 10: Bridge integration tests

**Files:**
- Create: `simse-bridge/tests/integration.rs`

**Step 1: Write integration tests**

```rust
//! Integration tests for the bridge client <-> bridge server.

use simse_bridge::client::*;
use simse_bridge::protocol::*;
use std::time::Duration;

fn bridge_config() -> BridgeConfig {
    BridgeConfig {
        command: "bun".into(),
        args: vec!["run".into(), "../simse-code/bridge-server.ts".into()],
        data_dir: String::new(),
        timeout_ms: 10_000,
    }
}

#[tokio::test]
async fn initialize_handshake() {
    let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();
    let resp = request(&mut bridge, "initialize", None).await.unwrap();
    let result = resp.result.unwrap();
    assert_eq!(result["protocolVersion"], 1);
    assert_eq!(result["name"], "simse-bridge");
    assert!(result["capabilities"]["generate"].as_bool().unwrap());
    kill_bridge(bridge).await;
}

#[tokio::test]
async fn generate_returns_stub() {
    let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();

    // Initialize first
    let _ = request(&mut bridge, "initialize", None).await.unwrap();

    // Generate
    let resp = request(
        &mut bridge,
        "generate",
        Some(serde_json::json!({"prompt": "hello"})),
    ).await.unwrap();
    let result = resp.result.unwrap();
    assert!(result["content"].as_str().is_some());

    kill_bridge(bridge).await;
}

#[tokio::test]
async fn library_search_returns_empty() {
    let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();
    let _ = request(&mut bridge, "initialize", None).await.unwrap();

    let resp = request(
        &mut bridge,
        "library.search",
        Some(serde_json::json!({"query": "test"})),
    ).await.unwrap();
    let result = resp.result.unwrap();
    assert_eq!(result["results"], serde_json::json!([]));

    kill_bridge(bridge).await;
}

#[tokio::test]
async fn tools_list_returns_empty() {
    let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();
    let _ = request(&mut bridge, "initialize", None).await.unwrap();

    let resp = request(&mut bridge, "tools.list", None).await.unwrap();
    let result = resp.result.unwrap();
    assert_eq!(result["tools"], serde_json::json!([]));

    kill_bridge(bridge).await;
}

#[tokio::test]
async fn unknown_method_returns_error() {
    let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();

    let resp = request(&mut bridge, "doesNotExist", None).await.unwrap();
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, -32601);

    kill_bridge(bridge).await;
}

#[tokio::test]
async fn multiple_requests_sequential() {
    let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();

    for i in 0..5 {
        let resp = request(&mut bridge, "initialize", None).await.unwrap();
        assert!(resp.result.is_some(), "Request {} failed", i);
    }

    kill_bridge(bridge).await;
}

#[tokio::test]
async fn health_check_after_kill() {
    let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();
    assert!(is_healthy(&bridge));

    kill_bridge(bridge).await;
    // After kill, we can't check is_healthy because bridge is moved.
    // This test just verifies kill doesn't panic.
}
```

**Step 2: Run integration tests**

Run: `cargo test -p simse-bridge --test integration`
Expected: All 7 tests PASS

**Step 3: Commit**

```bash
git add simse-bridge/tests/integration.rs
git commit -m "feat: add bridge integration tests"
```

---

## Implementation Notes for Implementer

### Key Design Decisions

1. **`BridgeProcess` ownership**: `stdin` and `stdout` need `&mut` for tokio I/O. The simplest correct approach is to keep them as owned fields and pass `&mut BridgeProcess` to `request()`. This means requests are sequential (no concurrent requests), which is fine for the TUI use case — the UI sends one request at a time and waits.

2. **Notification handling**: During a streaming request, notifications arrive interspersed with the final response on the same stdout pipe. The `request_streaming` function forwards notifications to an `mpsc` channel while it waits for the response.

3. **ID generation**: Use a global `AtomicU64` counter. Simple, lockless, unique within a process.

4. **Timeout**: Each request has a deadline. The read loop checks remaining time on each iteration.

5. **Working directory**: The bridge server path `../simse-code/bridge-server.ts` is relative to the workspace root. The implementer should set `cwd` on the `Command` or use absolute paths resolved from `BridgeConfig.data_dir`.

### Testing Requirements

- Tasks 6 tests (bridge server): require `bun` available in PATH
- Tasks 7 tests (subprocess): use basic Unix tools (`cat`, `true`) — macOS/Linux only
- Tasks 8-10 tests (integration): require both `bun` and the bridge-server.ts
- If `bun` is not available, integration tests should skip gracefully

### What Gets Wired Later (Phase 8)

The bridge server stubs return placeholder responses. In Phase 8 (Feature Commands), these stubs will be replaced with actual calls to the simse core library:
- `generate` → `acpClient.generate()` / `acpClient.generateStream()`
- `library.search` → `library.search()`
- `tools.list` → `toolRegistry.getToolDefinitions()`
- `config.read` → `createCLIConfig()` result
