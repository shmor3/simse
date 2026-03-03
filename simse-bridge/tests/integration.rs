//! Integration tests for the bridge client <-> bridge server roundtrip.
//!
//! These tests require `bun` to be available in PATH and the bridge-server.ts
//! to exist at ../simse-code/bridge-server.ts relative to the workspace root.

use simse_bridge::client::*;

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
	kill_bridge(bridge).await;
}

#[tokio::test]
async fn generate_returns_stub_response() {
	let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();
	let _ = request(&mut bridge, "initialize", None).await.unwrap();

	let resp = request(
		&mut bridge,
		"generate",
		Some(serde_json::json!({"prompt": "hello world"})),
	)
	.await
	.unwrap();
	let result = resp.result.unwrap();
	assert!(result["content"].is_array());
	assert!(result["content"][0]["text"].as_str().is_some());
	assert_eq!(result["stopReason"], "end_turn");
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
	)
	.await
	.unwrap();
	let result = resp.result.unwrap();
	assert_eq!(result["results"], serde_json::json!([]));
	kill_bridge(bridge).await;
}

#[tokio::test]
async fn library_add_returns_stub() {
	let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();
	let _ = request(&mut bridge, "initialize", None).await.unwrap();

	let resp = request(
		&mut bridge,
		"library.add",
		Some(serde_json::json!({"text": "some knowledge", "metadata": {}})),
	)
	.await
	.unwrap();
	assert!(resp.result.is_some());
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
async fn config_read_returns_stub() {
	let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();
	let _ = request(&mut bridge, "initialize", None).await.unwrap();

	let resp = request(&mut bridge, "config.read", None).await.unwrap();
	assert!(resp.result.is_some());
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

	// Send different method calls sequentially
	let r1 = request(&mut bridge, "initialize", None).await.unwrap();
	assert!(r1.result.is_some());

	let r2 = request(&mut bridge, "tools.list", None).await.unwrap();
	assert!(r2.result.is_some());

	let r3 = request(
		&mut bridge,
		"library.search",
		Some(serde_json::json!({"query": "test"})),
	)
	.await
	.unwrap();
	assert!(r3.result.is_some());

	let r4 = request(&mut bridge, "config.read", None).await.unwrap();
	assert!(r4.result.is_some());

	kill_bridge(bridge).await;
}

#[tokio::test]
async fn session_operations_return_stubs() {
	let mut bridge = spawn_bridge(&bridge_config()).await.unwrap();
	let _ = request(&mut bridge, "initialize", None).await.unwrap();

	let load = request(
		&mut bridge,
		"session.load",
		Some(serde_json::json!({"sessionId": "test-123"})),
	)
	.await
	.unwrap();
	assert!(load.result.is_some());

	let save = request(
		&mut bridge,
		"session.save",
		Some(serde_json::json!({"sessionId": "test-123", "messages": []})),
	)
	.await
	.unwrap();
	assert!(save.result.is_some());

	kill_bridge(bridge).await;
}
