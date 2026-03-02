//! Integration tests for agentic loop JSON-RPC handlers.
//!
//! Tests the loop/run and loop/cancel dispatch.
//! Full loop execution requires callback-based AcpClient and ToolExecutor,
//! so we verify dispatch routing and parameter validation.

use simse_core::rpc_protocol::JsonRpcRequest;
use simse_core::rpc_server::CoreRpcServer;
use simse_core::rpc_transport::NdjsonTransport;

// ---------------------------------------------------------------------------
// Helper: build an initialized server
// ---------------------------------------------------------------------------

fn make_server() -> CoreRpcServer {
	let transport = NdjsonTransport::new();
	CoreRpcServer::new(transport)
}

async fn make_initialized_server() -> CoreRpcServer {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 0,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	server
}

// ---------------------------------------------------------------------------
// RPC dispatch tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rpc_loop_run_requires_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "loop/run".to_string(),
			params: serde_json::json!({
				"sessionId": "test_session",
			}),
		})
		.await;
	// Should write NOT_INITIALIZED error (output goes to stdout, we just verify no panic)
}

#[tokio::test]
async fn rpc_loop_cancel_requires_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "loop/cancel".to_string(),
			params: serde_json::json!({
				"loopId": "test_loop",
			}),
		})
		.await;
}

#[tokio::test]
async fn rpc_loop_run_invalid_params() {
	let mut server = make_initialized_server().await;
	// Missing required "sessionId" field
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "loop/run".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn rpc_loop_run_session_not_found() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "loop/run".to_string(),
			params: serde_json::json!({
				"sessionId": "nonexistent_session",
			}),
		})
		.await;
	// Should write SESSION_NOT_FOUND error
}

#[tokio::test]
async fn rpc_loop_run_with_options() {
	let mut server = make_initialized_server().await;

	// Create a session first
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "session/create".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// We can't easily get the session ID from stdout output,
	// but we verify the handler path doesn't panic with full options
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "loop/run".to_string(),
			params: serde_json::json!({
				"sessionId": "nonexistent",
				"maxTurns": 5,
				"autoCompact": true,
				"compactionPrompt": "Summarize everything",
				"maxIdenticalToolCalls": 2,
				"systemPrompt": "You are a helpful assistant",
			}),
		})
		.await;
}

#[tokio::test]
async fn rpc_loop_cancel_nonexistent_loop() {
	let mut server = make_initialized_server().await;
	// Cancelling a non-existent loop should not panic, just return OK
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "loop/cancel".to_string(),
			params: serde_json::json!({
				"loopId": "nonexistent_loop",
			}),
		})
		.await;
}

#[tokio::test]
async fn rpc_loop_cancel_invalid_params() {
	let mut server = make_initialized_server().await;
	// Missing required "loopId" field
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "loop/cancel".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}
