//! Tests for the built-in library, VFS, and task tool registration.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use simse_core::error::SimseError;
use simse_core::tasks::{TaskList, TaskListOptions};
use simse_core::tools::builtin::{
	CompendiumResult, DirEntry, FileReadResult, LibraryStore, SearchResult, TopicInfo, VfsStore,
	VolumeInfo, register_library_tools, register_task_tools, register_vfs_tools,
};
use simse_core::tools::{ToolCallRequest, ToolRegistry, ToolRegistryOptions};

// ---------------------------------------------------------------------------
// Mock LibraryStore
// ---------------------------------------------------------------------------

struct MockLibraryStore {
	search_results: Vec<SearchResult>,
	add_id: String,
	delete_found: bool,
	topics: Vec<TopicInfo>,
	volumes: Vec<VolumeInfo>,
	compendium_result: CompendiumResult,
}

impl Default for MockLibraryStore {
	fn default() -> Self {
		Self {
			search_results: vec![
				SearchResult {
					text: "Rust is great".to_string(),
					topic: Some("programming/rust".to_string()),
					score: 0.95,
				},
				SearchResult {
					text: "Async programming".to_string(),
					topic: Some("programming/async".to_string()),
					score: 0.82,
				},
			],
			add_id: "vol_42".to_string(),
			delete_found: true,
			topics: vec![
				TopicInfo {
					topic: "programming".to_string(),
					entry_count: 10,
				},
				TopicInfo {
					topic: "programming/rust".to_string(),
					entry_count: 5,
				},
				TopicInfo {
					topic: "programming/async".to_string(),
					entry_count: 3,
				},
				TopicInfo {
					topic: "design".to_string(),
					entry_count: 2,
				},
			],
			volumes: vec![
				VolumeInfo {
					id: "v1".to_string(),
				},
				VolumeInfo {
					id: "v2".to_string(),
				},
				VolumeInfo {
					id: "v3".to_string(),
				},
			],
			compendium_result: CompendiumResult {
				compendium_id: "comp_1".to_string(),
				source_ids: vec!["v1".to_string(), "v2".to_string(), "v3".to_string()],
			},
		}
	}
}

#[async_trait]
impl LibraryStore for MockLibraryStore {
	async fn search(&self, _query: &str, max_results: usize) -> Result<Vec<SearchResult>, SimseError> {
		Ok(self.search_results.iter().take(max_results).cloned().collect())
	}

	async fn add(&self, _text: &str, _topic: &str) -> Result<String, SimseError> {
		Ok(self.add_id.clone())
	}

	async fn delete(&self, _id: &str) -> Result<bool, SimseError> {
		Ok(self.delete_found)
	}

	async fn get_topics(&self) -> Result<Vec<TopicInfo>, SimseError> {
		Ok(self.topics.clone())
	}

	async fn filter_by_topic(&self, _topics: &[String]) -> Result<Vec<VolumeInfo>, SimseError> {
		Ok(self.volumes.clone())
	}

	async fn compendium(&self, _ids: &[String]) -> Result<CompendiumResult, SimseError> {
		Ok(self.compendium_result.clone())
	}
}

// ---------------------------------------------------------------------------
// Mock VfsStore
// ---------------------------------------------------------------------------

struct MockVfsStore {
	read_result: FileReadResult,
	write_bytes: usize,
	dir_entries: Vec<DirEntry>,
	tree_output: String,
}

impl Default for MockVfsStore {
	fn default() -> Self {
		Self {
			read_result: FileReadResult {
				text: "Hello, world!".to_string(),
				content_type: "text".to_string(),
				size: 13,
			},
			write_bytes: 42,
			dir_entries: vec![
				DirEntry {
					name: "src".to_string(),
					entry_type: "directory".to_string(),
				},
				DirEntry {
					name: "README.md".to_string(),
					entry_type: "file".to_string(),
				},
				DirEntry {
					name: "Cargo.toml".to_string(),
					entry_type: "file".to_string(),
				},
			],
			tree_output: "vfs:///\n  src/\n    main.rs\n  README.md".to_string(),
		}
	}
}

