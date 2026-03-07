//! Integration tests for session JSON-RPC handlers.
//!
//! Since the `NdjsonTransport` writes to stdout (which we cannot easily
//! capture in tests), we test the handler logic by exercising the
//! `SessionManager` API through a `CoreContext` and verifying the dispatch
//! routing via `CoreRpcServer` construction.

use simse_core::config::AppConfig;
use simse_core::context::CoreContext;
use simse_core::rpc_protocol::JsonRpcRequest;
use simse_core::rpc_server::CoreRpcServer;
use simse_core::rpc_transport::NdjsonTransport;
use simse_core::server::session::SessionStatus;

// ---------------------------------------------------------------------------
// Helper: build an initialized server
// ---------------------------------------------------------------------------

fn make_server() -> CoreRpcServer {
	let transport = NdjsonTransport::new();
	CoreRpcServer::new(transport)
}

// ---------------------------------------------------------------------------
// SessionManager integration (verifies the API the handlers call)
// ---------------------------------------------------------------------------

#[test]
fn session_create_returns_prefixed_id() {
	let ctx = CoreContext::new(AppConfig::default());
	let id = ctx.session_manager.create();
	assert!(id.starts_with("sess_"), "ID should start with sess_");
}

#[test]
fn session_get_info_after_create() {
	let ctx = CoreContext::new(AppConfig::default());
	let id = ctx.session_manager.create();

	let info = ctx.session_manager.get_info(&id).expect("session should exist");
	assert_eq!(info.id, id);
	assert_eq!(info.status, SessionStatus::Active);
	assert_eq!(info.message_count, 0);
	assert!(info.created_at > 0);
	assert!(info.updated_at > 0);
}

#[test]
fn session_get_info_nonexistent() {
	let ctx = CoreContext::new(AppConfig::default());
	assert!(ctx.session_manager.get_info("nonexistent").is_none());
}

#[test]
fn session_list_empty() {
	let ctx = CoreContext::new(AppConfig::default());
	assert!(ctx.session_manager.list().is_empty());
}

#[test]
fn session_list_multiple() {
	let ctx = CoreContext::new(AppConfig::default());
	let id1 = ctx.session_manager.create();
	let id2 = ctx.session_manager.create();

	let list = ctx.session_manager.list();
	assert_eq!(list.len(), 2);

	let ids: Vec<&str> = list.iter().map(|s| s.id.as_str()).collect();
	assert!(ids.contains(&id1.as_str()));
	assert!(ids.contains(&id2.as_str()));
}

#[test]
fn session_delete_existing() {
	let ctx = CoreContext::new(AppConfig::default());
	let id = ctx.session_manager.create();
	assert!(ctx.session_manager.delete(&id));
	assert!(ctx.session_manager.get_info(&id).is_none());
}

#[test]
fn session_delete_nonexistent() {
	let ctx = CoreContext::new(AppConfig::default());
	assert!(!ctx.session_manager.delete("nonexistent"));
}

#[test]
fn session_update_status_to_completed() {
	let ctx = CoreContext::new(AppConfig::default());
	let id = ctx.session_manager.create();

	assert!(ctx.session_manager.update_status(&id, SessionStatus::Completed));

	let info = ctx.session_manager.get_info(&id).unwrap();
	assert_eq!(info.status, SessionStatus::Completed);
}

#[test]
fn session_update_status_to_aborted() {
	let ctx = CoreContext::new(AppConfig::default());
	let id = ctx.session_manager.create();

	assert!(ctx.session_manager.update_status(&id, SessionStatus::Aborted));

	let info = ctx.session_manager.get_info(&id).unwrap();
	assert_eq!(info.status, SessionStatus::Aborted);
}

#[test]
fn session_update_status_nonexistent() {
	let ctx = CoreContext::new(AppConfig::default());
	assert!(!ctx.session_manager.update_status("nonexistent", SessionStatus::Completed));
}

