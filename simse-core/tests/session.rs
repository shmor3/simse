use simse_core::server::session::*;

#[test]
fn test_create_session_returns_valid_id() {
	let mgr = SessionManager::new();
	let id = mgr.create();
	assert!(id.starts_with("sess_"), "ID should start with sess_ prefix");
	// Format: sess_{timestamp}_{counter_base36}
	let parts: Vec<&str> = id.splitn(3, '_').collect();
	assert_eq!(parts.len(), 3, "ID should have 3 underscore-separated parts");
}

#[test]
fn test_get_info_after_creation() {
	let mgr = SessionManager::new();
	let id = mgr.create();
	let info = mgr.get_info(&id);
	assert!(info.is_some());
	let info = info.unwrap();
	assert_eq!(info.id, id);
	assert_eq!(info.status, SessionStatus::Active);
	assert_eq!(info.message_count, 0);
	assert!(info.created_at > 0);
	assert!(info.updated_at > 0);
}

#[test]
fn test_get_info_nonexistent_returns_none() {
	let mgr = SessionManager::new();
	assert!(mgr.get_info("nonexistent").is_none());
}

#[test]
fn test_delete_existing_session() {
	let mgr = SessionManager::new();
	let id = mgr.create();
	assert!(mgr.delete(&id));
	assert!(mgr.get_info(&id).is_none());
}

#[test]
fn test_delete_nonexistent_returns_false() {
	let mgr = SessionManager::new();
	assert!(!mgr.delete("nonexistent"));
}

#[test]
fn test_list_sessions() {
	let mgr = SessionManager::new();
	let id1 = mgr.create();
	let id2 = mgr.create();
	let list = mgr.list();
	assert_eq!(list.len(), 2);
	let ids: Vec<&str> = list.iter().map(|s| s.id.as_str()).collect();
	assert!(ids.contains(&id1.as_str()));
	assert!(ids.contains(&id2.as_str()));
}

#[test]
fn test_list_empty() {
	let mgr = SessionManager::new();
	assert!(mgr.list().is_empty());
}

#[test]
fn test_update_status_active_to_completed() {
	let mgr = SessionManager::new();
	let id = mgr.create();
	assert!(mgr.update_status(&id, SessionStatus::Completed));
	let info = mgr.get_info(&id).unwrap();
	assert_eq!(info.status, SessionStatus::Completed);
}

#[test]
fn test_update_status_active_to_aborted() {
	let mgr = SessionManager::new();
	let id = mgr.create();
	assert!(mgr.update_status(&id, SessionStatus::Aborted));
	let info = mgr.get_info(&id).unwrap();
	assert_eq!(info.status, SessionStatus::Aborted);
}

#[test]
fn test_update_status_nonexistent_returns_false() {
	let mgr = SessionManager::new();
	assert!(!mgr.update_status("nonexistent", SessionStatus::Completed));
}

#[test]
fn test_update_status_updates_timestamp() {
	let mgr = SessionManager::new();
	let id = mgr.create();
	let before = mgr.get_info(&id).unwrap().updated_at;
	// Small sleep to ensure timestamp changes
	std::thread::sleep(std::time::Duration::from_millis(2));
	mgr.update_status(&id, SessionStatus::Completed);
	let after = mgr.get_info(&id).unwrap().updated_at;
	assert!(after >= before);
}

#[test]
fn test_fork_clones_conversation() {
	let mgr = SessionManager::new();
	let id = mgr.create();

	// Add messages to the original session
	mgr.with_state_transition(&id, |conv| {
		let conv = conv.add_user("hello");
		let conv = conv.add_assistant("world");
		(conv, ())
	});

	let forked_id = mgr.fork(&id);
	assert!(forked_id.is_some());
	let forked_id = forked_id.unwrap();

	// Forked session should have a different ID
	assert_ne!(id, forked_id);
	assert!(forked_id.starts_with("sess_"));

	// Forked session should have the same messages
	let forked_count = mgr.with_session(&forked_id, |session| {
		session.conversation.message_count()
	});
	assert_eq!(forked_count, Some(2));

	// Forked session should be active
	let info = mgr.get_info(&forked_id).unwrap();
	assert_eq!(info.status, SessionStatus::Active);
}

