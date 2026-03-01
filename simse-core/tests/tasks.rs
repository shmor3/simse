use simse_core::tasks::*;

#[test]
fn test_create_task() {
	let mut list = TaskList::new(None);
	let task = list.create(TaskCreateInput {
		subject: "Fix bug".into(),
		description: "Fix the login bug".into(),
		active_form: Some("Fixing bug".into()),
		owner: None,
		metadata: None,
	});
	assert_eq!(task.id, "1");
	assert_eq!(task.status, TaskStatus::Pending);
	assert_eq!(task.subject, "Fix bug");
	assert_eq!(task.description, "Fix the login bug");
	assert_eq!(task.active_form.as_deref(), Some("Fixing bug"));
	assert!(task.blocks.is_empty());
	assert!(task.blocked_by.is_empty());
}

#[test]
fn test_auto_increment_ids() {
	let mut list = TaskList::new(None);
	let t1 = list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let t2 = list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	assert_eq!(t1.id, "1");
	assert_eq!(t2.id, "2");
}

#[test]
fn test_get_task() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "Task".into(),
		description: "Desc".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let found = list.get("1");
	assert!(found.is_some());
	assert_eq!(found.unwrap().subject, "Task");
	assert!(list.get("999").is_none());
}

#[test]
fn test_update_status() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "Task".into(),
		description: "Desc".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let updated = list
		.update(
			"1",
			TaskUpdateInput {
				status: Some(TaskStatus::InProgress),
				..Default::default()
			},
		)
		.unwrap()
		.unwrap();
	assert_eq!(updated.status, TaskStatus::InProgress);
}

#[test]
fn test_update_fields() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "Old".into(),
		description: "Old desc".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let updated = list
		.update(
			"1",
			TaskUpdateInput {
				subject: Some("New".into()),
				description: Some("New desc".into()),
				active_form: Some("Running".into()),
				owner: Some("alice".into()),
				..Default::default()
			},
		)
		.unwrap()
		.unwrap();
	assert_eq!(updated.subject, "New");
	assert_eq!(updated.description, "New desc");
	assert_eq!(updated.active_form.as_deref(), Some("Running"));
	assert_eq!(updated.owner.as_deref(), Some("alice"));
}

#[test]
fn test_update_nonexistent_returns_none() {
	let mut list = TaskList::new(None);
	let result = list
		.update(
			"999",
			TaskUpdateInput {
				status: Some(TaskStatus::InProgress),
				..Default::default()
			},
		)
		.unwrap();
	assert!(result.is_none());
}

