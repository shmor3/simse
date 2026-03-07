//! Integration tests for task JSON-RPC handlers.
//!
//! Tests follow the same pattern as `rpc_session.rs`: exercise the `TaskList`
//! API through a `CoreContext`, and verify dispatch routing via `CoreRpcServer`.

use std::collections::HashMap;

use simse_core::config::AppConfig;
use simse_core::context::CoreContext;
use simse_core::rpc_protocol::JsonRpcRequest;
use simse_core::rpc_server::CoreRpcServer;
use simse_core::rpc_transport::NdjsonTransport;
use simse_core::tasks::{TaskCreateInput, TaskList, TaskListOptions, TaskStatus, TaskUpdateInput};

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
// TaskList integration (verifies the API the handlers call)
// ---------------------------------------------------------------------------

#[test]
fn task_create_returns_auto_id() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, task) = task_list.create(TaskCreateInput {
		subject: "Fix bug".to_string(),
		description: "Fix the login bug".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	ctx.task_list = new_list;
	assert_eq!(task.id, "1");
	assert_eq!(task.subject, "Fix bug");
	assert_eq!(task.description, "Fix the login bug");
	assert_eq!(task.status, TaskStatus::Pending);
	assert!(task.created_at > 0);
	assert!(task.updated_at > 0);
	assert!(task.blocks.is_empty());
	assert!(task.blocked_by.is_empty());
}

#[test]
fn task_create_with_all_fields() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let mut meta = HashMap::new();
	meta.insert("priority".to_string(), serde_json::json!("high"));

	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, task) = task_list.create(TaskCreateInput {
		subject: "Deploy".to_string(),
		description: "Deploy to prod".to_string(),
		active_form: Some("Deploying".to_string()),
		owner: Some("alice".to_string()),
		metadata: Some(meta),
	});
	ctx.task_list = new_list;

	assert_eq!(task.id, "1");
	assert_eq!(task.active_form, Some("Deploying".to_string()));
	assert_eq!(task.owner, Some("alice".to_string()));
	assert_eq!(
		task.metadata.as_ref().unwrap().get("priority").unwrap(),
		&serde_json::json!("high")
	);
}

