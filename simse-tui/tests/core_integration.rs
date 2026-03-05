//! Integration tests verifying simse-core direct dependency works correctly.
//!
//! These tests confirm that simse-tui can use simse-core types directly
//! (without the former simse-bridge intermediary) and that the key APIs
//! used by the TUI are accessible and functional.

use simse_core::agentic_loop::{CancellationToken, LoopCallbacks};
use simse_core::conversation::Conversation;
use simse_core::tools::{ToolCallResult, ToolRegistry, ToolRegistryOptions};

// ═══════════════════════════════════════════════════════════════
// Conversation — type unification
// ═══════════════════════════════════════════════════════════════

#[test]
fn conversation_type_unification() {
	// Verify simse-core Conversation is directly usable from simse-tui tests.
	let conv = Conversation::new(None);
	let _: &Conversation = &conv;
	assert_eq!(conv.message_count(), 0);
}

#[test]
fn conversation_add_messages_and_count() {
	let mut conv = Conversation::new(None);
	conv.add_user("Hello");
	conv.add_assistant("Hi there!");
	assert_eq!(conv.message_count(), 2);
}

// ═══════════════════════════════════════════════════════════════
// ToolRegistry — from simse-core
// ═══════════════════════════════════════════════════════════════

#[test]
fn tool_registry_from_core() {
	let registry = ToolRegistry::new(ToolRegistryOptions::default());
	assert_eq!(registry.tool_count(), 0);
	assert!(registry.get_tool_definitions().is_empty());
}

// ═══════════════════════════════════════════════════════════════
// CancellationToken
// ═══════════════════════════════════════════════════════════════

#[test]
fn cancellation_token_lifecycle() {
	let token = CancellationToken::new();
	assert!(!token.is_cancelled());
	token.cancel();
	assert!(token.is_cancelled());
}

#[test]
fn cancellation_token_clone_shares_state() {
	let token = CancellationToken::new();
	let clone = token.clone();
	assert!(!clone.is_cancelled());
	token.cancel();
	assert!(clone.is_cancelled());
}

// ═══════════════════════════════════════════════════════════════
// LoopCallbacks
// ═══════════════════════════════════════════════════════════════

#[test]
fn loop_callbacks_default() {
	let cb = LoopCallbacks::default();
	assert!(cb.on_stream_start.is_none());
	assert!(cb.on_stream_delta.is_none());
	assert!(cb.on_error.is_none());
	assert!(cb.on_tool_call_start.is_none());
	assert!(cb.on_tool_call_end.is_none());
	assert!(cb.on_turn_complete.is_none());
	assert!(cb.on_usage_update.is_none());
	assert!(cb.on_compaction.is_none());
	assert!(cb.on_doom_loop.is_none());
}

// ═══════════════════════════════════════════════════════════════
// ToolCallResult — diff field
// ═══════════════════════════════════════════════════════════════

#[test]
fn tool_call_result_has_diff_field() {
	let result = ToolCallResult {
		id: "1".into(),
		name: "test".into(),
		output: "ok".into(),
		is_error: false,
		duration_ms: None,
		diff: Some("--- a\n+++ b".into()),
	};
	assert_eq!(result.diff.unwrap(), "--- a\n+++ b");
}

#[test]
fn tool_call_result_diff_none_by_default() {
	let result = ToolCallResult {
		id: "2".into(),
		name: "read_file".into(),
		output: "file contents".into(),
		is_error: false,
		duration_ms: Some(42),
		diff: None,
	};
	assert!(result.diff.is_none());
	assert_eq!(result.duration_ms, Some(42));
}

// ═══════════════════════════════════════════════════════════════
// Config loading — simse-tui module
// ═══════════════════════════════════════════════════════════════

#[test]
fn config_loading_with_defaults() {
	// Test that config loading works with default options.
	let options = simse_tui::config::ConfigOptions::default();
	let config = simse_tui::config::load_config(&options);
	// Should at least have a data_dir.
	assert!(!config.data_dir.as_os_str().is_empty());
}

// ═══════════════════════════════════════════════════════════════
// Session store — simse-tui module
// ═══════════════════════════════════════════════════════════════

#[test]
fn session_store_create_and_list() {
	let dir = tempfile::tempdir().unwrap();
	let store = simse_tui::session_store::SessionStore::new(dir.path());
	let id = store.create("/tmp/test").unwrap();
	let sessions = store.list();
	assert_eq!(sessions.len(), 1);
	assert_eq!(sessions[0].id, id);
}

#[test]
fn session_store_round_trip() {
	let dir = tempfile::tempdir().unwrap();
	let store = simse_tui::session_store::SessionStore::new(dir.path());
	let id = store.create("/tmp/project").unwrap();

	let msg = simse_tui::session_store::SessionMessage {
		role: "user".into(),
		content: "Hello, world!".into(),
		tool_call_id: None,
		tool_name: None,
	};
	store.append(&id, &msg).unwrap();

	let messages = store.load(&id);
	assert_eq!(messages.len(), 1);
	assert_eq!(messages[0].content, "Hello, world!");

	let meta = store.get(&id).unwrap();
	assert_eq!(meta.message_count, 1);
}
