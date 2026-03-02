//! Built-in tool registration for library, VFS, and task tools.
//!
//! Ports `src/ai/tools/builtin-tools.ts` (~508 lines of TS) to Rust.
//!
//! Defines trait abstractions (`LibraryStore`, `VfsStore`) so that handlers
//! are testable with mocks. Task tools use `Arc<Mutex<TaskList>>` directly.
//!
//! Three registration functions add tool handlers to a `ToolRegistry`:
//! - `register_library_tools` — search, shelve, withdraw, catalog, compact
//! - `register_vfs_tools` — read, write, list, tree
//! - `register_task_tools` — create, get, update, delete, list

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::Value;

use crate::error::SimseError;
use crate::tasks::{TaskCreateInput, TaskList, TaskStatus, TaskUpdateInput};
use crate::tools::registry::ToolRegistry;
use crate::tools::types::{
	ToolAnnotations, ToolCategory, ToolDefinition, ToolHandler, ToolParameter,
};

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// A single search result from the library store.
#[derive(Debug, Clone)]
pub struct SearchResult {
	/// The text content of the volume.
	pub text: String,
	/// The topic classification, if any.
	pub topic: Option<String>,
	/// Relevance score (0.0 to 1.0).
	pub score: f64,
}

/// Information about a topic in the library catalog.
#[derive(Debug, Clone)]
pub struct TopicInfo {
	/// The topic path (e.g. "rust/async").
	pub topic: String,
	/// Number of entries (volumes) under this topic.
	pub entry_count: usize,
}

/// Minimal volume info used for filtering and compendium.
#[derive(Debug, Clone)]
pub struct VolumeInfo {
	/// The volume ID.
	pub id: String,
}

/// Result of a compendium (summarization) operation.
#[derive(Debug, Clone)]
pub struct CompendiumResult {
	/// ID of the created compendium.
	pub compendium_id: String,
	/// IDs of the source volumes that were compacted.
	pub source_ids: Vec<String>,
}

/// Result of reading a file from the VFS.
#[derive(Debug, Clone)]
pub struct FileReadResult {
	/// The file text content (empty for binary).
	pub text: String,
	/// Content type: "text" or "binary".
	pub content_type: String,
	/// File size in bytes.
	pub size: usize,
}

/// A directory entry from the VFS.
#[derive(Debug, Clone)]
pub struct DirEntry {
	/// Entry name (file or directory name).
	pub name: String,
	/// Entry type: "directory" or "file".
	pub entry_type: String,
}

// ---------------------------------------------------------------------------
// Trait abstractions
// ---------------------------------------------------------------------------

/// Abstraction over the vector store for library tool handlers.
#[async_trait]
pub trait LibraryStore: Send + Sync {
	/// Search the library for matching volumes.
	async fn search(
		&self,
		query: &str,
		max_results: usize,
	) -> Result<Vec<SearchResult>, SimseError>;

	/// Add a volume to the library.
	async fn add(&self, text: &str, topic: &str) -> Result<String, SimseError>;

	/// Delete a volume by ID. Returns `true` if found and deleted.
	async fn delete(&self, id: &str) -> Result<bool, SimseError>;

	/// Get the topic catalog.
	async fn get_topics(&self) -> Result<Vec<TopicInfo>, SimseError>;

	/// Filter volumes by topic(s).
	async fn filter_by_topic(
		&self,
		topics: &[String],
	) -> Result<Vec<VolumeInfo>, SimseError>;

	/// Create a compendium from the given volume IDs.
	async fn compendium(
		&self,
		ids: &[String],
	) -> Result<CompendiumResult, SimseError>;
}

/// Abstraction over the virtual filesystem for VFS tool handlers.
#[async_trait]
pub trait VfsStore: Send + Sync {
	/// Read a file by path.
	async fn read_file(&self, path: &str) -> Result<FileReadResult, SimseError>;

	/// Write a file, returning the number of bytes written.
	async fn write_file(
		&self,
		path: &str,
		content: &str,
	) -> Result<usize, SimseError>;

	/// List directory entries at the given path.
	async fn readdir(&self, path: &str) -> Result<Vec<DirEntry>, SimseError>;

	/// Get a tree view of the filesystem at the given path.
	async fn tree(&self, path: &str) -> Result<String, SimseError>;
}