#[test]
fn task_create_increments_ids() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, t1) = task_list.create(TaskCreateInput {
		subject: "Task 1".to_string(),
		description: "First".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let (new_list, t2) = new_list.create(TaskCreateInput {
		subject: "Task 2".to_string(),
		description: "Second".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	ctx.task_list = new_list;
	assert_eq!(t1.id, "1");
	assert_eq!(t2.id, "2");
}

#[test]
fn task_get_existing() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, task) = task_list.create(TaskCreateInput {
		subject: "Test".to_string(),
		description: "Desc".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	ctx.task_list = new_list;
	let found = ctx.task_list.get(&task.id).expect("task should exist");
	assert_eq!(found.subject, "Test");
}

#[test]
fn task_get_nonexistent() {
	let ctx = CoreContext::new(AppConfig::default());
	assert!(ctx.task_list.get("999").is_none());
}

#[test]
fn task_list_empty() {
	let ctx = CoreContext::new(AppConfig::default());
	assert!(ctx.task_list.list().is_empty());
}

#[test]
fn task_list_multiple() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "A".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let (new_list, _) = new_list.create(TaskCreateInput {
		subject: "B".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	ctx.task_list = new_list;
	assert_eq!(ctx.task_list.list().len(), 2);
}

#[test]
fn task_list_available_filters_correctly() {
	let mut ctx = CoreContext::new(AppConfig::default());

	// Task 1: pending, no owner => available
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "Available".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});

	// Task 2: pending, has owner => NOT available
	let (new_list, _) = new_list.create(TaskCreateInput {
		subject: "Owned".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: Some("bob".to_string()),
		metadata: None,
	});

	// Task 3: in_progress => NOT available
	let (new_list, _) = new_list.create(TaskCreateInput {
		subject: "Working".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let (new_list, _) = new_list
		.update(
			"3",
			TaskUpdateInput {
				status: Some(TaskStatus::InProgress),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	let available = ctx.task_list.list_available();
	assert_eq!(available.len(), 1);
	assert_eq!(available[0].subject, "Available");
}

#[test]
fn task_update_status() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "Work".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});

	let (new_list, updated) = new_list
		.update(
			"1",
			TaskUpdateInput {
				status: Some(TaskStatus::InProgress),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	assert_eq!(updated.unwrap().status, TaskStatus::InProgress);
}

#[test]
fn task_update_fields() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "Original".to_string(),
		description: "Old desc".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});

	let (new_list, updated) = new_list
		.update(
			"1",
			TaskUpdateInput {
				subject: Some("Updated".to_string()),
				description: Some("New desc".to_string()),
				active_form: Some("Working on it".to_string()),
				owner: Some("charlie".to_string()),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	let updated = updated.unwrap();
	assert_eq!(updated.subject, "Updated");
	assert_eq!(updated.description, "New desc");
	assert_eq!(updated.active_form, Some("Working on it".to_string()));
	assert_eq!(updated.owner, Some("charlie".to_string()));
}

#[test]
fn task_update_metadata_merge() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let mut initial_meta = HashMap::new();
	initial_meta.insert("key1".to_string(), serde_json::json!("val1"));
	initial_meta.insert("key2".to_string(), serde_json::json!("val2"));

	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "Meta".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: Some(initial_meta),
	});

	// Merge: update key1, remove key2 (null), add key3
	let mut update_meta = HashMap::new();
	update_meta.insert("key1".to_string(), serde_json::json!("updated"));
	update_meta.insert("key2".to_string(), serde_json::Value::Null);
	update_meta.insert("key3".to_string(), serde_json::json!("new"));

	let (new_list, updated) = new_list
		.update(
			"1",
			TaskUpdateInput {
				metadata: Some(update_meta),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	let meta = updated.unwrap().metadata.unwrap();
	assert_eq!(meta.get("key1").unwrap(), &serde_json::json!("updated"));
	assert!(meta.get("key2").is_none());
	assert_eq!(meta.get("key3").unwrap(), &serde_json::json!("new"));
}

#[test]
fn task_update_nonexistent() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, result) = task_list
		.update(
			"999",
			TaskUpdateInput {
				subject: Some("nope".to_string()),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;
	assert!(result.is_none());
}

#[test]
fn task_update_add_dependencies() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "Blocker".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let (new_list, _) = new_list.create(TaskCreateInput {
		subject: "Blocked".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});

	// Task 2 is blocked by task 1
	let (new_list, updated) = new_list
		.update(
			"2",
			TaskUpdateInput {
				add_blocked_by: Some(vec!["1".to_string()]),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	assert!(updated.unwrap().blocked_by.contains(&"1".to_string()));

	// Task 1 should now have "2" in its blocks (reciprocal)
	let blocker = ctx.task_list.get("1").unwrap();
	assert!(blocker.blocks.contains(&"2".to_string()));
}

#[test]
fn task_update_circular_dependency_error() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "A".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let (new_list, _) = new_list.create(TaskCreateInput {
		subject: "B".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});

	// A blocks B
	let (new_list, _) = new_list
		.update(
			"1",
			TaskUpdateInput {
				add_blocks: Some(vec!["2".to_string()]),
				..Default::default()
			},
		)
		.unwrap();

	// B blocks A => circular dependency
	let result = new_list.update(
		"2",
		TaskUpdateInput {
			add_blocks: Some(vec!["1".to_string()]),
			..Default::default()
		},
	);

	assert!(result.is_err());
	let err = result.unwrap_err();
	assert_eq!(err.code(), "TASK_CIRCULAR_DEPENDENCY");
}

#[test]
fn task_delete_existing() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "Deleteme".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let (new_list, deleted) = new_list.delete("1");
	ctx.task_list = new_list;
	assert!(deleted);
	assert!(ctx.task_list.get("1").is_none());
	assert_eq!(ctx.task_list.list().len(), 0);
}

#[test]
fn task_delete_nonexistent() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, deleted) = task_list.delete("999");
	ctx.task_list = new_list;
	assert!(!deleted);
}