#[async_trait]
impl VfsStore for MockVfsStore {
	async fn read_file(&self, _path: &str) -> Result<FileReadResult, SimseError> {
		Ok(self.read_result.clone())
	}

	async fn write_file(&self, _path: &str, _content: &str) -> Result<usize, SimseError> {
		Ok(self.write_bytes)
	}

	async fn readdir(&self, _path: &str) -> Result<Vec<DirEntry>, SimseError> {
		Ok(self.dir_entries.clone())
	}

	async fn tree(&self, _path: &str) -> Result<String, SimseError> {
		Ok(self.tree_output.clone())
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_registry() -> ToolRegistry {
	ToolRegistry::new(ToolRegistryOptions::default())
}

fn make_call(name: &str, args: serde_json::Value) -> ToolCallRequest {
	ToolCallRequest {
		id: "test_call".to_string(),
		name: name.to_string(),
		arguments: args,
	}
}

// ===========================================================================
// Library tool tests
// ===========================================================================

// ---------------------------------------------------------------------------
// 1. library_search returns formatted results
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_search_formatted_results() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	let call = make_call("library_search", serde_json::json!({"query": "rust"}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("1. [programming/rust] (score: 0.95)"));
	assert!(result.output.contains("Rust is great"));
	assert!(result.output.contains("2. [programming/async] (score: 0.82)"));
	assert!(result.output.contains("Async programming"));
}

// ---------------------------------------------------------------------------
// 2. library_search with no results
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_search_no_results() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore {
		search_results: vec![],
		..Default::default()
	});
	register_library_tools(&mut registry, store);

	let call = make_call(
		"library_search",
		serde_json::json!({"query": "nonexistent"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "No matching volumes found.");
}

// ---------------------------------------------------------------------------
// 3. library_shelve returns ID
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_shelve_returns_id() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	let call = make_call(
		"library_shelve",
		serde_json::json!({"text": "important note", "topic": "notes"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Shelved volume with ID: vol_42");
}

// ---------------------------------------------------------------------------
// 4. library_withdraw found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_withdraw_found() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore {
		delete_found: true,
		..Default::default()
	});
	register_library_tools(&mut registry, store);

	let call = make_call("library_withdraw", serde_json::json!({"id": "vol_1"}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Withdrew volume: vol_1");
}

// ---------------------------------------------------------------------------
// 5. library_withdraw not found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_withdraw_not_found() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore {
		delete_found: false,
		..Default::default()
	});
	register_library_tools(&mut registry, store);

	let call = make_call(
		"library_withdraw",
		serde_json::json!({"id": "missing_vol"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Volume not found: missing_vol");
}

// ---------------------------------------------------------------------------
// 6. library_catalog with topics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_catalog_all_topics() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	let call = make_call("library_catalog", serde_json::json!({}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("programming (10 volumes)"));
	// Sub-topics should be indented
	assert!(result.output.contains("  programming/rust (5 volumes)"));
	assert!(result.output.contains("  programming/async (3 volumes)"));
	assert!(result.output.contains("design (2 volumes)"));
}

// ---------------------------------------------------------------------------
// 7. library_catalog filtered by topic
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_catalog_filtered() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	let call = make_call(
		"library_catalog",
		serde_json::json!({"topic": "programming"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("programming (10 volumes)"));
	assert!(result.output.contains("programming/rust (5 volumes)"));
	// "design" should not appear
	assert!(!result.output.contains("design"));
}

// ---------------------------------------------------------------------------
// 8. library_catalog no topics found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_catalog_empty() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore {
		topics: vec![],
		..Default::default()
	});
	register_library_tools(&mut registry, store);

