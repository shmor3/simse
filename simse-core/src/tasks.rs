//! Task list with dependency tracking and cycle detection.
//!
//! Ports `src/ai/tasks/task-list.ts` + `types.ts` (~366 lines).
//!
//! Provides auto-incrementing IDs, BFS circular dependency detection,
//! reciprocal dependency maintenance, metadata merge with null filtering,
//! and configurable task limits.

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use im::HashMap as ImHashMap;
use serde::{Deserialize, Serialize};

use crate::error::{SimseError, TaskErrorCode};

// ---------------------------------------------------------------------------
// TaskStatus
// ---------------------------------------------------------------------------

/// Status of a task within the task list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
	Pending,
	InProgress,
	Completed,
	Deleted,
}

// ---------------------------------------------------------------------------
// TaskItem
// ---------------------------------------------------------------------------

/// A single task in the task list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskItem {
	pub id: String,
	pub subject: String,
	pub description: String,
	pub status: TaskStatus,
	pub active_form: Option<String>,
	pub owner: Option<String>,
	pub metadata: Option<HashMap<String, serde_json::Value>>,
	pub blocks: Vec<String>,
	pub blocked_by: Vec<String>,
	pub created_at: u64,
	pub updated_at: u64,
}

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input for creating a new task.
#[derive(Debug, Clone)]
pub struct TaskCreateInput {
	pub subject: String,
	pub description: String,
	pub active_form: Option<String>,
	pub owner: Option<String>,
	pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Input for updating an existing task.
#[derive(Debug, Clone, Default)]
pub struct TaskUpdateInput {
	pub status: Option<TaskStatus>,
	pub subject: Option<String>,
	pub description: Option<String>,
	pub active_form: Option<String>,
	pub owner: Option<String>,
	pub metadata: Option<HashMap<String, serde_json::Value>>,
	pub add_blocks: Option<Vec<String>>,
	pub add_blocked_by: Option<Vec<String>>,
}

/// Options for configuring a task list.
#[derive(Debug, Clone, Default)]
pub struct TaskListOptions {
	/// Maximum number of tasks allowed. Default: 100.
	pub max_tasks: Option<usize>,
}

// ---------------------------------------------------------------------------
// TaskList
// ---------------------------------------------------------------------------

/// Task list with dependency tracking, cycle detection, and auto-incrementing IDs.
///
/// Uses `im::HashMap` for persistent (structural sharing) task storage,
/// enabling cheap cloning and functional-style state transitions.
#[derive(Debug, Clone)]
pub struct TaskList {
	tasks: ImHashMap<String, TaskItem>,
	next_id: u64,
	max_tasks: usize,
}

/// Return the current Unix timestamp in milliseconds.
fn now_millis() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64
}

impl TaskList {
	/// Create a new task list with optional configuration.
	pub fn new(options: Option<TaskListOptions>) -> Self {
		let opts = options.unwrap_or_default();
		Self {
			tasks: ImHashMap::new(),
			next_id: 1,
			max_tasks: opts.max_tasks.unwrap_or(100),
		}
	}

	// -- Creation ----------------------------------------------------------

	/// Create a new task. Panics if the task limit is exceeded.
	///
	/// For fallible creation, use [`create_checked`].
	pub fn create(self, input: TaskCreateInput) -> (Self, TaskItem) {
		self.create_checked(input)
			.expect("task limit reached; use create_checked for fallible creation")
	}

	/// Create a new task, returning an error if the task limit is exceeded.
	pub fn create_checked(
		mut self,
		input: TaskCreateInput,
	) -> Result<(Self, TaskItem), SimseError> {
		if self.tasks.len() >= self.max_tasks {
			return Err(SimseError::task(
				TaskErrorCode::LimitReached,
				format!(
					"Task limit reached: maximum {} tasks allowed",
					self.max_tasks
				),
			));
		}

		let id = self.next_id.to_string();
		self.next_id += 1;
		let now = now_millis();

		let task = TaskItem {
			id: id.clone(),
			subject: input.subject,
			description: input.description,
			status: TaskStatus::Pending,
			active_form: input.active_form,
			owner: input.owner,
			metadata: input.metadata,
			blocks: Vec::new(),
			blocked_by: Vec::new(),
			created_at: now,
			updated_at: now,
		};

		self.tasks = self.tasks.update(id, task.clone());
		Ok((self, task))
	}