#[test]
fn task_delete_cleans_up_dependencies() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "A".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let (new_list, _) = new_list.create(TaskCreateInput {
		subject: "B".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});

	// A blocks B
	let (new_list, _) = new_list
		.update(
			"1",
			TaskUpdateInput {
				add_blocks: Some(vec!["2".to_string()]),
				..Default::default()
			},
		)
		.unwrap();

	// Delete A — B's blocked_by should be cleaned up
	let (new_list, _) = new_list.delete("1");
	ctx.task_list = new_list;
	let b = ctx.task_list.get("2").unwrap();
	assert!(b.blocked_by.is_empty());
}

#[test]
fn task_create_checked_limit_reached() {
	let mut ctx = CoreContext::new(AppConfig::default());
	// Replace task_list with a limited one
	ctx.task_list = TaskList::new(Some(TaskListOptions {
		max_tasks: Some(2),
	}));

	let task_list = std::mem::replace(
		&mut ctx.task_list,
		TaskList::new(Some(TaskListOptions {
			max_tasks: Some(2),
		})),
	);
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "1".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let (new_list, _) = new_list.create(TaskCreateInput {
		subject: "2".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});

	let result = new_list.create_checked(TaskCreateInput {
		subject: "3".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});

	assert!(result.is_err());
	let err = result.unwrap_err();
	assert_eq!(err.code(), "TASK_LIMIT_REACHED");
}

#[test]
fn task_item_serialization_camel_case() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, task) = task_list.create(TaskCreateInput {
		subject: "Test".to_string(),
		description: "Desc".to_string(),
		active_form: Some("Testing".to_string()),
		owner: Some("user1".to_string()),
		metadata: None,
	});
	ctx.task_list = new_list;

	let json = serde_json::to_value(&task).unwrap();
	// Verify camelCase field names
	assert!(json.get("id").is_some());
	assert!(json.get("subject").is_some());
	assert!(json.get("description").is_some());
	assert!(json.get("status").is_some());
	assert!(json.get("activeForm").is_some());
	assert!(json.get("owner").is_some());
	assert!(json.get("blocks").is_some());
	assert!(json.get("blockedBy").is_some());
	assert!(json.get("createdAt").is_some());
	assert!(json.get("updatedAt").is_some());

	// Verify snake_case names are NOT present
	assert!(json.get("active_form").is_none());
	assert!(json.get("blocked_by").is_none());
	assert!(json.get("created_at").is_none());
	assert!(json.get("updated_at").is_none());
}

#[test]
fn task_status_serialization() {
	assert_eq!(
		serde_json::to_value(TaskStatus::Pending).unwrap(),
		serde_json::json!("pending")
	);
	assert_eq!(
		serde_json::to_value(TaskStatus::InProgress).unwrap(),
		serde_json::json!("in_progress")
	);
	assert_eq!(
		serde_json::to_value(TaskStatus::Completed).unwrap(),
		serde_json::json!("completed")
	);
	assert_eq!(
		serde_json::to_value(TaskStatus::Deleted).unwrap(),
		serde_json::json!("deleted")
	);
}