#[test]
fn test_fork_nonexistent_returns_none() {
	let mgr = SessionManager::new();
	assert!(mgr.fork("nonexistent").is_none());
}

#[test]
fn test_fork_fresh_event_bus() {
	let mgr = SessionManager::new();
	let id = mgr.create();

	// Subscribe to an event on the original session
	mgr.with_session(&id, |session| {
		let _unsub = session
			.event_bus
			.subscribe("test.event", |_| { /* handler */ });
	});

	let forked_id = mgr.fork(&id).unwrap();

	// Forked session should have a fresh event bus (different from original)
	// We verify by publishing on forked bus and checking it doesn't panic
	mgr.with_session(&forked_id, |session| {
		session
			.event_bus
			.publish("test.event", serde_json::json!({}));
	});
}

#[test]
fn test_multiple_sessions_unique_ids() {
	let mgr = SessionManager::new();
	let mut ids = Vec::new();
	for _ in 0..10 {
		ids.push(mgr.create());
	}
	// All IDs should be unique
	let mut unique = ids.clone();
	unique.sort();
	unique.dedup();
	assert_eq!(unique.len(), ids.len());
}

#[test]
fn test_with_session_mutate_conversation() {
	let mgr = SessionManager::new();
	let id = mgr.create();

	mgr.with_state_transition(&id, |conv| {
		(conv.add_user("test message"), ())
	});

	let info = mgr.get_info(&id).unwrap();
	assert_eq!(info.message_count, 1);
}

#[test]
fn test_with_session_nonexistent_returns_none() {
	let mgr = SessionManager::new();
	let result = mgr.with_session("nonexistent", |_session| 42);
	assert!(result.is_none());
}

#[test]
fn test_fork_preserves_system_prompt() {
	let mgr = SessionManager::new();
	let id = mgr.create();

	mgr.with_state_transition(&id, |conv| {
		let conv = conv.set_system_prompt("you are helpful".into());
		let conv = conv.add_user("hi");
		(conv, ())
	});

	let forked_id = mgr.fork(&id).unwrap();

	let prompt = mgr.with_session(&forked_id, |session| {
		session.conversation.system_prompt().map(|s| s.to_string())
	});
	assert_eq!(prompt, Some(Some("you are helpful".to_string())));
}

#[test]
fn test_fork_independent_conversations() {
	let mgr = SessionManager::new();
	let id = mgr.create();

	mgr.with_state_transition(&id, |conv| {
		(conv.add_user("original"), ())
	});

	let forked_id = mgr.fork(&id).unwrap();

	// Mutate forked conversation
	mgr.with_state_transition(&forked_id, |conv| {
		(conv.add_assistant("forked reply"), ())
	});

	// Original should be unchanged
	let original_count = mgr.with_session(&id, |session| {
		session.conversation.message_count()
	});
	assert_eq!(original_count, Some(1));

	// Forked should have the extra message
	let forked_count = mgr.with_session(&forked_id, |session| {
		session.conversation.message_count()
	});
	assert_eq!(forked_count, Some(2));
}

#[test]
fn test_delete_after_fork_does_not_affect_fork() {
	let mgr = SessionManager::new();
	let id = mgr.create();

	mgr.with_state_transition(&id, |conv| {
		(conv.add_user("hello"), ())
	});

	let forked_id = mgr.fork(&id).unwrap();
	assert!(mgr.delete(&id));

	// Forked session should still exist and have the message
	let count = mgr.with_session(&forked_id, |session| {
		session.conversation.message_count()
	});
	assert_eq!(count, Some(1));
}

#[test]
fn test_default_impl() {
	let mgr = SessionManager::default();
	let id = mgr.create();
	assert!(id.starts_with("sess_"));
}

#[test]
fn test_clone_shares_state() {
	let mgr = SessionManager::new();
	let id = mgr.create();

	let mgr2 = mgr.clone();
	let info = mgr2.get_info(&id);
	assert!(info.is_some());
}
