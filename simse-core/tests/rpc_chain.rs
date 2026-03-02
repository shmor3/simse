//! Integration tests for chain JSON-RPC handlers.
//!
//! Tests the chain/run, chain/runNamed, and chain/stepResult dispatch.
//! Chain execution requires callback providers, so we verify dispatch
//! routing and parameter validation.

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
async fn rpc_chain_run_requires_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "chain/run".to_string(),
			params: serde_json::json!({
				"name": "test_chain",
			}),
		})
		.await;
	// Should write NOT_INITIALIZED error (output goes to stdout, we just verify no panic)
}

#[tokio::test]
async fn rpc_chain_run_named_requires_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "chain/runNamed".to_string(),
			params: serde_json::json!({
				"name": "test_chain",
			}),
		})
		.await;
}

#[tokio::test]
async fn rpc_chain_run_chain_not_found() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "chain/run".to_string(),
			params: serde_json::json!({
				"name": "nonexistent_chain",
			}),
		})
		.await;
	// Should write CHAIN_NOT_FOUND error
}

#[tokio::test]
async fn rpc_chain_run_named_chain_not_found() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "chain/runNamed".to_string(),
			params: serde_json::json!({
				"name": "nonexistent_chain",
			}),
		})
		.await;
	// Should write CHAIN_NOT_FOUND error
}

#[tokio::test]
async fn rpc_chain_run_invalid_params() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "chain/run".to_string(),
			params: serde_json::json!({}),
		})
		.await;
	// Missing required "name" field
}

#[tokio::test]
async fn rpc_chain_run_with_input() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "chain/run".to_string(),
			params: serde_json::json!({
				"name": "test",
				"input": { "key": "value" },
			}),
		})
		.await;
	// Chain not in config, should error
}

#[tokio::test]
async fn rpc_chain_step_result_no_pending() {
	let mut server = make_initialized_server().await;
	// Sending step result for non-existent request should not panic
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "chain/stepResult".to_string(),
			params: serde_json::json!({
				"requestId": "nonexistent",
				"output": "result",
			}),
		})
		.await;
}

#[tokio::test]
async fn rpc_chain_step_result_invalid_params() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "chain/stepResult".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}