// ---------------------------------------------------------------------------
// Helper: build a ToolParameter
// ---------------------------------------------------------------------------

fn param(param_type: &str, description: &str, required: bool) -> ToolParameter {
	ToolParameter {
		param_type: param_type.to_string(),
		description: description.to_string(),
		required,
	}
}

// ---------------------------------------------------------------------------
// Library tool registration
// ---------------------------------------------------------------------------

/// Register library tools: search, shelve, withdraw, catalog, compact.
pub fn register_library_tools(
	registry: &mut ToolRegistry,
	store: Arc<dyn LibraryStore>,
) {
	// 1. library_search
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"query".to_string(),
			param("string", "The search query", true),
		);
		parameters.insert(
			"maxResults".to_string(),
			param(
				"number",
				"Maximum number of results to return (default: 5)",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "library_search".to_string(),
			description: "Search the library for relevant volumes and context. Returns matching volumes ranked by relevance.".to_string(),
			parameters,
			category: ToolCategory::Library,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let store = Arc::clone(&store);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let store = Arc::clone(&store);
			Box::pin(async move {
				let query = args
					.get("query")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let max_results = args
					.get("maxResults")
					.and_then(|v| v.as_u64())
					.unwrap_or(5) as usize;

				let results = store.search(query, max_results).await?;

				if results.is_empty() {
					return Ok("No matching volumes found.".to_string());
				}

				let formatted: Vec<String> = results
					.iter()
					.enumerate()
					.map(|(i, r)| {
						let topic = r
							.topic
							.as_deref()
							.unwrap_or("uncategorized");
						format!(
							"{}. [{}] (score: {:.2})\n   {}",
							i + 1,
							topic,
							r.score,
							r.text,
						)
					})
					.collect();

				Ok(formatted.join("\n\n"))
			})
		});

		registry.register(definition, handler);
	}

	// 2. library_shelve
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"text".to_string(),
			param("string", "The text content to shelve", true),
		);
		parameters.insert(
			"topic".to_string(),
			param("string", "Topic category for the volume", true),
		);

		let definition = ToolDefinition {
			name: "library_shelve".to_string(),
			description: "Shelve a volume in the library for long-term storage."
				.to_string(),
			parameters,
			category: ToolCategory::Library,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let store = Arc::clone(&store);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let store = Arc::clone(&store);
			Box::pin(async move {
				let text = args
					.get("text")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let topic = args
					.get("topic")
					.and_then(|v| v.as_str())
					.unwrap_or("general");

				let id = store.add(text, topic).await?;
				Ok(format!("Shelved volume with ID: {}", id))
			})
		});

		registry.register(definition, handler);
	}

	// 3. library_withdraw
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"id".to_string(),
			param("string", "The volume ID to withdraw", true),
		);

		let definition = ToolDefinition {
			name: "library_withdraw".to_string(),
			description: "Withdraw a volume from the library by ID.".to_string(),
			parameters,
			category: ToolCategory::Library,
			annotations: Some(ToolAnnotations {
				destructive: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let store = Arc::clone(&store);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let store = Arc::clone(&store);
			Box::pin(async move {
				let id = args
					.get("id")
					.and_then(|v| v.as_str())
					.unwrap_or("");

				let deleted = store.delete(id).await?;
				if deleted {
					Ok(format!("Withdrew volume: {}", id))
				} else {
					Ok(format!("Volume not found: {}", id))
				}
			})
		});

		registry.register(definition, handler);
	}

	// 4. library_catalog
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"topic".to_string(),
			param(
				"string",
				"Optional topic to filter by (shows subtopics)",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "library_catalog".to_string(),
			description: "Browse the topic catalog. Returns the hierarchical topic tree with volume counts.".to_string(),
			parameters,
			category: ToolCategory::Library,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let store = Arc::clone(&store);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let store = Arc::clone(&store);
			Box::pin(async move {
				let topics = store.get_topics().await?;
				let filter_topic = args
					.get("topic")
					.and_then(|v| v.as_str());

				let filtered: Vec<&TopicInfo> = if let Some(filter) = filter_topic
				{
					topics
						.iter()
						.filter(|t| {
							t.topic == filter
								|| t.topic.starts_with(&format!("{}/", filter))
						})
						.collect()
				} else {
					topics.iter().collect()
				};

				if filtered.is_empty() {
					return Ok("No topics found.".to_string());
				}

				let lines: Vec<String> = filtered
					.iter()
					.map(|t| {
						let depth = t.topic.matches('/').count();
						let indent = "  ".repeat(depth);
						format!(
							"{}{} ({} volumes)",
							indent, t.topic, t.entry_count
						)
					})
					.collect();

				Ok(lines.join("\n"))
			})
		});

		registry.register(definition, handler);
	}

	// 5. library_compact
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"topic".to_string(),
			param("string", "The topic to compact", true),
		);

		let definition = ToolDefinition {
			name: "library_compact".to_string(),
			description: "Trigger a compendium (summarization) for a specific topic. Condenses multiple volumes into a single summary.".to_string(),
			parameters,
			category: ToolCategory::Library,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let store = Arc::clone(&store);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let store = Arc::clone(&store);
			Box::pin(async move {
				let topic = args
					.get("topic")
					.and_then(|v| v.as_str())
					.unwrap_or("");

				let volumes = store
					.filter_by_topic(&[topic.to_string()])
					.await?;

				if volumes.len() < 2 {
					return Ok(format!(
						"Topic \"{}\" has fewer than 2 volumes — nothing to compact.",
						topic
					));
				}

				let ids: Vec<String> =
					volumes.iter().map(|v| v.id.clone()).collect();
				let result = store.compendium(&ids).await?;

				Ok(format!(
					"Created compendium {} from {} volumes.",
					result.compendium_id,
					result.source_ids.len()
				))
			})
		});

		registry.register(definition, handler);
	}
}