#[test]
fn test_dependency_tracking() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.update(
		"2",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["1".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let task2 = list.get("2").unwrap();
	assert_eq!(task2.blocked_by, vec!["1"]);
	let task1 = list.get("1").unwrap();
	assert_eq!(task1.blocks, vec!["2"]);
}

#[test]
fn test_add_blocks_dependency() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	// Task 1 blocks task 2
	list.update(
		"1",
		TaskUpdateInput {
			add_blocks: Some(vec!["2".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let task1 = list.get("1").unwrap();
	assert_eq!(task1.blocks, vec!["2"]);
	let task2 = list.get("2").unwrap();
	assert_eq!(task2.blocked_by, vec!["1"]);
}

#[test]
fn test_circular_dependency_rejected() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.update(
		"2",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["1".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let result = list.update(
		"1",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["2".into()]),
			..Default::default()
		},
	);
	assert!(result.is_err()); // circular dependency
}

#[test]
fn test_circular_dependency_via_add_blocks() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.update(
		"1",
		TaskUpdateInput {
			add_blocks: Some(vec!["2".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	// Now try to make B block A => circular
	let result = list.update(
		"2",
		TaskUpdateInput {
			add_blocks: Some(vec!["1".into()]),
			..Default::default()
		},
	);
	assert!(result.is_err());
}

#[test]
fn test_self_dependency_ignored() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	// Trying to block self should be silently ignored
	let result = list
		.update(
			"1",
			TaskUpdateInput {
				add_blocked_by: Some(vec!["1".into()]),
				..Default::default()
			},
		)
		.unwrap();
	assert!(result.is_some());
	let task = list.get("1").unwrap();
	assert!(task.blocked_by.is_empty());
	assert!(task.blocks.is_empty());
}

#[test]
fn test_duplicate_dependency_ignored() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.update(
		"2",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["1".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	// Add same dependency again
	list.update(
		"2",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["1".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let task2 = list.get("2").unwrap();
	// Should still only have one entry
	assert_eq!(task2.blocked_by.len(), 1);
}

#[test]
fn test_completing_unblocks_dependents() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.update(
		"2",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["1".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	list.update(
		"1",
		TaskUpdateInput {
			status: Some(TaskStatus::Completed),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let task2 = list.get("2").unwrap();
	assert!(task2.blocked_by.is_empty()); // unblocked
}

#[test]
fn test_delete_cleans_up_deps() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.update(
		"2",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["1".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	list.delete("1");
	let task2 = list.get("2").unwrap();
	assert!(task2.blocked_by.is_empty());
}

#[test]
fn test_delete_nonexistent_returns_false() {
	let mut list = TaskList::new(None);
	assert!(!list.delete("999"));
}

#[test]
fn test_delete_removes_task() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	assert!(list.delete("1"));
	assert!(list.get("1").is_none());
	assert_eq!(list.task_count(), 0);
}

#[test]
fn test_list_all() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let all = list.list();
	assert_eq!(all.len(), 2);
}

#[test]
fn test_list_available() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: Some("alice".into()),
		metadata: None,
	});
	let available = list.list_available();
	assert_eq!(available.len(), 1); // B has owner
	assert_eq!(available[0].subject, "A");
}

#[test]
fn test_list_available_excludes_blocked() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.update(
		"2",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["1".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let available = list.list_available();
	assert_eq!(available.len(), 1);
	assert_eq!(available[0].subject, "A");
}

#[test]
fn test_list_available_includes_unblocked_after_completion() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.update(
		"2",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["1".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	list.update(
		"1",
		TaskUpdateInput {
			status: Some(TaskStatus::Completed),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let available = list.list_available();
	// B should now be available (A is completed, B is pending + unblocked)
	assert_eq!(available.len(), 1);
	assert_eq!(available[0].subject, "B");
}

#[test]
fn test_get_blocked() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.update(
		"2",
		TaskUpdateInput {
			add_blocked_by: Some(vec!["1".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let blocked = list.get_blocked();
	assert_eq!(blocked.len(), 1);
	assert_eq!(blocked[0].subject, "B");
}

#[test]
fn test_task_count() {
	let mut list = TaskList::new(None);
	assert_eq!(list.task_count(), 0);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	assert_eq!(list.task_count(), 1);
}

#[test]
fn test_clear() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.clear();
	assert_eq!(list.task_count(), 0);
	// IDs should reset
	let t = list.create(TaskCreateInput {
		subject: "C".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	assert_eq!(t.id, "1");
}

#[test]
fn test_task_limit() {
	let mut list = TaskList::new(Some(TaskListOptions { max_tasks: Some(2) }));
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	let result = list.create_checked(TaskCreateInput {
		subject: "C".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	assert!(result.is_err()); // limit reached
}

#[test]
fn test_create_always_succeeds_under_limit() {
	let mut list = TaskList::new(Some(TaskListOptions { max_tasks: Some(5) }));
	let task = list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	assert_eq!(task.id, "1");
}

#[test]
fn test_metadata_on_create() {
	let mut list = TaskList::new(None);
	let mut meta = std::collections::HashMap::new();
	meta.insert("key1".into(), serde_json::json!("val1"));
	let task = list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: Some(meta),
	});
	let m = task.metadata.as_ref().unwrap();
	assert_eq!(m.get("key1").unwrap(), &serde_json::json!("val1"));
}

#[test]
fn test_metadata_merge() {
	let mut list = TaskList::new(None);
	let mut meta = std::collections::HashMap::new();
	meta.insert("key1".into(), serde_json::json!("val1"));
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: Some(meta),
	});
	let mut new_meta = std::collections::HashMap::new();
	new_meta.insert("key2".into(), serde_json::json!("val2"));
	new_meta.insert("key1".into(), serde_json::Value::Null); // delete key1
	list.update(
		"1",
		TaskUpdateInput {
			metadata: Some(new_meta),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let task = list.get("1").unwrap();
	let meta = task.metadata.as_ref().unwrap();
	assert!(!meta.contains_key("key1")); // deleted
	assert_eq!(meta.get("key2").unwrap(), &serde_json::json!("val2"));
}

#[test]
fn test_metadata_merge_preserves_existing() {
	let mut list = TaskList::new(None);
	let mut meta = std::collections::HashMap::new();
	meta.insert("a".into(), serde_json::json!(1));
	meta.insert("b".into(), serde_json::json!(2));
	list.create(TaskCreateInput {
		subject: "T".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: Some(meta),
	});
	let mut new_meta = std::collections::HashMap::new();
	new_meta.insert("c".into(), serde_json::json!(3));
	list.update(
		"1",
		TaskUpdateInput {
			metadata: Some(new_meta),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	let task = list.get("1").unwrap();
	let meta = task.metadata.as_ref().unwrap();
	assert_eq!(meta.get("a").unwrap(), &serde_json::json!(1));
	assert_eq!(meta.get("b").unwrap(), &serde_json::json!(2));
	assert_eq!(meta.get("c").unwrap(), &serde_json::json!(3));
}

#[test]
fn test_timestamps_set() {
	let mut list = TaskList::new(None);
	let task = list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	assert!(task.created_at > 0);
	assert!(task.updated_at > 0);
	assert_eq!(task.created_at, task.updated_at);
}

#[test]
fn test_transitive_cycle_detection() {
	let mut list = TaskList::new(None);
	// A -> B -> C, then try C -> A which would create a cycle
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "C".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	// A blocks B
	list.update(
		"1",
		TaskUpdateInput {
			add_blocks: Some(vec!["2".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	// B blocks C
	list.update(
		"2",
		TaskUpdateInput {
			add_blocks: Some(vec!["3".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	// C blocks A => cycle
	let result = list.update(
		"3",
		TaskUpdateInput {
			add_blocks: Some(vec!["1".into()]),
			..Default::default()
		},
	);
	assert!(result.is_err());
}

#[test]
fn test_delete_cleans_up_both_directions() {
	let mut list = TaskList::new(None);
	list.create(TaskCreateInput {
		subject: "A".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "B".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	list.create(TaskCreateInput {
		subject: "C".into(),
		description: "".into(),
		active_form: None,
		owner: None,
		metadata: None,
	});
	// A blocks B, B blocks C
	list.update(
		"1",
		TaskUpdateInput {
			add_blocks: Some(vec!["2".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	list.update(
		"2",
		TaskUpdateInput {
			add_blocks: Some(vec!["3".into()]),
			..Default::default()
		},
	)
	.unwrap()
	.unwrap();
	// Delete B: should clean up A's blocks and C's blocked_by
	list.delete("2");
	let task1 = list.get("1").unwrap();
	assert!(task1.blocks.is_empty());
	let task3 = list.get("3").unwrap();
	assert!(task3.blocked_by.is_empty());
}