	// -- Read --------------------------------------------------------------

	/// Get a task by ID.
	pub fn get(&self, id: &str) -> Option<&TaskItem> {
		self.tasks.get(id)
	}

	/// List all tasks.
	pub fn list(&self) -> Vec<&TaskItem> {
		self.tasks.values().collect()
	}

	/// List tasks that are pending, have no owner, and are not blocked
	/// by any incomplete dependency.
	pub fn list_available(&self) -> Vec<&TaskItem> {
		self.tasks
			.values()
			.filter(|t| {
				t.status == TaskStatus::Pending
					&& t.owner.is_none()
					&& t.blocked_by.iter().all(|dep_id| {
						self.tasks
							.get(dep_id)
							.is_some_and(|dep| dep.status == TaskStatus::Completed)
					})
			})
			.collect()
	}

	/// List tasks that are blocked by unresolved dependencies.
	pub fn get_blocked(&self) -> Vec<&TaskItem> {
		self.tasks
			.values()
			.filter(|t| {
				t.status != TaskStatus::Completed
					&& t.blocked_by.iter().any(|dep_id| {
						self.tasks
							.get(dep_id)
							.is_none_or(|dep| dep.status != TaskStatus::Completed)
					})
			})
			.collect()
	}

	/// Number of tasks in the list.
	pub fn task_count(&self) -> usize {
		self.tasks.len()
	}

	// -- Update ------------------------------------------------------------

	/// Update a task by ID. Returns `Ok((self, None))` if the task does not exist.
	/// Returns `Err` if a circular dependency would be created.
	pub fn update(
		mut self,
		id: &str,
		input: TaskUpdateInput,
	) -> Result<(Self, Option<TaskItem>), SimseError> {
		if !self.tasks.contains_key(id) {
			return Ok((self, None));
		}

		// Process add_blocks
		if let Some(ref targets) = input.add_blocks {
			for target_id in targets {
				if target_id == id {
					continue; // Can't block self
				}
				let already_blocking = self
					.tasks
					.get(id)
					.is_some_and(|t| t.blocks.contains(target_id));
				if already_blocking {
					continue;
				}

				// Check for circular dependency: id blocks target_id
				if self.would_create_cycle(id, target_id) {
					return Err(SimseError::task(
						TaskErrorCode::CircularDependency,
						format!(
							"Circular dependency: task {} and task {} would form a cycle",
							id, target_id
						),
					));
				}

				// Add reciprocal: id.blocks += target_id, target.blocked_by += id
				self = self.update_task(id, |mut task| {
					task.blocks.push(target_id.clone());
					task.updated_at = now_millis();
					task
				});
				let id_str = id.to_string();
				self = self.update_task(target_id, |mut task| {
					if !task.blocked_by.contains(&id_str) {
						task.blocked_by.push(id_str.clone());
						task.updated_at = now_millis();
					}
					task
				});
			}
		}

		// Process add_blocked_by
		if let Some(ref deps) = input.add_blocked_by {
			for dep_id in deps {
				if dep_id == id {
					continue; // Can't be blocked by self
				}
				let already_blocked = self
					.tasks
					.get(id)
					.is_some_and(|t| t.blocked_by.contains(dep_id));
				if already_blocked {
					continue;
				}

				// Check for circular dependency: dep_id blocks id
				if self.would_create_cycle(dep_id, id) {
					return Err(SimseError::task(
						TaskErrorCode::CircularDependency,
						format!(
							"Circular dependency: task {} and task {} would form a cycle",
							id, dep_id
						),
					));
				}

				// Add reciprocal: id.blocked_by += dep_id, dep.blocks += id
				self = self.update_task(id, |mut task| {
					task.blocked_by.push(dep_id.clone());
					task.updated_at = now_millis();
					task
				});
				let id_str = id.to_string();
				self = self.update_task(dep_id, |mut task| {
					if !task.blocks.contains(&id_str) {
						task.blocks.push(id_str.clone());
						task.updated_at = now_millis();
					}
					task
				});
			}
		}

		// When completing a task, remove it from dependents' blocked_by
		let is_completing = input.status == Some(TaskStatus::Completed)
			&& self
				.tasks
				.get(id)
				.is_some_and(|t| t.status != TaskStatus::Completed);

		if is_completing {
			let blocks: Vec<String> = self
				.tasks
				.get(id)
				.map_or_else(Vec::new, |t| t.blocks.clone());

			let id_str = id.to_string();
			for blocked_id in &blocks {
				self = self.update_task(blocked_id, |mut task| {
					task.blocked_by.retain(|b| b != &id_str);
					task.updated_at = now_millis();
					task
				});
			}
		}

		// Apply field updates
		self = self.update_task(id, |mut task| {
			if let Some(ref status) = input.status {
				task.status = status.clone();
			}
			if let Some(ref subject) = input.subject {
				task.subject = subject.clone();
			}
			if let Some(ref description) = input.description {
				task.description = description.clone();
			}
			if let Some(ref active_form) = input.active_form {
				task.active_form = Some(active_form.clone());
			}
			if let Some(ref owner) = input.owner {
				task.owner = Some(owner.clone());
			}

			// Metadata merge with null filtering
			if let Some(ref new_meta) = input.metadata {
				let mut merged = task.metadata.take().unwrap_or_default();
				for (key, value) in new_meta {
					if value.is_null() {
						merged.remove(key);
					} else {
						merged.insert(key.clone(), value.clone());
					}
				}
				if merged.is_empty() {
					task.metadata = None;
				} else {
					task.metadata = Some(merged);
				}
			}

			task.updated_at = now_millis();
			task
		});

		let result = self.tasks.get(id).cloned();
		Ok((self, result))
	}