// ---------------------------------------------------------------------------
// VFS tool registration
// ---------------------------------------------------------------------------

/// Register VFS tools: read, write, list, tree.
pub fn register_vfs_tools(
	registry: &mut ToolRegistry,
	vfs: Arc<dyn VfsStore>,
) {
	// 1. vfs_read
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param(
				"string",
				"VFS path using vfs:// scheme (e.g. vfs:///hello.js)",
				true,
			),
		);

		let definition = ToolDefinition {
			name: "vfs_read".to_string(),
			description: "Read a file from the virtual filesystem sandbox."
				.to_string(),
			parameters,
			category: ToolCategory::Vfs,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let vfs = Arc::clone(&vfs);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let vfs = Arc::clone(&vfs);
			Box::pin(async move {
				let path = args
					.get("path")
					.and_then(|v| v.as_str())
					.unwrap_or("vfs:///");

				let result = vfs.read_file(path).await?;
				if result.content_type == "binary" {
					Ok(format!("[Binary file: {} bytes]", result.size))
				} else {
					Ok(result.text)
				}
			})
		});

		registry.register(definition, handler);
	}

	// 2. vfs_write
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param(
				"string",
				"VFS path using vfs:// scheme (e.g. vfs:///hello.js)",
				true,
			),
		);
		parameters.insert(
			"content".to_string(),
			param("string", "The file content to write", true),
		);

		let definition = ToolDefinition {
			name: "vfs_write".to_string(),
			description: "Write a file to the virtual filesystem sandbox."
				.to_string(),
			parameters,
			category: ToolCategory::Vfs,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let vfs = Arc::clone(&vfs);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let vfs = Arc::clone(&vfs);
			Box::pin(async move {
				let path = args
					.get("path")
					.and_then(|v| v.as_str())
					.unwrap_or("vfs:///");
				let content = args
					.get("content")
					.and_then(|v| v.as_str())
					.unwrap_or("");

				let bytes_written = vfs.write_file(path, content).await?;
				Ok(format!("Wrote {} bytes to {}", bytes_written, path))
			})
		});

		registry.register(definition, handler);
	}

	// 3. vfs_list
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param(
				"string",
				"VFS path using vfs:// scheme (e.g. vfs:///hello.js)",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "vfs_list".to_string(),
			description:
				"List files and directories in the virtual filesystem sandbox."
					.to_string(),
			parameters,
			category: ToolCategory::Vfs,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let vfs = Arc::clone(&vfs);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let vfs = Arc::clone(&vfs);
			Box::pin(async move {
				let path = args
					.get("path")
					.and_then(|v| v.as_str())
					.unwrap_or("vfs:///");

				let entries = vfs.readdir(path).await?;

				if entries.is_empty() {
					return Ok("Directory is empty.".to_string());
				}

				let lines: Vec<String> = entries
					.iter()
					.map(|e| {
						let prefix = if e.entry_type == "directory" {
							"d"
						} else {
							"f"
						};
						format!("{} {}", prefix, e.name)
					})
					.collect();

				Ok(lines.join("\n"))
			})
		});

		registry.register(definition, handler);
	}

	// 4. vfs_tree
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param(
				"string",
				"VFS path using vfs:// scheme (e.g. vfs:///hello.js)",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "vfs_tree".to_string(),
			description:
				"Show a tree view of the virtual filesystem sandbox."
					.to_string(),
			parameters,
			category: ToolCategory::Vfs,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let vfs = Arc::clone(&vfs);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let vfs = Arc::clone(&vfs);
			Box::pin(async move {
				let path = args
					.get("path")
					.and_then(|v| v.as_str())
					.unwrap_or("vfs:///");

				vfs.tree(path).await
			})
		});

		registry.register(definition, handler);
	}
}