#[test]
fn task_completing_unblocks_dependents() {
	let mut ctx = CoreContext::new(AppConfig::default());
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.create(TaskCreateInput {
		subject: "Blocker".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let (new_list, _) = new_list.create(TaskCreateInput {
		subject: "Blocked".to_string(),
		description: "".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});

	// Task 1 blocks task 2
	let (new_list, _) = new_list
		.update(
			"1",
			TaskUpdateInput {
				add_blocks: Some(vec!["2".to_string()]),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	// Task 2 should NOT be available (blocked by task 1)
	assert!(ctx.task_list.list_available().iter().all(|t| t.id != "2"));

	// Complete task 1 — should unblock task 2
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list
		.update(
			"1",
			TaskUpdateInput {
				status: Some(TaskStatus::Completed),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	let task2 = ctx.task_list.get("2").unwrap();
	assert!(task2.blocked_by.is_empty());

	// Task 2 should now be available
	let available = ctx.task_list.list_available();
	assert!(available.iter().any(|t| t.id == "2"));
}

// ---------------------------------------------------------------------------
// Dispatch routing tests — verify handlers can be invoked without panic
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_task_create_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Test",
				"description": "Desc"
			}),
		})
		.await;
	// Should write not-initialized error, not panic
}

#[tokio::test]
async fn dispatch_task_get_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/get".to_string(),
			params: serde_json::json!({ "id": "1" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_list_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_list_available_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/listAvailable".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_update_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/update".to_string(),
			params: serde_json::json!({ "id": "1", "status": "completed" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_delete_before_init() {
	let mut server = make_server();
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/delete".to_string(),
			params: serde_json::json!({ "id": "1" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_create_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Test task",
				"description": "A test task"
			}),
		})
		.await;
	// Should succeed without panic
}

#[tokio::test]
async fn dispatch_task_create_with_all_fields() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Deploy",
				"description": "Deploy to production",
				"activeForm": "Deploying",
				"owner": "alice",
				"metadata": { "env": "prod" }
			}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_create_missing_required_fields() {
	let mut server = make_initialized_server().await;
	// Missing "description" — should return INVALID_PARAMS error, not panic
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({ "subject": "Incomplete" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_get_after_init() {
	let mut server = make_initialized_server().await;
	// Create a task first
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Test",
				"description": "Desc"
			}),
		})
		.await;

	// Get it
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "task/get".to_string(),
			params: serde_json::json!({ "id": "1" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_get_nonexistent() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/get".to_string(),
			params: serde_json::json!({ "id": "999" }),
		})
		.await;
	// Should write TASK_NOT_FOUND error, not panic
}

#[tokio::test]
async fn dispatch_task_list_after_init() {
	let mut server = make_initialized_server().await;
	// Create some tasks
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "A",
				"description": ""
			}),
		})
		.await;
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "B",
				"description": ""
			}),
		})
		.await;

	// List them
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "task/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_list_available_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Available",
				"description": ""
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "task/listAvailable".to_string(),
			params: serde_json::json!({}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_update_status_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Work",
				"description": ""
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "task/update".to_string(),
			params: serde_json::json!({
				"id": "1",
				"status": "in_progress"
			}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_update_fields_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Old",
				"description": "Old desc"
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "task/update".to_string(),
			params: serde_json::json!({
				"id": "1",
				"subject": "New",
				"description": "New desc",
				"activeForm": "Working",
				"owner": "bob",
				"metadata": { "key": "value" }
			}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_update_invalid_status() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "X",
				"description": ""
			}),
		})
		.await;

	// Invalid status string — should return error, not panic
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "task/update".to_string(),
			params: serde_json::json!({
				"id": "1",
				"status": "invalid_status"
			}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_update_nonexistent() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/update".to_string(),
			params: serde_json::json!({
				"id": "999",
				"subject": "nope"
			}),
		})
		.await;
	// Should write TASK_NOT_FOUND error, not panic
}

#[tokio::test]
async fn dispatch_task_update_dependencies() {
	let mut server = make_initialized_server().await;

	// Create two tasks
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Blocker",
				"description": ""
			}),
		})
		.await;
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Blocked",
				"description": ""
			}),
		})
		.await;

	// Task 2 blocked by task 1
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "task/update".to_string(),
			params: serde_json::json!({
				"id": "2",
				"addBlockedBy": ["1"]
			}),
		})
		.await;

	// Task 1 add blocks task 2
	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "task/update".to_string(),
			params: serde_json::json!({
				"id": "1",
				"addBlocks": ["2"]
			}),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_delete_after_init() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Delete me",
				"description": ""
			}),
		})
		.await;

	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "task/delete".to_string(),
			params: serde_json::json!({ "id": "1" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_delete_nonexistent() {
	let mut server = make_initialized_server().await;
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/delete".to_string(),
			params: serde_json::json!({ "id": "999" }),
		})
		.await;
	// Should return { deleted: false }, not panic
}