	// -- Delete ------------------------------------------------------------

	/// Delete a task by ID. Returns `(self, true)` if the task was found and removed.
	///
	/// Cleans up all dependency references from other tasks.
	pub fn delete(mut self, id: &str) -> (Self, bool) {
		if !self.tasks.contains_key(id) {
			return (self, false);
		}
		self.tasks = self.tasks.without(id);

		let id_str = id.to_string();
		let now = now_millis();

		// Remove from other tasks' blocks and blocked_by
		let keys: Vec<String> = self.tasks.keys().cloned().collect();
		for key in keys {
			self = self.update_task(&key, |mut task| {
				let mut changed = false;
				if task.blocks.contains(&id_str) {
					task.blocks.retain(|b| b != &id_str);
					changed = true;
				}
				if task.blocked_by.contains(&id_str) {
					task.blocked_by.retain(|b| b != &id_str);
					changed = true;
				}
				if changed {
					task.updated_at = now;
				}
				task
			});
		}

		(self, true)
	}

	// -- Clear -------------------------------------------------------------

	/// Remove all tasks and reset the ID counter. Returns the updated task list.
	pub fn clear(mut self) -> Self {
		self.tasks = ImHashMap::new();
		self.next_id = 1;
		self
	}

	// -- Internal ----------------------------------------------------------

	/// Apply a transformation function to a task by ID. If the task exists,
	/// the function receives the current value and returns the updated value.
	fn update_task(mut self, id: &str, f: impl FnOnce(TaskItem) -> TaskItem) -> Self {
		if let Some(task) = self.tasks.get(id) {
			let updated = f(task.clone());
			self.tasks = self.tasks.update(id.to_string(), updated);
		}
		self
	}

	/// BFS cycle detection: checks if adding "blocker_id blocks blocked_id"
	/// would create a cycle. A cycle exists if `blocked_id` can already
	/// reach `blocker_id` via `blocks` edges (i.e., `blocked_id` already
	/// transitively blocks `blocker_id`).
	fn would_create_cycle(&self, blocker_id: &str, blocked_id: &str) -> bool {
		let mut visited = std::collections::HashSet::new();
		let mut queue = VecDeque::new();
		queue.push_back(blocked_id.to_string());

		while let Some(current) = queue.pop_front() {
			if current == blocker_id {
				return true;
			}
			if !visited.insert(current.clone()) {
				continue;
			}
			if let Some(task) = self.tasks.get(&current) {
				for dep in &task.blocks {
					queue.push_back(dep.clone());
				}
			}
		}

		false
	}
}