// ---------------------------------------------------------------------------
// Task tool registration
// ---------------------------------------------------------------------------

/// Parse a status string into a `TaskStatus`.
fn parse_task_status(s: &str) -> Option<TaskStatus> {
	match s {
		"pending" => Some(TaskStatus::Pending),
		"in_progress" => Some(TaskStatus::InProgress),
		"completed" => Some(TaskStatus::Completed),
		"deleted" => Some(TaskStatus::Deleted),
		_ => None,
	}
}

/// Split a comma-separated string into a vec of trimmed, non-empty strings.
fn split_comma_ids(s: &str) -> Vec<String> {
	s.split(',')
		.map(|part| part.trim().to_string())
		.filter(|part| !part.is_empty())
		.collect()
}

/// Register task tools: create, get, update, delete, list.
pub fn register_task_tools(
	registry: &mut ToolRegistry,
	task_list: Arc<Mutex<TaskList>>,
) {
	// 1. task_create
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"subject".to_string(),
			param(
				"string",
				"Brief imperative title (e.g. \"Fix authentication bug\")",
				true,
			),
		);
		parameters.insert(
			"description".to_string(),
			param(
				"string",
				"Detailed description of what needs to be done",
				true,
			),
		);
		parameters.insert(
			"activeForm".to_string(),
			param(
				"string",
				"Present continuous form shown while in progress (e.g. \"Fixing authentication bug\")",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "task_create".to_string(),
			description:
				"Create a new task to track work. Returns the task ID."
					.to_string(),
			parameters,
			category: ToolCategory::Task,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let tl = Arc::clone(&task_list);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let tl = Arc::clone(&tl);
			Box::pin(async move {
				let subject = args
					.get("subject")
					.and_then(|v| v.as_str())
					.unwrap_or("")
					.to_string();
				let description = args
					.get("description")
					.and_then(|v| v.as_str())
					.unwrap_or("")
					.to_string();
				let active_form = args
					.get("activeForm")
					.and_then(|v| v.as_str())
					.map(|s| s.to_string());

				let mut list = tl.lock().unwrap_or_else(|e| e.into_inner());
				let task = list.create_checked(TaskCreateInput {
					subject: subject.clone(),
					description,
					active_form,
					owner: None,
					metadata: None,
				})?;

				Ok(format!("Created task #{}: {}", task.id, task.subject))
			})
		});

		registry.register(definition, handler);
	}

	// 2. task_get
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"id".to_string(),
			param("string", "The task ID", true),
		);

		let definition = ToolDefinition {
			name: "task_get".to_string(),
			description: "Get full details of a task by ID.".to_string(),
			parameters,
			category: ToolCategory::Task,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let tl = Arc::clone(&task_list);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let tl = Arc::clone(&tl);
			Box::pin(async move {
				let id = args
					.get("id")
					.and_then(|v| v.as_str())
					.unwrap_or("");

				let list = tl.lock().unwrap_or_else(|e| e.into_inner());
				match list.get(id) {
					Some(task) => {
						let json = serde_json::to_string_pretty(task)
							.unwrap_or_else(|_| format!("{:?}", task));
						Ok(json)
					}
					None => Ok(format!("Task not found: {}", id)),
				}
			})
		});

		registry.register(definition, handler);
	}

	// 3. task_update
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"id".to_string(),
			param("string", "The task ID", true),
		);
		parameters.insert(
			"status".to_string(),
			param(
				"string",
				"New status: \"pending\", \"in_progress\", or \"completed\"",
				false,
			),
		);
		parameters.insert(
			"subject".to_string(),
			param("string", "New subject", false),
		);
		parameters.insert(
			"description".to_string(),
			param("string", "New description", false),
		);
		parameters.insert(
			"activeForm".to_string(),
			param("string", "New active form text", false),
		);
		parameters.insert(
			"addBlocks".to_string(),
			param(
				"string",
				"Comma-separated task IDs that this task blocks",
				false,
			),
		);
		parameters.insert(
			"addBlockedBy".to_string(),
			param(
				"string",
				"Comma-separated task IDs that block this task",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "task_update".to_string(),
			description:
				"Update a task (status, subject, description, dependencies)."
					.to_string(),
			parameters,
			category: ToolCategory::Task,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let tl = Arc::clone(&task_list);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let tl = Arc::clone(&tl);
			Box::pin(async move {
				let id = args
					.get("id")
					.and_then(|v| v.as_str())
					.unwrap_or("");

				let status = args
					.get("status")
					.and_then(|v| v.as_str())
					.and_then(parse_task_status);
				let subject = args
					.get("subject")
					.and_then(|v| v.as_str())
					.map(|s| s.to_string());
				let description = args
					.get("description")
					.and_then(|v| v.as_str())
					.map(|s| s.to_string());
				let active_form = args
					.get("activeForm")
					.and_then(|v| v.as_str())
					.map(|s| s.to_string());
				let add_blocks = args
					.get("addBlocks")
					.and_then(|v| v.as_str())
					.map(split_comma_ids);
				let add_blocked_by = args
					.get("addBlockedBy")
					.and_then(|v| v.as_str())
					.map(split_comma_ids);

				let update = TaskUpdateInput {
					status,
					subject,
					description,
					active_form,
					owner: None,
					metadata: None,
					add_blocks,
					add_blocked_by,
				};

				let mut list = tl.lock().unwrap_or_else(|e| e.into_inner());
				match list.update(id, update)? {
					Some(task) => Ok(format!(
						"Updated task #{}: {} [{:?}]",
						task.id, task.subject, task.status
					)),
					None => Ok(format!("Task not found: {}", id)),
				}
			})
		});

		registry.register(definition, handler);
	}

	// 4. task_delete
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"id".to_string(),
			param("string", "The task ID", true),
		);

		let definition = ToolDefinition {
			name: "task_delete".to_string(),
			description: "Delete a task by ID.".to_string(),
			parameters,
			category: ToolCategory::Task,
			annotations: Some(ToolAnnotations {
				destructive: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let tl = Arc::clone(&task_list);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let tl = Arc::clone(&tl);
			Box::pin(async move {
				let id = args
					.get("id")
					.and_then(|v| v.as_str())
					.unwrap_or("");

				let mut list = tl.lock().unwrap_or_else(|e| e.into_inner());
				if list.delete(id) {
					Ok(format!("Deleted task #{}", id))
				} else {
					Ok(format!("Task not found: {}", id))
				}
			})
		});

		registry.register(definition, handler);
	}

	// 5. task_list
	{
		let definition = ToolDefinition {
			name: "task_list".to_string(),
			description:
				"List all tasks with their status, subject, and dependencies."
					.to_string(),
			parameters: HashMap::new(),
			category: ToolCategory::Task,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let tl = Arc::clone(&task_list);
		let handler: ToolHandler = Arc::new(move |_args: Value| {
			let tl = Arc::clone(&tl);
			Box::pin(async move {
				let list = tl.lock().unwrap_or_else(|e| e.into_inner());
				let tasks = list.list();

				if tasks.is_empty() {
					return Ok("No tasks.".to_string());
				}

				let lines: Vec<String> = tasks
					.iter()
					.map(|t| {
						let mut line = format!(
							"#{} [{:?}] {}",
							t.id, t.status, t.subject
						);
						if !t.blocked_by.is_empty() {
							line.push_str(&format!(
								" (blocked by: {})",
								t.blocked_by.join(", ")
							));
						}
						if !t.blocks.is_empty() {
							line.push_str(&format!(
								" (blocks: {})",
								t.blocks.join(", ")
							));
						}
						line
					})
					.collect();

				Ok(lines.join("\n"))
			})
		});

		registry.register(definition, handler);
	}
}