#[tokio::test]
async fn dispatch_task_full_lifecycle() {
	let mut server = make_initialized_server().await;

	// Create
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!({
				"subject": "Implement feature",
				"description": "Build the new login flow",
				"activeForm": "Implementing feature"
			}),
		})
		.await;

	// Get
	server
		.dispatch(JsonRpcRequest {
			id: 2,
			method: "task/get".to_string(),
			params: serde_json::json!({ "id": "1" }),
		})
		.await;

	// List
	server
		.dispatch(JsonRpcRequest {
			id: 3,
			method: "task/list".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// List available
	server
		.dispatch(JsonRpcRequest {
			id: 4,
			method: "task/listAvailable".to_string(),
			params: serde_json::json!({}),
		})
		.await;

	// Update to in_progress
	server
		.dispatch(JsonRpcRequest {
			id: 5,
			method: "task/update".to_string(),
			params: serde_json::json!({
				"id": "1",
				"status": "in_progress",
				"owner": "dev1"
			}),
		})
		.await;

	// Update to completed
	server
		.dispatch(JsonRpcRequest {
			id: 6,
			method: "task/update".to_string(),
			params: serde_json::json!({
				"id": "1",
				"status": "completed"
			}),
		})
		.await;

	// Delete
	server
		.dispatch(JsonRpcRequest {
			id: 7,
			method: "task/delete".to_string(),
			params: serde_json::json!({ "id": "1" }),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_create_null_params() {
	let mut server = make_initialized_server().await;
	// Null params — should return INVALID_PARAMS error, not panic
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/create".to_string(),
			params: serde_json::json!(null),
		})
		.await;
}

#[tokio::test]
async fn dispatch_task_update_missing_id() {
	let mut server = make_initialized_server().await;
	// Missing "id" field — should return INVALID_PARAMS error
	server
		.dispatch(JsonRpcRequest {
			id: 1,
			method: "task/update".to_string(),
			params: serde_json::json!({ "subject": "no id" }),
		})
		.await;
}

// ---------------------------------------------------------------------------
// CoreContext task_list wiring test
// ---------------------------------------------------------------------------

#[test]
fn core_context_task_list_is_functional() {
	let mut ctx = CoreContext::new(AppConfig::default());

	// Full lifecycle through the context's task list
	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, task) = task_list.create(TaskCreateInput {
		subject: "Wiring test".to_string(),
		description: "Verify task list wiring".to_string(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	ctx.task_list = new_list;
	assert_eq!(task.id, "1");

	assert!(ctx.task_list.get("1").is_some());
	assert_eq!(ctx.task_list.list().len(), 1);
	assert_eq!(ctx.task_list.list_available().len(), 1);

	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list
		.update(
			"1",
			TaskUpdateInput {
				status: Some(TaskStatus::InProgress),
				owner: Some("agent".to_string()),
				..Default::default()
			},
		)
		.unwrap();
	ctx.task_list = new_list;

	let updated = ctx.task_list.get("1").unwrap();
	assert_eq!(updated.status, TaskStatus::InProgress);
	assert_eq!(updated.owner, Some("agent".to_string()));

	// No longer available (in_progress)
	assert!(ctx.task_list.list_available().is_empty());

	let task_list = std::mem::replace(&mut ctx.task_list, TaskList::new(None));
	let (new_list, _) = task_list.delete("1");
	ctx.task_list = new_list;
	assert!(ctx.task_list.list().is_empty());
}