#[test]
fn session_fork_clones_conversation() {
	let ctx = CoreContext::new(AppConfig::default());
	let id = ctx.session_manager.create();

	// Add messages to the original session
	ctx.session_manager.with_state_transition(&id, |conv| {
		let conv = conv.add_user("hello");
		let conv = conv.add_assistant("world");
		(conv, ())
	});

	let forked_id = ctx.session_manager.fork(&id).expect("fork should succeed");

	// Forked session should have a different ID
	assert_ne!(id, forked_id);
	assert!(forked_id.starts_with("sess_"));

	// Forked session should have the same message count
	let info = ctx.session_manager.get_info(&forked_id).unwrap();
	assert_eq!(info.message_count, 2);
	assert_eq!(info.status, SessionStatus::Active);
}

#[test]
fn session_fork_nonexistent() {
	let ctx = CoreContext::new(AppConfig::default());
	assert!(ctx.session_manager.fork("nonexistent").is_none());
}

#[test]
fn session_fork_independent() {
	let ctx = CoreContext::new(AppConfig::default());
	let id = ctx.session_manager.create();

	ctx.session_manager.with_state_transition(&id, |conv| {
		(conv.add_user("original"), ())
	});

	let forked_id = ctx.session_manager.fork(&id).unwrap();

	// Mutate forked conversation
	ctx.session_manager.with_state_transition(&forked_id, |conv| {
		(conv.add_assistant("forked reply"), ())
	});

	// Original should be unchanged
	let original_info = ctx.session_manager.get_info(&id).unwrap();
	assert_eq!(original_info.message_count, 1);

	// Forked should have the extra message
	let forked_info = ctx.session_manager.get_info(&forked_id).unwrap();
	assert_eq!(forked_info.message_count, 2);
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — verify handlers can be invoked without panic
// ---------------------------------------------------------------------------
//
// These tests construct a `CoreRpcServer` and call dispatch with session
// method names. Since transport writes to stdout, we verify no panics
// and that the server state is updated correctly.

#[tokio::test]
async fn dispatch_session_create_before_init_writes_error() {
	// Server is NOT initialized — handlers should call write_not_initialized
	// and return without panic.
	let mut server = make_server();
	let req = JsonRpcRequest {
		id: 1,
		method: "session/create".to_string(),
		params: serde_json::json!({}),
	};
	// This will write an error to stdout (not initialized), but should not panic.
	server.dispatch(req).await;
}

#[tokio::test]
async fn dispatch_session_get_before_init_writes_error() {
	let mut server = make_server();
	let req = JsonRpcRequest {
		id: 2,
		method: "session/get".to_string(),
		params: serde_json::json!({ "id": "nonexistent" }),
	};
	server.dispatch(req).await;
}

#[tokio::test]
async fn dispatch_session_list_before_init_writes_error() {
	let mut server = make_server();
	let req = JsonRpcRequest {
		id: 3,
		method: "session/list".to_string(),
		params: serde_json::json!({}),
	};
	server.dispatch(req).await;
}

#[tokio::test]
async fn dispatch_session_delete_before_init_writes_error() {
	let mut server = make_server();
	let req = JsonRpcRequest {
		id: 4,
		method: "session/delete".to_string(),
		params: serde_json::json!({ "id": "nonexistent" }),
	};
	server.dispatch(req).await;
}

#[tokio::test]
async fn dispatch_session_update_status_before_init_writes_error() {
	let mut server = make_server();
	let req = JsonRpcRequest {
		id: 5,
		method: "session/updateStatus".to_string(),
		params: serde_json::json!({ "id": "nonexistent", "status": "completed" }),
	};
	server.dispatch(req).await;
}

#[tokio::test]
async fn dispatch_session_fork_before_init_writes_error() {
	let mut server = make_server();
	let req = JsonRpcRequest {
		id: 6,
		method: "session/fork".to_string(),
		params: serde_json::json!({ "id": "nonexistent" }),
	};
	server.dispatch(req).await;
}

#[tokio::test]
async fn dispatch_session_create_after_init() {
	let mut server = make_server();
	// Initialize first
	let init_req = JsonRpcRequest {
		id: 1,
		method: "core/initialize".to_string(),
		params: serde_json::json!({}),
	};
	server.dispatch(init_req).await;

	// Now create a session — should not panic
	let req = JsonRpcRequest {
		id: 2,
		method: "session/create".to_string(),
		params: serde_json::json!({}),
	};
	server.dispatch(req).await;
}

#[tokio::test]
async fn dispatch_session_list_after_init() {
	let mut server = make_server();
	let init_req = JsonRpcRequest {
		id: 1,
		method: "core/initialize".to_string(),
		params: serde_json::json!({}),
	};
	server.dispatch(init_req).await;

	let req = JsonRpcRequest {
		id: 2,
		method: "session/list".to_string(),
		params: serde_json::json!({}),
	};
	server.dispatch(req).await;
}

#[tokio::test]
async fn dispatch_all_session_methods_after_init() {
	let mut server = make_server();

	// Initialize
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Create
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "session/create".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// List
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "session/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Get (will fail since we don't capture the created ID, but should not panic)
	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "session/get".to_string(),
			params: serde_json::json!({ "id": "nonexistent" }),
		})
		.await;

	// Delete
	server
		.dispatch(JsonRpcRequest {
			id: 5,
			method: "session/delete".to_string(),
			params: serde_json::json!({ "id": "nonexistent" }),
		})
		.await;

	// Update status
	server
		.dispatch(JsonRpcRequest {
			id: 6,
			method: "session/updateStatus".to_string(),
			params: serde_json::json!({ "id": "nonexistent", "status": "completed" }),
		})
		.await;

	// Fork
	server
		.dispatch(JsonRpcRequest {
			id: 7,
			method: "session/fork".to_string(),
			params: serde_json::json!({ "id": "nonexistent" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_session_update_status_invalid_status() {
	let mut server = make_server();

	// Initialize
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Update with invalid status — should not panic
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "session/updateStatus".to_string(),
			params: serde_json::json!({ "id": "x", "status": "invalid_status" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_session_get_missing_params() {
	let mut server = make_server();

	// Initialize
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "core/initialize".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Get with no params — should return INVALID_PARAMS error, not panic
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "session/get".to_string(),
			params: serde_json::json!(null),
		})
		.await;
}

// ---------------------------------------------------------------------------
// SessionManager via CoreContext — verifies the wiring
// ---------------------------------------------------------------------------

#[test]
fn core_context_session_manager_is_functional() {
	let ctx = CoreContext::new(AppConfig::default());

	// Full lifecycle through the context's session manager
	let id = ctx.session_manager.create();
	assert!(ctx.session_manager.get_info(&id).is_some());
	assert_eq!(ctx.session_manager.list().len(), 1);

	ctx.session_manager.update_status(&id, SessionStatus::Completed);
	assert_eq!(
		ctx.session_manager.get_info(&id).unwrap().status,
		SessionStatus::Completed
	);

	let forked_id = ctx.session_manager.fork(&id).unwrap();
	assert_eq!(ctx.session_manager.list().len(), 2);

	ctx.session_manager.delete(&id);
	assert_eq!(ctx.session_manager.list().len(), 1);
	assert!(ctx.session_manager.get_info(&forked_id).is_some());
}

#[test]
fn session_status_serialization() {
	// Verify that SessionStatus serializes to lowercase strings
	let active = serde_json::to_value(SessionStatus::Active).unwrap();
	assert_eq!(active, serde_json::json!("active"));

	let completed = serde_json::to_value(SessionStatus::Completed).unwrap();
	assert_eq!(completed, serde_json::json!("completed"));

	let aborted = serde_json::to_value(SessionStatus::Aborted).unwrap();
	assert_eq!(aborted, serde_json::json!("aborted"));
}

#[test]
fn session_status_deserialization() {
	let active: SessionStatus = serde_json::from_value(serde_json::json!("active")).unwrap();
	assert_eq!(active, SessionStatus::Active);

	let completed: SessionStatus =
		serde_json::from_value(serde_json::json!("completed")).unwrap();
	assert_eq!(completed, SessionStatus::Completed);

	let aborted: SessionStatus = serde_json::from_value(serde_json::json!("aborted")).unwrap();
	assert_eq!(aborted, SessionStatus::Aborted);
}