	let call = make_call("library_catalog", serde_json::json!({}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "No topics found.");
}

// ---------------------------------------------------------------------------
// 9. library_compact with sufficient volumes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_compact_sufficient_volumes() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	let call = make_call(
		"library_compact",
		serde_json::json!({"topic": "programming"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(
		result.output,
		"Created compendium comp_1 from 3 volumes."
	);
}

// ---------------------------------------------------------------------------
// 10. library_compact with insufficient volumes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_compact_insufficient_volumes() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore {
		volumes: vec![VolumeInfo {
			id: "v1".to_string(),
		}],
		..Default::default()
	});
	register_library_tools(&mut registry, store);

	let call = make_call(
		"library_compact",
		serde_json::json!({"topic": "sparse"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result
		.output
		.contains("has fewer than 2 volumes"));
	assert!(result.output.contains("nothing to compact"));
}

// ===========================================================================
// VFS tool tests
// ===========================================================================

// ---------------------------------------------------------------------------
// 11. vfs_read text file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_vfs_read_text_file() {
	let mut registry = make_registry();
	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore::default());
	register_vfs_tools(&mut registry, vfs);

	let call = make_call(
		"vfs_read",
		serde_json::json!({"path": "vfs:///hello.txt"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Hello, world!");
}

// ---------------------------------------------------------------------------
// 12. vfs_read binary file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_vfs_read_binary_file() {
	let mut registry = make_registry();
	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore {
		read_result: FileReadResult {
			text: String::new(),
			content_type: "binary".to_string(),
			size: 1024,
		},
		..Default::default()
	});
	register_vfs_tools(&mut registry, vfs);

	let call = make_call(
		"vfs_read",
		serde_json::json!({"path": "vfs:///image.png"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "[Binary file: 1024 bytes]");
}

// ---------------------------------------------------------------------------
// 13. vfs_write returns byte count
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_vfs_write_returns_byte_count() {
	let mut registry = make_registry();
	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore {
		write_bytes: 128,
		..Default::default()
	});
	register_vfs_tools(&mut registry, vfs);

	let call = make_call(
		"vfs_write",
		serde_json::json!({"path": "vfs:///output.txt", "content": "test data"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Wrote 128 bytes to vfs:///output.txt");
}

// ---------------------------------------------------------------------------
// 14. vfs_list directory entries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_vfs_list_directory_entries() {
	let mut registry = make_registry();
	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore::default());
	register_vfs_tools(&mut registry, vfs);

	let call = make_call("vfs_list", serde_json::json!({"path": "vfs:///"}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("d src"));
	assert!(result.output.contains("f README.md"));
	assert!(result.output.contains("f Cargo.toml"));
}

// ---------------------------------------------------------------------------
// 15. vfs_list empty directory
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_vfs_list_empty_directory() {
	let mut registry = make_registry();
	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore {
		dir_entries: vec![],
		..Default::default()
	});
	register_vfs_tools(&mut registry, vfs);

	let call = make_call("vfs_list", serde_json::json!({}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Directory is empty.");
}

// ---------------------------------------------------------------------------
// 16. vfs_tree output
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_vfs_tree_output() {
	let mut registry = make_registry();
	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore::default());
	register_vfs_tools(&mut registry, vfs);

	let call = make_call("vfs_tree", serde_json::json!({}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("vfs:///"));
	assert!(result.output.contains("src/"));
	assert!(result.output.contains("main.rs"));
}

// ===========================================================================
// Task tool tests
// ===========================================================================

fn make_task_list() -> Arc<Mutex<TaskList>> {
	Arc::new(Mutex::new(TaskList::new(Some(TaskListOptions {
		max_tasks: Some(100),
	}))))
}

// ---------------------------------------------------------------------------
// 17. task_create and verify
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_create_and_verify() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	let call = make_call(
		"task_create",
		serde_json::json!({
			"subject": "Fix bug",
			"description": "Fix the login bug",
			"activeForm": "Fixing bug"
		}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("Created task #1: Fix bug"));

	// Verify task exists in the list
	let list = tl.lock().unwrap();
	let task = list.get("1").unwrap();
	assert_eq!(task.subject, "Fix bug");
	assert_eq!(task.description, "Fix the login bug");
	assert_eq!(task.active_form.as_deref(), Some("Fixing bug"));
}

// ---------------------------------------------------------------------------
// 18. task_get found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_get_found() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	// Create a task first
	{
		let mut list = tl.lock().unwrap();
		list.create(simse_core::tasks::TaskCreateInput {
			subject: "Test task".to_string(),
			description: "A test task".to_string(),
			active_form: None,
			owner: None,
			metadata: None,
		});
	}

	let call = make_call("task_get", serde_json::json!({"id": "1"}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	// Should be JSON
	assert!(result.output.contains("\"subject\": \"Test task\""));
	assert!(result.output.contains("\"description\": \"A test task\""));
}

// ---------------------------------------------------------------------------
// 19. task_get not found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_get_not_found() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	let call = make_call("task_get", serde_json::json!({"id": "999"}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Task not found: 999");
}

// ---------------------------------------------------------------------------
// 20. task_update status change
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_update_status_change() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	// Create a task
	{
		let mut list = tl.lock().unwrap();
		list.create(simse_core::tasks::TaskCreateInput {
			subject: "Do work".to_string(),
			description: "Work description".to_string(),
			active_form: None,
			owner: None,
			metadata: None,
		});
	}

	let call = make_call(
		"task_update",
		serde_json::json!({"id": "1", "status": "in_progress"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("Updated task #1: Do work"));
	assert!(result.output.contains("InProgress"));

	// Verify the status changed
	let list = tl.lock().unwrap();
	let task = list.get("1").unwrap();
	assert_eq!(task.status, simse_core::tasks::TaskStatus::InProgress);
}

// ---------------------------------------------------------------------------
// 21. task_update subject change
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_update_subject_change() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	// Create a task
	{
		let mut list = tl.lock().unwrap();
		list.create(simse_core::tasks::TaskCreateInput {
			subject: "Old subject".to_string(),
			description: "Desc".to_string(),
			active_form: None,
			owner: None,
			metadata: None,
		});
	}

	let call = make_call(
		"task_update",
		serde_json::json!({"id": "1", "subject": "New subject"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("New subject"));
}

// ---------------------------------------------------------------------------
// 22. task_update not found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_update_not_found() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	let call = make_call(
		"task_update",
		serde_json::json!({"id": "999", "status": "completed"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Task not found: 999");
}

// ---------------------------------------------------------------------------
// 23. task_delete found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_delete_found() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	// Create a task
	{
		let mut list = tl.lock().unwrap();
		list.create(simse_core::tasks::TaskCreateInput {
			subject: "To delete".to_string(),
			description: "Will be deleted".to_string(),
			active_form: None,
			owner: None,
			metadata: None,
		});
	}

	let call = make_call("task_delete", serde_json::json!({"id": "1"}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Deleted task #1");

	// Verify task is gone
	let list = tl.lock().unwrap();
	assert!(list.get("1").is_none());
}

// ---------------------------------------------------------------------------
// 24. task_delete not found
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_delete_not_found() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	let call = make_call("task_delete", serde_json::json!({"id": "999"}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "Task not found: 999");
}

// ---------------------------------------------------------------------------
// 25. task_list formatted output
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_list_formatted_output() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	// Create multiple tasks
	{
		let mut list = tl.lock().unwrap();
		list.create(simse_core::tasks::TaskCreateInput {
			subject: "First task".to_string(),
			description: "Desc 1".to_string(),
			active_form: None,
			owner: None,
			metadata: None,
		});
		list.create(simse_core::tasks::TaskCreateInput {
			subject: "Second task".to_string(),
			description: "Desc 2".to_string(),
			active_form: None,
			owner: None,
			metadata: None,
		});
	}

	let call = make_call("task_list", serde_json::json!({}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("[Pending]"));
	assert!(result.output.contains("First task"));
	assert!(result.output.contains("Second task"));
}

// ---------------------------------------------------------------------------
// 26. task_list empty
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_list_empty() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	let call = make_call("task_list", serde_json::json!({}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert_eq!(result.output, "No tasks.");
}

// ---------------------------------------------------------------------------
// 27. task_list with dependencies
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_list_with_dependencies() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	// Create tasks with dependencies
	{
		let mut list = tl.lock().unwrap();
		list.create(simse_core::tasks::TaskCreateInput {
			subject: "Base task".to_string(),
			description: "Desc".to_string(),
			active_form: None,
			owner: None,
			metadata: None,
		});
		list.create(simse_core::tasks::TaskCreateInput {
			subject: "Dependent task".to_string(),
			description: "Desc".to_string(),
			active_form: None,
			owner: None,
			metadata: None,
		});
		// Make task 2 blocked by task 1
		list.update(
			"2",
			simse_core::tasks::TaskUpdateInput {
				add_blocked_by: Some(vec!["1".to_string()]),
				..Default::default()
			},
		)
		.unwrap();
	}

	let call = make_call("task_list", serde_json::json!({}));
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("(blocked by: 1)"));
	assert!(result.output.contains("(blocks: 2)"));
}

// ---------------------------------------------------------------------------
// 28. task_update with addBlocks comma-separated
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_update_with_add_blocks() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, Arc::clone(&tl));

	// Create 3 tasks
	{
		let mut list = tl.lock().unwrap();
		for i in 1..=3 {
			list.create(simse_core::tasks::TaskCreateInput {
				subject: format!("Task {}", i),
				description: "Desc".to_string(),
				active_form: None,
				owner: None,
				metadata: None,
			});
		}
	}

	// Task 1 blocks tasks 2 and 3
	let call = make_call(
		"task_update",
		serde_json::json!({"id": "1", "addBlocks": "2, 3"}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	assert!(result.output.contains("Updated task #1"));

	// Verify dependencies
	let list = tl.lock().unwrap();
	let task1 = list.get("1").unwrap();
	assert!(task1.blocks.contains(&"2".to_string()));
	assert!(task1.blocks.contains(&"3".to_string()));

	let task2 = list.get("2").unwrap();
	assert!(task2.blocked_by.contains(&"1".to_string()));
}

// ===========================================================================
// Registration verification tests
// ===========================================================================

// ---------------------------------------------------------------------------
// 29. All library tools registered
// ---------------------------------------------------------------------------

#[test]
fn test_library_tools_registered() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	assert!(registry.is_registered("library_search"));
	assert!(registry.is_registered("library_shelve"));
	assert!(registry.is_registered("library_withdraw"));
	assert!(registry.is_registered("library_catalog"));
	assert!(registry.is_registered("library_compact"));
	assert_eq!(registry.tool_count(), 5);
}

// ---------------------------------------------------------------------------
// 30. All VFS tools registered
// ---------------------------------------------------------------------------

#[test]
fn test_vfs_tools_registered() {
	let mut registry = make_registry();
	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore::default());
	register_vfs_tools(&mut registry, vfs);

	assert!(registry.is_registered("vfs_read"));
	assert!(registry.is_registered("vfs_write"));
	assert!(registry.is_registered("vfs_list"));
	assert!(registry.is_registered("vfs_tree"));
	assert_eq!(registry.tool_count(), 4);
}

// ---------------------------------------------------------------------------
// 31. All task tools registered
// ---------------------------------------------------------------------------

#[test]
fn test_task_tools_registered() {
	let mut registry = make_registry();
	let tl = make_task_list();
	register_task_tools(&mut registry, tl);

	assert!(registry.is_registered("task_create"));
	assert!(registry.is_registered("task_get"));
	assert!(registry.is_registered("task_update"));
	assert!(registry.is_registered("task_delete"));
	assert!(registry.is_registered("task_list"));
	assert_eq!(registry.tool_count(), 5);
}

// ---------------------------------------------------------------------------
// 32. Tool categories are correct
// ---------------------------------------------------------------------------

#[test]
fn test_tool_categories() {
	let mut registry = make_registry();

	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore::default());
	register_vfs_tools(&mut registry, vfs);

	let tl = make_task_list();
	register_task_tools(&mut registry, tl);

	use simse_core::tools::ToolCategory;

	// Library tools
	assert_eq!(
		registry.get_tool_definition("library_search").unwrap().category,
		ToolCategory::Library
	);
	assert_eq!(
		registry.get_tool_definition("library_shelve").unwrap().category,
		ToolCategory::Library
	);

	// VFS tools
	assert_eq!(
		registry.get_tool_definition("vfs_read").unwrap().category,
		ToolCategory::Vfs
	);
	assert_eq!(
		registry.get_tool_definition("vfs_write").unwrap().category,
		ToolCategory::Vfs
	);

	// Task tools
	assert_eq!(
		registry.get_tool_definition("task_create").unwrap().category,
		ToolCategory::Task
	);
	assert_eq!(
		registry.get_tool_definition("task_list").unwrap().category,
		ToolCategory::Task
	);
}

// ---------------------------------------------------------------------------
// 33. Tool annotations are correct
// ---------------------------------------------------------------------------

#[test]
fn test_tool_annotations() {
	let mut registry = make_registry();

	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore::default());
	register_vfs_tools(&mut registry, vfs);

	let tl = make_task_list();
	register_task_tools(&mut registry, tl);

	// Read-only tools
	let search_def = registry.get_tool_definition("library_search").unwrap();
	assert_eq!(
		search_def.annotations.as_ref().unwrap().read_only,
		Some(true)
	);

	let vfs_read_def = registry.get_tool_definition("vfs_read").unwrap();
	assert_eq!(
		vfs_read_def.annotations.as_ref().unwrap().read_only,
		Some(true)
	);

	let task_get_def = registry.get_tool_definition("task_get").unwrap();
	assert_eq!(
		task_get_def.annotations.as_ref().unwrap().read_only,
		Some(true)
	);

	// Destructive tools
	let withdraw_def = registry.get_tool_definition("library_withdraw").unwrap();
	assert_eq!(
		withdraw_def.annotations.as_ref().unwrap().destructive,
		Some(true)
	);

	let task_delete_def = registry.get_tool_definition("task_delete").unwrap();
	assert_eq!(
		task_delete_def.annotations.as_ref().unwrap().destructive,
		Some(true)
	);
}

// ---------------------------------------------------------------------------
// 34. library_search respects maxResults
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_library_search_respects_max_results() {
	let mut registry = make_registry();
	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	let call = make_call(
		"library_search",
		serde_json::json!({"query": "rust", "maxResults": 1}),
	);
	let result = registry.execute(&call).await;

	assert!(!result.is_error);
	// Should only contain 1 result
	assert!(result.output.contains("1. [programming/rust]"));
	assert!(!result.output.contains("2. [programming/async]"));
}

// ---------------------------------------------------------------------------
// 35. All tools registered together
// ---------------------------------------------------------------------------

#[test]
fn test_all_tools_registered_together() {
	let mut registry = make_registry();

	let store: Arc<dyn LibraryStore> = Arc::new(MockLibraryStore::default());
	register_library_tools(&mut registry, store);

	let vfs: Arc<dyn VfsStore> = Arc::new(MockVfsStore::default());
	register_vfs_tools(&mut registry, vfs);

	let tl = make_task_list();
	register_task_tools(&mut registry, tl);

	// 5 library + 4 VFS + 5 task = 14 total
	assert_eq!(registry.tool_count(), 14);
}
