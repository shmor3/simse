// ---------------------------------------------------------------------------
// In-memory VFS core — Rust port of createVirtualFS (TypeScript)
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use regex::Regex;

use crate::diff::{compute_diff, DiffHunk};
use crate::error::VfsError;
use crate::glob::match_glob;
use crate::path::{
	ancestor_paths, base_name, normalize_path, parent_path, path_depth, validate_path, VfsLimits,
	VFS_ROOT,
};
use crate::search::{SearchMatch, SearchMode, SearchOptions, search_text};

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContentType {
	Text,
	Binary,
}

impl ContentType {
	fn as_str(&self) -> &str {
		match self {
			Self::Text => "text",
			Self::Binary => "binary",
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NodeType {
	File,
	Directory,
}

impl NodeType {
	fn as_str(&self) -> &str {
		match self {
			Self::File => "file",
			Self::Directory => "directory",
		}
	}
}

#[derive(Debug, Clone)]
struct InternalNode {
	node_type: NodeType,
	content_type: ContentType,
	text: Option<String>,
	data: Option<Vec<u8>>,
	size: u64,
	created_at: u64,
	modified_at: u64,
}

#[derive(Debug, Clone)]
struct HistoryEntryInternal {
	version: usize,
	content_type: ContentType,
	text: Option<String>,
	data: Option<Vec<u8>>,
	size: u64,
	timestamp: u64,
}

// ---------------------------------------------------------------------------
// Public result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReadResult {
	pub content_type: String,
	pub text: Option<String>,
	pub data_base64: Option<String>,
	pub size: u64,
}

#[derive(Debug, Clone)]
pub struct StatResult {
	pub path: String,
	pub node_type: String,
	pub size: u64,
	pub created_at: u64,
	pub modified_at: u64,
}

#[derive(Debug, Clone)]
pub struct DirEntryResult {
	pub name: String,
	pub node_type: String,
}

#[derive(Debug, Clone)]
pub struct HistoryEntryResult {
	pub version: usize,
	pub content_type: String,
	pub text: Option<String>,
	pub base64: Option<String>,
	pub size: u64,
	pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct SearchOpts {
	pub glob: Option<String>,
	pub max_results: usize,
	pub mode: String,
	pub context_before: usize,
	pub context_after: usize,
	pub count_only: bool,
}

impl Default for SearchOpts {
	fn default() -> Self {
		Self {
			glob: None,
			max_results: 100,
			mode: "substring".to_string(),
			context_before: 0,
			context_after: 0,
			count_only: false,
		}
	}
}

#[derive(Debug, Clone)]
pub enum SearchOutput {
	Results(Vec<SearchMatch>),
	Count(usize),
}

#[derive(Debug, Clone)]
pub struct DiffResultOutput {
	pub old_path: String,
	pub new_path: String,
	pub hunks: Vec<DiffHunk>,
	pub additions: usize,
	pub deletions: usize,
}

#[derive(Debug, Clone)]
pub struct SnapshotData {
	pub files: Vec<SnapshotFile>,
	pub directories: Vec<SnapshotDir>,
}

#[derive(Debug, Clone)]
pub struct SnapshotFile {
	pub path: String,
	pub content_type: String,
	pub text: Option<String>,
	pub base64: Option<String>,
	pub created_at: u64,
	pub modified_at: u64,
}

#[derive(Debug, Clone)]
pub struct SnapshotDir {
	pub path: String,
	pub created_at: u64,
	pub modified_at: u64,
}

#[derive(Debug, Clone)]
pub struct MetricsResult {
	pub total_size: u64,
	pub node_count: usize,
	pub file_count: usize,
	pub directory_count: usize,
}

#[derive(Debug, Clone)]
pub enum VfsEvent {
	Write {
		path: String,
		size: u64,
		content_type: String,
		is_new: bool,
	},
	Delete { path: String },
	Rename { old_path: String, new_path: String },
	Mkdir { path: String },
}

#[derive(Debug, Clone)]
pub enum TransactionOp {
	WriteFile {
		path: String,
		content: String,
	},
	DeleteFile {
		path: String,
	},
	Mkdir {
		path: String,
	},
	Rmdir {
		path: String,
	},
	Rename {
		old_path: String,
		new_path: String,
	},
	Copy {
		src: String,
		dest: String,
	},
}

// ---------------------------------------------------------------------------
// VirtualFs
// ---------------------------------------------------------------------------

pub struct VirtualFs {
	nodes: HashMap<String, InternalNode>,
	history: HashMap<String, Vec<HistoryEntryInternal>>,
	limits: VfsLimits,
	max_history_per_file: usize,
	total_size: u64,
	file_count: usize,
	dir_count: usize,
	pending_events: Vec<VfsEvent>,
}

impl VirtualFs {
	// -- Constructor ------------------------------------------------------

	pub fn new(limits: VfsLimits, max_history: usize) -> Self {
		let now = Self::now();
		let mut nodes = HashMap::new();
		nodes.insert(
			VFS_ROOT.to_string(),
			InternalNode {
				node_type: NodeType::Directory,
				content_type: ContentType::Text,
				text: None,
				data: None,
				size: 0,
				created_at: now,
				modified_at: now,
			},
		);

		Self {
			nodes,
			history: HashMap::new(),
			limits,
			max_history_per_file: max_history,
			total_size: 0,
			file_count: 0,
			dir_count: 1,
			pending_events: Vec::new(),
		}
	}

	// -- Helpers (private) ------------------------------------------------

	fn now() -> u64 {
		SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap()
			.as_millis() as u64
	}

	fn assert_valid_path(&self, path: &str) -> Result<String, VfsError> {
		let normalized =
			normalize_path(path).map_err(|_| VfsError::InvalidPath(path.to_string()))?;
		if let Some(err) = validate_path(&normalized, &self.limits) {
			return Err(VfsError::InvalidPath(err));
		}
		Ok(normalized)
	}

	fn assert_node_exists<'a>(&'a self, path: &str) -> Result<&'a InternalNode, VfsError> {
		self.nodes
			.get(path)
			.ok_or_else(|| VfsError::NotFound(format!("No such file or directory: {}", path)))
	}

	fn assert_is_file(path: &str, node: &InternalNode) -> Result<(), VfsError> {
		if node.node_type != NodeType::File {
			return Err(VfsError::NotAFile(format!("Not a file: {}", path)));
		}
		Ok(())
	}

	fn assert_is_directory(path: &str, node: &InternalNode) -> Result<(), VfsError> {
		if node.node_type != NodeType::Directory {
			return Err(VfsError::NotADirectory(format!(
				"Not a directory: {}",
				path
			)));
		}
		Ok(())
	}

	fn assert_node_limit(&self) -> Result<(), VfsError> {
		if self.nodes.len() >= self.limits.max_node_count {
			return Err(VfsError::LimitExceeded(format!(
				"Maximum node count exceeded ({})",
				self.limits.max_node_count
			)));
		}
		Ok(())
	}

	fn assert_file_size(&self, size: u64, path: &str) -> Result<(), VfsError> {
		if size > self.limits.max_file_size {
			return Err(VfsError::LimitExceeded(format!(
				"File size {} exceeds limit ({}): {}",
				size, self.limits.max_file_size, path
			)));
		}
		Ok(())
	}

	fn assert_total_size(&self, additional: u64) -> Result<(), VfsError> {
		if self.total_size + additional > self.limits.max_total_size {
			return Err(VfsError::LimitExceeded(format!(
				"Total storage size would exceed limit ({})",
				self.limits.max_total_size
			)));
		}
		Ok(())
	}

	fn ensure_parent_exists(&self, path: &str) -> Result<(), VfsError> {
		let parent = match parent_path(path) {
			Some(p) => p,
			None => return Ok(()),
		};
		let parent_node = self
			.nodes
			.get(&parent)
			.ok_or_else(|| VfsError::NotFound(format!("Parent directory does not exist: {}", parent)))?;
		Self::assert_is_directory(&parent, parent_node)?;
		Ok(())
	}

	fn create_parents(&mut self, path: &str) {
		let ancestors = ancestor_paths(path);
		let ts = Self::now();
		for ancestor in &ancestors {
			if !self.nodes.contains_key(ancestor) {
				// Best-effort: we check node limit but don't fail hard here
				// (the TS version calls assertNodeLimit which can throw, but
				// createParents is always called in contexts that already validated).
				if self.nodes.len() >= self.limits.max_node_count {
					return;
				}
				self.nodes.insert(
					ancestor.clone(),
					InternalNode {
						node_type: NodeType::Directory,
						content_type: ContentType::Text,
						text: None,
						data: None,
						size: 0,
						created_at: ts,
						modified_at: ts,
					},
				);
				self.dir_count += 1;
			}
		}
	}

	fn get_direct_children(&self, dir_path: &str) -> Vec<String> {
		let prefix = if dir_path == VFS_ROOT {
			VFS_ROOT.to_string()
		} else {
			format!("{}/", dir_path)
		};
		let mut result = Vec::new();
		for key in self.nodes.keys() {
			if key == dir_path {
				continue;
			}
			if !key.starts_with(&prefix) {
				continue;
			}
			let remainder = &key[prefix.len()..];
			if !remainder.contains('/') {
				result.push(key.clone());
			}
		}
		result
	}

	fn get_descendants(&self, dir_path: &str) -> Vec<String> {
		let prefix = if dir_path == VFS_ROOT {
			VFS_ROOT.to_string()
		} else {
			format!("{}/", dir_path)
		};
		let mut result = Vec::new();
		for key in self.nodes.keys() {
			if key == dir_path {
				continue;
			}
			if key.starts_with(&prefix) {
				result.push(key.clone());
			}
		}
		// Sort by path depth (directories before their children)
		result.sort_by(|a, b| {
			let depth_a = a.matches('/').count();
			let depth_b = b.matches('/').count();
			depth_a.cmp(&depth_b).then(a.cmp(b))
		});
		result
	}

	fn update_parent_modified_at(&mut self, path: &str, ts: u64) {
		if let Some(parent) = parent_path(path) {
			if let Some(parent_node) = self.nodes.get_mut(&parent) {
				parent_node.modified_at = ts;
			}
		}
	}

	fn record_history(&mut self, path: &str, node: &InternalNode) {
		if node.node_type != NodeType::File {
			return;
		}

		let entries = self
			.history
			.entry(path.to_string())
			.or_insert_with(Vec::new);

		let version = entries.len() + 1;
		let entry = HistoryEntryInternal {
			version,
			content_type: node.content_type.clone(),
			text: node.text.clone(),
			data: node.data.clone(),
			size: node.size,
			timestamp: node.modified_at,
		};

		entries.push(entry);

		// Trim to limit
		let max = self.max_history_per_file;
		if entries.len() > max {
			let drain_count = entries.len() - max;
			entries.drain(0..drain_count);
		}
	}

	fn get_node(&self, path: &str) -> Result<&InternalNode, VfsError> {
		self.nodes.get(path).ok_or_else(|| {
			VfsError::InvalidOperation(format!("Internal error: missing node {}", path))
		})
	}

	// -- File operations --------------------------------------------------

	pub fn read_file(&self, path: &str) -> Result<ReadResult, VfsError> {
		let normalized = self.assert_valid_path(path)?;
		let node = self.assert_node_exists(&normalized)?;
		Self::assert_is_file(&normalized, node)?;

		if node.content_type == ContentType::Binary {
			let base64_data = node
				.data
				.as_ref()
				.map(|d| BASE64.encode(d));
			return Ok(ReadResult {
				content_type: "binary".to_string(),
				text: None,
				data_base64: base64_data,
				size: node.size,
			});
		}

		Ok(ReadResult {
			content_type: "text".to_string(),
			text: Some(node.text.clone().unwrap_or_default()),
			data_base64: None,
			size: node.size,
		})
	}

	pub fn write_file(
		&mut self,
		path: &str,
		content: &str,
		content_type: Option<&str>,
		create_parents: bool,
	) -> Result<(), VfsError> {
		let normalized = self.assert_valid_path(path)?;
		if normalized == VFS_ROOT {
			return Err(VfsError::InvalidOperation(
				"Cannot write to root directory as a file".to_string(),
			));
		}

		let is_binary = content_type == Some("binary");

		// Compute new size
		let (new_size, text_val, data_val) = if is_binary {
			let decoded = BASE64
				.decode(content)
				.map_err(|e| VfsError::InvalidOperation(format!("Invalid base64: {}", e)))?;
			let sz = decoded.len() as u64;
			(sz, None, Some(decoded))
		} else {
			let sz = content.len() as u64;
			(sz, Some(content.to_string()), None)
		};

		self.assert_file_size(new_size, &normalized)?;

		let existing = self.nodes.get(&normalized).cloned();

		if let Some(ref ex) = existing {
			if ex.node_type == NodeType::Directory {
				return Err(VfsError::NotAFile(format!(
					"Cannot overwrite directory with file: {}",
					normalized
				)));
			}
		}

		let old_size = existing.as_ref().map_or(0, |n| n.size);
		let size_delta = new_size as i64 - old_size as i64;
		if size_delta > 0 {
			self.assert_total_size(size_delta as u64)?;
		}

		if create_parents {
			self.create_parents(&normalized);
		} else {
			self.ensure_parent_exists(&normalized)?;
		}

		if existing.is_none() {
			self.assert_node_limit()?;
			self.file_count += 1;
		} else if let Some(ref ex) = existing {
			if ex.node_type == NodeType::File {
				self.record_history(&normalized, ex);
			}
		}

		let ts = Self::now();
		let ct = if is_binary {
			ContentType::Binary
		} else {
			ContentType::Text
		};

		let created_at = existing.as_ref().map_or(ts, |n| n.created_at);

		self.nodes.insert(
			normalized.clone(),
			InternalNode {
				node_type: NodeType::File,
				content_type: ct,
				text: text_val,
				data: data_val,
				size: new_size,
				created_at,
				modified_at: ts,
			},
		);

		let is_new_file = existing.is_none();
		self.total_size = (self.total_size as i64 + size_delta) as u64;
		self.update_parent_modified_at(&normalized, ts);

		self.pending_events.push(VfsEvent::Write {
			path: normalized,
			size: new_size,
			content_type: if is_binary { "binary".to_string() } else { "text".to_string() },
			is_new: is_new_file,
		});

		Ok(())
	}

	pub fn append_file(&mut self, path: &str, content: &str) -> Result<(), VfsError> {
		let normalized = self.assert_valid_path(path)?;
		let node = self.assert_node_exists(&normalized)?.clone();
		Self::assert_is_file(&normalized, &node)?;

		if node.content_type != ContentType::Text {
			return Err(VfsError::InvalidOperation(format!(
				"Cannot append to binary file: {}",
				normalized
			)));
		}

		self.record_history(&normalized, &node);

		let current_text = node.text.clone().unwrap_or_default();
		let new_text = format!("{}{}", current_text, content);
		let new_size = new_text.len() as u64;

		self.assert_file_size(new_size, &normalized)?;
		let size_delta = new_size as i64 - node.size as i64;
		if size_delta > 0 {
			self.assert_total_size(size_delta as u64)?;
		}

		let ts = Self::now();
		self.nodes.insert(
			normalized.clone(),
			InternalNode {
				node_type: NodeType::File,
				content_type: ContentType::Text,
				text: Some(new_text),
				data: None,
				size: new_size,
				created_at: node.created_at,
				modified_at: ts,
			},
		);
		self.total_size = (self.total_size as i64 + size_delta) as u64;

		self.pending_events.push(VfsEvent::Write {
			path: normalized,
			size: new_size,
			content_type: "text".to_string(),
			is_new: false,
		});

		Ok(())
	}

	pub fn delete_file(&mut self, path: &str) -> Result<bool, VfsError> {
		let normalized = self.assert_valid_path(path)?;
		let node = match self.nodes.get(&normalized) {
			Some(n) => n.clone(),
			None => return Ok(false),
		};
		Self::assert_is_file(&normalized, &node)?;

		self.total_size -= node.size;
		self.file_count -= 1;
		self.nodes.remove(&normalized);
		self.history.remove(&normalized);

		self.pending_events
			.push(VfsEvent::Delete { path: normalized });

		Ok(true)
	}

	// -- Directory operations ---------------------------------------------

	pub fn mkdir(&mut self, path: &str, recursive: bool) -> Result<(), VfsError> {
		let normalized = self.assert_valid_path(path)?;
		if normalized == VFS_ROOT {
			return Ok(());
		}

		if let Some(existing) = self.nodes.get(&normalized) {
			if existing.node_type == NodeType::Directory {
				return Ok(());
			}
			return Err(VfsError::NotADirectory(format!(
				"Path exists and is not a directory: {}",
				normalized
			)));
		}

		if recursive {
			self.create_parents(&normalized);
		} else {
			self.ensure_parent_exists(&normalized)?;
		}

		self.assert_node_limit()?;
		let ts = Self::now();
		self.nodes.insert(
			normalized.clone(),
			InternalNode {
				node_type: NodeType::Directory,
				content_type: ContentType::Text,
				text: None,
				data: None,
				size: 0,
				created_at: ts,
				modified_at: ts,
			},
		);
		self.dir_count += 1;

		self.pending_events
			.push(VfsEvent::Mkdir { path: normalized });

		Ok(())
	}

	pub fn readdir(
		&self,
		path: &str,
		recursive: bool,
	) -> Result<Vec<DirEntryResult>, VfsError> {
		let normalized = self.assert_valid_path(path)?;
		let node = self.assert_node_exists(&normalized)?;
		Self::assert_is_directory(&normalized, node)?;

		if recursive {
			let descendants = self.get_descendants(&normalized);
			let prefix = if normalized == VFS_ROOT {
				VFS_ROOT.to_string()
			} else {
				format!("{}/", normalized)
			};
			let mut entries: Vec<DirEntryResult> = Vec::new();
			for p in &descendants {
				let n = self.get_node(p)?;
				let relative_name = &p[prefix.len()..];
				entries.push(DirEntryResult {
					name: relative_name.to_string(),
					node_type: n.node_type.as_str().to_string(),
				});
			}
			return Ok(entries);
		}

		let children = self.get_direct_children(&normalized);
		let mut entries: Vec<DirEntryResult> = Vec::new();
		for child_path in &children {
			let n = self.get_node(child_path)?;
			entries.push(DirEntryResult {
				name: base_name(child_path).to_string(),
				node_type: n.node_type.as_str().to_string(),
			});
		}
		Ok(entries)
	}

	pub fn rmdir(&mut self, path: &str, recursive: bool) -> Result<bool, VfsError> {
		let normalized = self.assert_valid_path(path)?;
		if normalized == VFS_ROOT {
			return Err(VfsError::InvalidOperation(
				"Cannot delete root directory".to_string(),
			));
		}

		let node = match self.nodes.get(&normalized) {
			Some(n) => n.clone(),
			None => return Ok(false),
		};
		Self::assert_is_directory(&normalized, &node)?;

		let children = self.get_direct_children(&normalized);

		if !children.is_empty() && !recursive {
			return Err(VfsError::NotEmpty(format!(
				"Directory is not empty: {}",
				normalized
			)));
		}

		if recursive {
			let descendants = self.get_descendants(&normalized);
			for desc in &descendants {
				if let Some(desc_node) = self.nodes.get(desc) {
					match desc_node.node_type {
						NodeType::File => {
							self.total_size -= desc_node.size;
							self.file_count -= 1;
							self.history.remove(desc);
						}
						NodeType::Directory => {
							self.dir_count -= 1;
						}
					}
				}
				self.nodes.remove(desc);
			}
		}

		self.dir_count -= 1;
		self.nodes.remove(&normalized);

		self.pending_events
			.push(VfsEvent::Delete { path: normalized });

		Ok(true)
	}

	// -- Navigation -------------------------------------------------------

	pub fn stat(&self, path: &str) -> Result<StatResult, VfsError> {
		let normalized = self.assert_valid_path(path)?;
		let node = self.assert_node_exists(&normalized)?;

		Ok(StatResult {
			path: normalized,
			node_type: node.node_type.as_str().to_string(),
			size: node.size,
			created_at: node.created_at,
			modified_at: node.modified_at,
		})
	}

	pub fn exists(&self, path: &str) -> Result<bool, VfsError> {
		// TS version uses resolve (normalize only), no validation
		let normalized =
			normalize_path(path).map_err(|_| VfsError::InvalidPath(path.to_string()))?;
		Ok(self.nodes.contains_key(&normalized))
	}

	pub fn rename(&mut self, old_path: &str, new_path: &str) -> Result<(), VfsError> {
		let normalized_old = self.assert_valid_path(old_path)?;
		let normalized_new = self.assert_valid_path(new_path)?;

		if normalized_old == VFS_ROOT {
			return Err(VfsError::InvalidOperation(
				"Cannot rename root directory".to_string(),
			));
		}

		if normalized_old == normalized_new {
			return Ok(());
		}

		let node = self.assert_node_exists(&normalized_old)?.clone();

		// Cannot move directory into its own descendant
		if node.node_type == NodeType::Directory
			&& normalized_new.starts_with(&format!("{}/", normalized_old))
		{
			return Err(VfsError::InvalidOperation(format!(
				"Cannot move directory into its own descendant: {} -> {}",
				normalized_old, normalized_new
			)));
		}

		if self.nodes.contains_key(&normalized_new) {
			return Err(VfsError::AlreadyExists(format!(
				"Destination already exists: {}",
				normalized_new
			)));
		}

		self.ensure_parent_exists(&normalized_new)?;

		if node.node_type == NodeType::Directory {
			let descendants = self.get_descendants(&normalized_old);
			for desc in &descendants {
				let desc_node = self.get_node(desc)?.clone();
				let new_desc_path =
					format!("{}{}", normalized_new, &desc[normalized_old.len()..]);
				self.nodes.insert(new_desc_path.clone(), desc_node);
				self.nodes.remove(desc);

				// Transfer history
				if let Some(hist) = self.history.remove(desc) {
					self.history.insert(new_desc_path, hist);
				}
			}
		}

		let ts = Self::now();
		let mut moved_node = node;
		moved_node.modified_at = ts;
		self.nodes.insert(normalized_new.clone(), moved_node);
		self.nodes.remove(&normalized_old);

		// Transfer history for the node itself
		if let Some(hist) = self.history.remove(&normalized_old) {
			self.history.insert(normalized_new.clone(), hist);
		}

		self.pending_events.push(VfsEvent::Rename {
			old_path: normalized_old,
			new_path: normalized_new,
		});

		Ok(())
	}

	pub fn copy(
		&mut self,
		src: &str,
		dest: &str,
		overwrite: bool,
		recursive: bool,
	) -> Result<(), VfsError> {
		let normalized_src = self.assert_valid_path(src)?;
		let normalized_dest = self.assert_valid_path(dest)?;

		let src_node = self.assert_node_exists(&normalized_src)?.clone();

		let dest_exists = self.nodes.contains_key(&normalized_dest);
		if dest_exists && !overwrite {
			return Err(VfsError::AlreadyExists(format!(
				"Destination already exists: {}",
				normalized_dest
			)));
		}

		self.ensure_parent_exists(&normalized_dest)?;

		if src_node.node_type == NodeType::File {
			if src_node.content_type == ContentType::Text {
				let text = src_node.text.clone().unwrap_or_default();
				self.write_file(&normalized_dest, &text, Some("text"), false)?;
			} else {
				let data = src_node.data.clone().unwrap_or_default();
				let b64 = BASE64.encode(&data);
				self.write_file(&normalized_dest, &b64, Some("binary"), false)?;
			}
		} else {
			if !recursive {
				return Err(VfsError::InvalidOperation(format!(
					"Cannot copy directory without recursive option: {}",
					normalized_src
				)));
			}

			// Clean dest if overwriting
			if dest_exists && overwrite {
				if let Some(dest_node) = self.nodes.get(&normalized_dest).cloned() {
					if dest_node.node_type == NodeType::Directory {
						self.rmdir(&normalized_dest, true)?;
					} else {
						self.delete_file(&normalized_dest)?;
					}
				}
			}

			self.mkdir(&normalized_dest, false)?;
			let descendants = self.get_descendants(&normalized_src);
			for desc in &descendants {
				let desc_node = self.get_node(desc)?.clone();
				let new_desc_path =
					format!("{}{}", normalized_dest, &desc[normalized_src.len()..]);

				if desc_node.node_type == NodeType::Directory {
					self.mkdir(&new_desc_path, false)?;
				} else if desc_node.content_type == ContentType::Text {
					let text = desc_node.text.clone().unwrap_or_default();
					self.write_file(&new_desc_path, &text, Some("text"), false)?;
				} else {
					let data = desc_node.data.clone().unwrap_or_default();
					let b64 = BASE64.encode(&data);
					self.write_file(&new_desc_path, &b64, Some("binary"), false)?;
				}
			}
		}

		Ok(())
	}

	// -- Query operations -------------------------------------------------

	pub fn glob(&self, patterns: Vec<String>) -> Vec<String> {
		let mut positive_patterns: Vec<String> = Vec::new();
		let mut negative_patterns: Vec<String> = Vec::new();

		for p in &patterns {
			if let Some(stripped) = p.strip_prefix('!') {
				negative_patterns.push(stripped.to_string());
			} else {
				positive_patterns.push(p.clone());
			}
		}

		let match_all = positive_patterns.is_empty();

		let mut results: Vec<String> = Vec::new();
		for (path, node) in &self.nodes {
			if path == VFS_ROOT {
				continue;
			}
			if node.node_type != NodeType::File {
				continue;
			}

			let included =
				match_all || positive_patterns.iter().any(|p| match_glob(path, p));
			if !included {
				continue;
			}

			let excluded = negative_patterns.iter().any(|p| match_glob(path, p));
			if excluded {
				continue;
			}

			results.push(path.clone());
		}
		results.sort();
		results
	}

	pub fn tree(&self, root_path: Option<&str>) -> Result<String, VfsError> {
		let normalized = match root_path {
			Some(p) => self.assert_valid_path(p)?,
			None => VFS_ROOT.to_string(),
		};
		let node = self.assert_node_exists(&normalized)?;
		Self::assert_is_directory(&normalized, node)?;

		let mut lines: Vec<String> = Vec::new();
		let root_name = if normalized == VFS_ROOT {
			VFS_ROOT.to_string()
		} else {
			base_name(&normalized).to_string()
		};
		lines.push(root_name);

		self.build_tree(&normalized, "", &mut lines);

		Ok(lines.join("\n"))
	}

	fn build_tree(&self, dir_path: &str, prefix: &str, lines: &mut Vec<String>) {
		let mut children = self.get_direct_children(dir_path);
		children.sort();

		for (i, child_path) in children.iter().enumerate() {
			let child_node = match self.nodes.get(child_path) {
				Some(n) => n,
				None => continue,
			};
			let is_last = i == children.len() - 1;
			let connector = if is_last {
				"\u{2514}\u{2500}\u{2500} "
			} else {
				"\u{251C}\u{2500}\u{2500} "
			};
			let child_prefix = if is_last {
				"    "
			} else {
				"\u{2502}   "
			};
			let name = base_name(child_path);

			if child_node.node_type == NodeType::Directory {
				lines.push(format!("{}{}{}/", prefix, connector, name));
				self.build_tree(child_path, &format!("{}{}", prefix, child_prefix), lines);
			} else {
				lines.push(format!(
					"{}{}{} ({} bytes)",
					prefix, connector, name, child_node.size
				));
			}
		}
	}

	pub fn du(&self, path: &str) -> Result<u64, VfsError> {
		let normalized = self.assert_valid_path(path)?;
		let node = self.assert_node_exists(&normalized)?;

		if node.node_type == NodeType::File {
			return Ok(node.size);
		}

		let mut total: u64 = 0;
		let descendants = self.get_descendants(&normalized);
		for desc in &descendants {
			if let Some(desc_node) = self.nodes.get(desc.as_str()) {
				if desc_node.node_type == NodeType::File {
					total += desc_node.size;
				}
			}
		}
		Ok(total)
	}

	pub fn search(
		&self,
		query: &str,
		options: SearchOpts,
	) -> Result<SearchOutput, VfsError> {
		let mode = match options.mode.as_str() {
			"regex" => SearchMode::Regex,
			_ => SearchMode::Substring,
		};

		let compiled_regex = if mode == SearchMode::Regex {
			Some(
				Regex::new(query)
					.map_err(|e| VfsError::InvalidOperation(format!("Invalid regex: {}", e)))?,
			)
		} else {
			None
		};

		let search_opts = SearchOptions {
			max_results: options.max_results,
			mode,
			context_before: options.context_before,
			context_after: options.context_after,
			count_only: options.count_only,
		};

		let mut count: usize = 0;
		let mut results: Vec<SearchMatch> = Vec::new();

		for (path, node) in &self.nodes {
			if !options.count_only && results.len() >= options.max_results {
				break;
			}
			if node.node_type != NodeType::File || node.content_type != ContentType::Text {
				continue;
			}
			if let Some(ref glob_pattern) = options.glob {
				if !match_glob(path, glob_pattern) {
					continue;
				}
			}

			let text = node.text.as_deref().unwrap_or("");
			let hit_limit = search_text(
				path,
				text,
				query,
				&search_opts,
				compiled_regex.as_ref(),
				&mut count,
				&mut results,
			);
			if hit_limit {
				break;
			}
		}

		if options.count_only {
			Ok(SearchOutput::Count(count))
		} else {
			Ok(SearchOutput::Results(results))
		}
	}

	// -- History & Diff ---------------------------------------------------

	pub fn history(&self, path: &str) -> Result<Vec<HistoryEntryResult>, VfsError> {
		let normalized = self.assert_valid_path(path)?;
		let node = self.assert_node_exists(&normalized)?;
		Self::assert_is_file(&normalized, node)?;

		let entries = match self.history.get(&normalized) {
			Some(e) if !e.is_empty() => e,
			_ => return Ok(Vec::new()),
		};

		let results: Vec<HistoryEntryResult> = entries
			.iter()
			.map(|e| HistoryEntryResult {
				version: e.version,
				content_type: e.content_type.as_str().to_string(),
				text: e.text.clone(),
				base64: e.data.as_ref().map(|d| BASE64.encode(d)),
				size: e.size,
				timestamp: e.timestamp,
			})
			.collect();

		Ok(results)
	}

	pub fn diff(
		&self,
		old_path: &str,
		new_path: &str,
		context: usize,
	) -> Result<DiffResultOutput, VfsError> {
		let normalized_old = self.assert_valid_path(old_path)?;
		let normalized_new = self.assert_valid_path(new_path)?;
		let old_node = self.assert_node_exists(&normalized_old)?;
		let new_node = self.assert_node_exists(&normalized_new)?;
		Self::assert_is_file(&normalized_old, old_node)?;
		Self::assert_is_file(&normalized_new, new_node)?;

		let old_lines_text = Self::get_text_lines(old_node);
		let new_lines_text = Self::get_text_lines(new_node);

		let old_refs: Vec<&str> = old_lines_text.iter().map(|s| s.as_str()).collect();
		let new_refs: Vec<&str> = new_lines_text.iter().map(|s| s.as_str()).collect();

		let diff_output = compute_diff(
			&old_refs,
			&new_refs,
			context,
			self.limits.max_diff_lines as u32,
		)
		.map_err(|e| VfsError::LimitExceeded(e))?;

		Ok(DiffResultOutput {
			old_path: normalized_old,
			new_path: normalized_new,
			hunks: diff_output.hunks,
			additions: diff_output.additions,
			deletions: diff_output.deletions,
		})
	}

	pub fn diff_versions(
		&self,
		path: &str,
		old_ver: usize,
		new_ver: Option<usize>,
		context: usize,
	) -> Result<DiffResultOutput, VfsError> {
		let normalized = self.assert_valid_path(path)?;
		let node = self.assert_node_exists(&normalized)?;
		Self::assert_is_file(&normalized, node)?;

		let entries = self.history.get(&normalized);
		let hist_len = entries.map_or(0, |e| e.len());
		let current_version = hist_len + 1;

		let get_version_lines = |version: usize| -> Result<Vec<String>, VfsError> {
			if version == current_version {
				return Ok(Self::get_text_lines(node));
			}
			let ents = entries.ok_or_else(|| {
				VfsError::NotFound(format!(
					"Version {} not found for {} (available: 1-{})",
					version, normalized, current_version
				))
			})?;
			if version < 1 || version > ents.len() {
				return Err(VfsError::NotFound(format!(
					"Version {} not found for {} (available: 1-{})",
					version, normalized, current_version
				)));
			}
			Ok(Self::get_history_entry_lines(&ents[version - 1]))
		};

		let actual_new_ver = new_ver.unwrap_or(current_version);
		let old_lines_text = get_version_lines(old_ver)?;
		let new_lines_text = get_version_lines(actual_new_ver)?;

		let old_refs: Vec<&str> = old_lines_text.iter().map(|s| s.as_str()).collect();
		let new_refs: Vec<&str> = new_lines_text.iter().map(|s| s.as_str()).collect();

		let diff_output = compute_diff(
			&old_refs,
			&new_refs,
			context,
			self.limits.max_diff_lines as u32,
		)
		.map_err(|e| VfsError::LimitExceeded(e))?;

		Ok(DiffResultOutput {
			old_path: format!("{}@v{}", normalized, old_ver),
			new_path: format!("{}@v{}", normalized, actual_new_ver),
			hunks: diff_output.hunks,
			additions: diff_output.additions,
			deletions: diff_output.deletions,
		})
	}

	pub fn checkout(&mut self, path: &str, version: usize) -> Result<(), VfsError> {
		let normalized = self.assert_valid_path(path)?;
		let node = self.assert_node_exists(&normalized)?.clone();
		Self::assert_is_file(&normalized, &node)?;

		let hist_len = self.history.get(&normalized).map_or(0, |e| e.len());
		let current_version = hist_len + 1;

		if version == current_version {
			return Ok(()); // Already at this version
		}

		let entries = self.history.get(&normalized).ok_or_else(|| {
			VfsError::NotFound(format!(
				"Version {} not found for {} (available: 1-{})",
				version, normalized, current_version
			))
		})?;

		if version < 1 || version > entries.len() {
			return Err(VfsError::NotFound(format!(
				"Version {} not found for {} (available: 1-{})",
				version, normalized, current_version
			)));
		}

		let entry = entries[version - 1].clone();

		// Record current state as history before reverting
		self.record_history(&normalized, &node);

		let ts = Self::now();
		if entry.content_type == ContentType::Binary {
			let binary = entry.data.clone().unwrap_or_default();
			let size_delta = binary.len() as i64 - node.size as i64;
			self.nodes.insert(
				normalized.clone(),
				InternalNode {
					node_type: NodeType::File,
					content_type: ContentType::Binary,
					text: None,
					data: Some(binary.clone()),
					size: binary.len() as u64,
					created_at: node.created_at,
					modified_at: ts,
				},
			);
			self.total_size = (self.total_size as i64 + size_delta) as u64;
		} else {
			let text = entry.text.clone().unwrap_or_default();
			let new_size = text.len() as u64;
			let size_delta = new_size as i64 - node.size as i64;
			self.nodes.insert(
				normalized,
				InternalNode {
					node_type: NodeType::File,
					content_type: ContentType::Text,
					text: Some(text),
					data: None,
					size: new_size,
					created_at: node.created_at,
					modified_at: ts,
				},
			);
			self.total_size = (self.total_size as i64 + size_delta) as u64;
		}

		Ok(())
	}

	// -- Helpers for diff -------------------------------------------------

	fn get_text_lines(node: &InternalNode) -> Vec<String> {
		if node.content_type != ContentType::Text {
			return vec!["[binary content]".to_string()];
		}
		node.text
			.as_deref()
			.unwrap_or("")
			.split('\n')
			.map(|s| s.to_string())
			.collect()
	}

	fn get_history_entry_lines(entry: &HistoryEntryInternal) -> Vec<String> {
		if entry.content_type != ContentType::Text {
			return vec!["[binary content]".to_string()];
		}
		entry
			.text
			.as_deref()
			.unwrap_or("")
			.split('\n')
			.map(|s| s.to_string())
			.collect()
	}

	// -- Snapshot & Restore -----------------------------------------------

	pub fn snapshot(&self) -> SnapshotData {
		let mut files: Vec<SnapshotFile> = Vec::new();
		let mut directories: Vec<SnapshotDir> = Vec::new();

		for (path, node) in &self.nodes {
			if path == VFS_ROOT && node.node_type == NodeType::Directory {
				continue;
			}
			if node.node_type == NodeType::File {
				files.push(SnapshotFile {
					path: path.clone(),
					content_type: node.content_type.as_str().to_string(),
					text: node.text.clone(),
					base64: node.data.as_ref().map(|d| BASE64.encode(d)),
					created_at: node.created_at,
					modified_at: node.modified_at,
				});
			} else {
				directories.push(SnapshotDir {
					path: path.clone(),
					created_at: node.created_at,
					modified_at: node.modified_at,
				});
			}
		}

		SnapshotData { files, directories }
	}

	pub fn restore(&mut self, snap: SnapshotData) -> Result<(), VfsError> {
		// Validate all entries against limits before committing
		let mut total_size: u64 = 0;
		let mut total_nodes: usize = 1; // root

		let mut sorted_dirs = snap.directories.clone();
		sorted_dirs.sort_by_key(|d| {
			normalize_path(&d.path)
				.map(|p| path_depth(&p))
				.unwrap_or(0)
		});

		for dir in &sorted_dirs {
			let normalized = normalize_path(&dir.path)
				.map_err(|_| VfsError::InvalidPath(format!("Snapshot restore failed: invalid path {}", dir.path)))?;
			if let Some(err) = validate_path(&normalized, &self.limits) {
				return Err(VfsError::InvalidPath(format!(
					"Snapshot restore failed: {}",
					err
				)));
			}
			total_nodes += 1;
		}

		for file in &snap.files {
			let normalized = normalize_path(&file.path)
				.map_err(|_| VfsError::InvalidPath(format!("Snapshot restore failed: invalid path {}", file.path)))?;
			if let Some(err) = validate_path(&normalized, &self.limits) {
				return Err(VfsError::InvalidPath(format!(
					"Snapshot restore failed: {}",
					err
				)));
			}

			let file_size: u64 = if file.content_type == "binary" {
				if let Some(ref b64) = file.base64 {
					(b64.len() as f64 * 0.75) as u64
				} else {
					0
				}
			} else {
				file.text.as_deref().unwrap_or("").len() as u64
			};

			if file_size > self.limits.max_file_size {
				return Err(VfsError::LimitExceeded(format!(
					"Snapshot restore failed: file size {} exceeds limit ({}): {}",
					file_size, self.limits.max_file_size, file.path
				)));
			}
			total_size += file_size;
			total_nodes += 1;
		}

		// Validate parent directories exist for every file
		let dir_paths: std::collections::HashSet<String> = sorted_dirs
			.iter()
			.filter_map(|d| normalize_path(&d.path).ok())
			.collect();

		for file in &snap.files {
			let normalized = normalize_path(&file.path).unwrap();
			let ancestors = ancestor_paths(&normalized);
			for ancestor in &ancestors {
				if ancestor == VFS_ROOT {
					continue;
				}
				if !dir_paths.contains(ancestor) {
					return Err(VfsError::InvalidOperation(format!(
						"Snapshot restore failed: missing parent directory \"{}\" for file \"{}\"",
						ancestor, file.path
					)));
				}
			}
		}

		if total_nodes > self.limits.max_node_count {
			return Err(VfsError::LimitExceeded(format!(
				"Snapshot restore failed: node count {} exceeds limit ({})",
				total_nodes, self.limits.max_node_count
			)));
		}

		if total_size > self.limits.max_total_size {
			return Err(VfsError::LimitExceeded(format!(
				"Snapshot restore failed: total size {} exceeds limit ({})",
				total_size, self.limits.max_total_size
			)));
		}

		// Validation passed — commit
		self.do_clear();

		for dir in &sorted_dirs {
			let normalized = normalize_path(&dir.path).unwrap();
			self.nodes.insert(
				normalized,
				InternalNode {
					node_type: NodeType::Directory,
					content_type: ContentType::Text,
					text: None,
					data: None,
					size: 0,
					created_at: dir.created_at,
					modified_at: dir.modified_at,
				},
			);
			self.dir_count += 1;
		}

		for file in &snap.files {
			let normalized = normalize_path(&file.path).unwrap();
			if file.content_type == "binary" {
				if let Some(ref b64) = file.base64 {
					let binary = BASE64.decode(b64).unwrap_or_default();
					let sz = binary.len() as u64;
					self.nodes.insert(
						normalized,
						InternalNode {
							node_type: NodeType::File,
							content_type: ContentType::Binary,
							text: None,
							data: Some(binary),
							size: sz,
							created_at: file.created_at,
							modified_at: file.modified_at,
						},
					);
					self.total_size += sz;
				} else {
					self.nodes.insert(
						normalized,
						InternalNode {
							node_type: NodeType::File,
							content_type: ContentType::Binary,
							text: None,
							data: Some(Vec::new()),
							size: 0,
							created_at: file.created_at,
							modified_at: file.modified_at,
						},
					);
				}
			} else {
				let text = file.text.clone().unwrap_or_default();
				let sz = text.len() as u64;
				self.nodes.insert(
					normalized,
					InternalNode {
						node_type: NodeType::File,
						content_type: ContentType::Text,
						text: Some(text),
						data: None,
						size: sz,
						created_at: file.created_at,
						modified_at: file.modified_at,
					},
				);
				self.total_size += sz;
			}
			self.file_count += 1;
		}

		Ok(())
	}

	pub fn clear(&mut self) {
		self.do_clear();
	}

	fn do_clear(&mut self) {
		self.nodes.clear();
		self.history.clear();
		self.total_size = 0;
		self.file_count = 0;
		self.dir_count = 0;

		// Re-init root
		let now = Self::now();
		self.nodes.insert(
			VFS_ROOT.to_string(),
			InternalNode {
				node_type: NodeType::Directory,
				content_type: ContentType::Text,
				text: None,
				data: None,
				size: 0,
				created_at: now,
				modified_at: now,
			},
		);
		self.dir_count = 1;
	}

	// -- Transaction ------------------------------------------------------

	pub fn transaction(&mut self, ops: Vec<TransactionOp>) -> Result<(), VfsError> {
		let snap = self.snapshot();
		for op in &ops {
			let result = match op {
				TransactionOp::WriteFile { path, content } => {
					self.write_file(path, content, None, false)
				}
				TransactionOp::DeleteFile { path } => self.delete_file(path).map(|_| ()),
				TransactionOp::Mkdir { path } => self.mkdir(path, true),
				TransactionOp::Rmdir { path } => self.rmdir(path, true).map(|_| ()),
				TransactionOp::Rename { old_path, new_path } => {
					self.rename(old_path, new_path)
				}
				TransactionOp::Copy { src, dest } => {
					self.copy(src, dest, false, false)
				}
			};
			if let Err(e) = result {
				// Rollback
				let _ = self.restore(snap);
				return Err(e);
			}
		}
		Ok(())
	}

	// -- Events & Metrics -------------------------------------------------

	pub fn drain_events(&mut self) -> Vec<VfsEvent> {
		std::mem::take(&mut self.pending_events)
	}

	pub fn metrics(&self) -> MetricsResult {
		MetricsResult {
			total_size: self.total_size,
			node_count: self.nodes.len(),
			file_count: self.file_count,
			directory_count: self.dir_count,
		}
	}
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;

	fn new_vfs() -> VirtualFs {
		VirtualFs::new(VfsLimits::default(), 50)
	}

	// -- Constructor --

	#[test]
	fn new_vfs_has_root() {
		let vfs = new_vfs();
		assert!(vfs.exists("vfs:///").unwrap());
		let m = vfs.metrics();
		assert_eq!(m.directory_count, 1);
		assert_eq!(m.file_count, 0);
		assert_eq!(m.total_size, 0);
	}

	// -- write_file / read_file --

	#[test]
	fn write_and_read_text_file() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///hello.txt", "hello world", None, false)
			.unwrap();
		let r = vfs.read_file("vfs:///hello.txt").unwrap();
		assert_eq!(r.content_type, "text");
		assert_eq!(r.text.unwrap(), "hello world");
		assert_eq!(r.size, 11);
	}

	#[test]
	fn write_and_read_binary_file() {
		let mut vfs = new_vfs();
		let data = vec![0u8, 1, 2, 255];
		let b64 = BASE64.encode(&data);
		vfs.write_file("vfs:///bin.dat", &b64, Some("binary"), false)
			.unwrap();
		let r = vfs.read_file("vfs:///bin.dat").unwrap();
		assert_eq!(r.content_type, "binary");
		let decoded = BASE64.decode(r.data_base64.unwrap()).unwrap();
		assert_eq!(decoded, data);
	}

	#[test]
	fn write_creates_parents() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///a/b/c.txt", "nested", None, true)
			.unwrap();
		assert!(vfs.exists("vfs:///a").unwrap());
		assert!(vfs.exists("vfs:///a/b").unwrap());
		let r = vfs.read_file("vfs:///a/b/c.txt").unwrap();
		assert_eq!(r.text.unwrap(), "nested");
	}

	#[test]
	fn write_without_parent_fails() {
		let mut vfs = new_vfs();
		let err = vfs
			.write_file("vfs:///noparent/file.txt", "x", None, false)
			.unwrap_err();
		assert!(matches!(err, VfsError::NotFound(_)));
	}

	#[test]
	fn overwrite_existing_file() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "old", None, false).unwrap();
		vfs.write_file("vfs:///f.txt", "new content", None, false)
			.unwrap();
		let r = vfs.read_file("vfs:///f.txt").unwrap();
		assert_eq!(r.text.unwrap(), "new content");
	}

	#[test]
	fn cannot_write_to_root() {
		let mut vfs = new_vfs();
		let err = vfs
			.write_file("vfs:///", "data", None, false)
			.unwrap_err();
		assert!(matches!(err, VfsError::InvalidOperation(_)));
	}

	#[test]
	fn cannot_overwrite_directory_with_file() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///dir", false).unwrap();
		let err = vfs
			.write_file("vfs:///dir", "data", None, false)
			.unwrap_err();
		assert!(matches!(err, VfsError::NotAFile(_)));
	}

	// -- append_file --

	#[test]
	fn append_to_text_file() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///a.txt", "hello", None, false)
			.unwrap();
		vfs.append_file("vfs:///a.txt", " world").unwrap();
		let r = vfs.read_file("vfs:///a.txt").unwrap();
		assert_eq!(r.text.unwrap(), "hello world");
	}

	#[test]
	fn append_to_binary_fails() {
		let mut vfs = new_vfs();
		let b64 = BASE64.encode(&[1, 2, 3]);
		vfs.write_file("vfs:///b.dat", &b64, Some("binary"), false)
			.unwrap();
		let err = vfs.append_file("vfs:///b.dat", "x").unwrap_err();
		assert!(matches!(err, VfsError::InvalidOperation(_)));
	}

	// -- delete_file --

	#[test]
	fn delete_existing_file() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "data", None, false)
			.unwrap();
		assert!(vfs.delete_file("vfs:///f.txt").unwrap());
		assert!(!vfs.exists("vfs:///f.txt").unwrap());
	}

	#[test]
	fn delete_nonexistent_returns_false() {
		let mut vfs = new_vfs();
		assert!(!vfs.delete_file("vfs:///nope.txt").unwrap());
	}

	// -- mkdir --

	#[test]
	fn mkdir_creates_directory() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///mydir", false).unwrap();
		let s = vfs.stat("vfs:///mydir").unwrap();
		assert_eq!(s.node_type, "directory");
	}

	#[test]
	fn mkdir_recursive() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///a/b/c", true).unwrap();
		assert!(vfs.exists("vfs:///a").unwrap());
		assert!(vfs.exists("vfs:///a/b").unwrap());
		assert!(vfs.exists("vfs:///a/b/c").unwrap());
	}

	#[test]
	fn mkdir_idempotent() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///dir", false).unwrap();
		vfs.mkdir("vfs:///dir", false).unwrap(); // no error
	}

	#[test]
	fn mkdir_on_file_fails() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "x", None, false).unwrap();
		let err = vfs.mkdir("vfs:///f.txt", false).unwrap_err();
		assert!(matches!(err, VfsError::NotADirectory(_)));
	}

	// -- readdir --

	#[test]
	fn readdir_lists_children() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///a.txt", "a", None, false).unwrap();
		vfs.write_file("vfs:///b.txt", "b", None, false).unwrap();
		vfs.mkdir("vfs:///sub", false).unwrap();
		let entries = vfs.readdir("vfs:///", false).unwrap();
		assert_eq!(entries.len(), 3);
	}

	#[test]
	fn readdir_recursive() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///sub", false).unwrap();
		vfs.write_file("vfs:///sub/f.txt", "x", None, false)
			.unwrap();
		let entries = vfs.readdir("vfs:///", true).unwrap();
		assert!(entries.len() >= 2); // sub, sub/f.txt
	}

	// -- rmdir --

	#[test]
	fn rmdir_empty() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///dir", false).unwrap();
		assert!(vfs.rmdir("vfs:///dir", false).unwrap());
		assert!(!vfs.exists("vfs:///dir").unwrap());
	}

	#[test]
	fn rmdir_nonempty_fails() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///dir", false).unwrap();
		vfs.write_file("vfs:///dir/f.txt", "x", None, false)
			.unwrap();
		let err = vfs.rmdir("vfs:///dir", false).unwrap_err();
		assert!(matches!(err, VfsError::NotEmpty(_)));
	}

	#[test]
	fn rmdir_recursive() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///dir", false).unwrap();
		vfs.write_file("vfs:///dir/f.txt", "x", None, false)
			.unwrap();
		assert!(vfs.rmdir("vfs:///dir", true).unwrap());
		assert!(!vfs.exists("vfs:///dir").unwrap());
		assert!(!vfs.exists("vfs:///dir/f.txt").unwrap());
	}

	#[test]
	fn rmdir_root_fails() {
		let mut vfs = new_vfs();
		let err = vfs.rmdir("vfs:///", false).unwrap_err();
		assert!(matches!(err, VfsError::InvalidOperation(_)));
	}

	// -- stat --

	#[test]
	fn stat_file() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "hello", None, false)
			.unwrap();
		let s = vfs.stat("vfs:///f.txt").unwrap();
		assert_eq!(s.node_type, "file");
		assert_eq!(s.size, 5);
	}

	#[test]
	fn stat_not_found() {
		let vfs = new_vfs();
		let err = vfs.stat("vfs:///nope").unwrap_err();
		assert!(matches!(err, VfsError::NotFound(_)));
	}

	// -- exists --

	#[test]
	fn exists_works() {
		let mut vfs = new_vfs();
		assert!(vfs.exists("vfs:///").unwrap());
		assert!(!vfs.exists("vfs:///nope").unwrap());
		vfs.write_file("vfs:///f.txt", "x", None, false).unwrap();
		assert!(vfs.exists("vfs:///f.txt").unwrap());
	}

	// -- rename --

	#[test]
	fn rename_file() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///old.txt", "data", None, false)
			.unwrap();
		vfs.rename("vfs:///old.txt", "vfs:///new.txt").unwrap();
		assert!(!vfs.exists("vfs:///old.txt").unwrap());
		let r = vfs.read_file("vfs:///new.txt").unwrap();
		assert_eq!(r.text.unwrap(), "data");
	}

	#[test]
	fn rename_directory_with_descendants() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///src", false).unwrap();
		vfs.write_file("vfs:///src/a.txt", "a", None, false)
			.unwrap();
		vfs.rename("vfs:///src", "vfs:///dst").unwrap();
		assert!(!vfs.exists("vfs:///src").unwrap());
		assert!(vfs.exists("vfs:///dst").unwrap());
		assert!(vfs.exists("vfs:///dst/a.txt").unwrap());
	}

	#[test]
	fn rename_same_path_noop() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "data", None, false)
			.unwrap();
		vfs.rename("vfs:///f.txt", "vfs:///f.txt").unwrap();
		assert!(vfs.exists("vfs:///f.txt").unwrap());
	}

	#[test]
	fn rename_to_existing_fails() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///a.txt", "a", None, false).unwrap();
		vfs.write_file("vfs:///b.txt", "b", None, false).unwrap();
		let err = vfs.rename("vfs:///a.txt", "vfs:///b.txt").unwrap_err();
		assert!(matches!(err, VfsError::AlreadyExists(_)));
	}

	#[test]
	fn rename_into_descendant_fails() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///dir", false).unwrap();
		vfs.mkdir("vfs:///dir/sub", false).unwrap();
		let err = vfs
			.rename("vfs:///dir", "vfs:///dir/sub/new")
			.unwrap_err();
		assert!(matches!(err, VfsError::InvalidOperation(_)));
	}

	// -- copy --

	#[test]
	fn copy_file() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///src.txt", "data", None, false)
			.unwrap();
		vfs.copy("vfs:///src.txt", "vfs:///dst.txt", false, false)
			.unwrap();
		let r = vfs.read_file("vfs:///dst.txt").unwrap();
		assert_eq!(r.text.unwrap(), "data");
	}

	#[test]
	fn copy_directory_recursive() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///src", false).unwrap();
		vfs.write_file("vfs:///src/a.txt", "a", None, false)
			.unwrap();
		vfs.copy("vfs:///src", "vfs:///dst", false, true).unwrap();
		assert!(vfs.exists("vfs:///dst").unwrap());
		let r = vfs.read_file("vfs:///dst/a.txt").unwrap();
		assert_eq!(r.text.unwrap(), "a");
	}

	#[test]
	fn copy_dir_without_recursive_fails() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///src", false).unwrap();
		let err = vfs
			.copy("vfs:///src", "vfs:///dst", false, false)
			.unwrap_err();
		assert!(matches!(err, VfsError::InvalidOperation(_)));
	}

	// -- glob --

	#[test]
	fn glob_positive_pattern() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///a.txt", "a", None, false).unwrap();
		vfs.write_file("vfs:///b.rs", "b", None, false).unwrap();
		let results = vfs.glob(vec!["vfs:///*.txt".to_string()]);
		assert_eq!(results, vec!["vfs:///a.txt"]);
	}

	#[test]
	fn glob_negation_pattern() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///a.txt", "a", None, false).unwrap();
		vfs.write_file("vfs:///b.txt", "b", None, false).unwrap();
		vfs.write_file("vfs:///c.rs", "c", None, false).unwrap();
		let results = vfs.glob(vec!["!vfs:///*.rs".to_string()]);
		assert!(results.contains(&"vfs:///a.txt".to_string()));
		assert!(results.contains(&"vfs:///b.txt".to_string()));
		assert!(!results.contains(&"vfs:///c.rs".to_string()));
	}

	// -- tree --

	#[test]
	fn tree_basic() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///a.txt", "a", None, false).unwrap();
		vfs.mkdir("vfs:///dir", false).unwrap();
		vfs.write_file("vfs:///dir/b.txt", "bb", None, false)
			.unwrap();
		let t = vfs.tree(None).unwrap();
		assert!(t.contains("a.txt"));
		assert!(t.contains("dir/"));
		assert!(t.contains("b.txt"));
	}

	// -- du --

	#[test]
	fn du_file() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "hello", None, false)
			.unwrap();
		assert_eq!(vfs.du("vfs:///f.txt").unwrap(), 5);
	}

	#[test]
	fn du_directory() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///d", false).unwrap();
		vfs.write_file("vfs:///d/a.txt", "aaa", None, false)
			.unwrap();
		vfs.write_file("vfs:///d/b.txt", "bb", None, false)
			.unwrap();
		assert_eq!(vfs.du("vfs:///d").unwrap(), 5);
	}

	// -- search --

	#[test]
	fn search_substring() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "hello world\nfoo bar\nhello again", None, false)
			.unwrap();
		let result = vfs
			.search(
				"hello",
				SearchOpts {
					mode: "substring".to_string(),
					..SearchOpts::default()
				},
			)
			.unwrap();
		match result {
			SearchOutput::Results(r) => assert_eq!(r.len(), 2),
			_ => panic!("expected results"),
		}
	}

	#[test]
	fn search_count_only() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "aaa\nbbb\naaa", None, false)
			.unwrap();
		let result = vfs
			.search(
				"aaa",
				SearchOpts {
					count_only: true,
					..SearchOpts::default()
				},
			)
			.unwrap();
		match result {
			SearchOutput::Count(c) => assert_eq!(c, 2),
			_ => panic!("expected count"),
		}
	}

	// -- history --

	#[test]
	fn history_records_versions() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "v1", None, false).unwrap();
		vfs.write_file("vfs:///f.txt", "v2", None, false).unwrap();
		vfs.write_file("vfs:///f.txt", "v3", None, false).unwrap();
		let h = vfs.history("vfs:///f.txt").unwrap();
		// v1 and v2 are in history (v3 is current)
		assert_eq!(h.len(), 2);
		assert_eq!(h[0].text.as_deref(), Some("v1"));
		assert_eq!(h[1].text.as_deref(), Some("v2"));
	}

	// -- checkout --

	#[test]
	fn checkout_restores_version() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "v1", None, false).unwrap();
		vfs.write_file("vfs:///f.txt", "v2", None, false).unwrap();
		vfs.checkout("vfs:///f.txt", 1).unwrap();
		let r = vfs.read_file("vfs:///f.txt").unwrap();
		assert_eq!(r.text.unwrap(), "v1");
	}

	// -- diff --

	#[test]
	fn diff_two_files() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///a.txt", "hello\nworld", None, false)
			.unwrap();
		vfs.write_file("vfs:///b.txt", "hello\nearth", None, false)
			.unwrap();
		let d = vfs.diff("vfs:///a.txt", "vfs:///b.txt", 3).unwrap();
		assert_eq!(d.additions, 1);
		assert_eq!(d.deletions, 1);
	}

	// -- diff_versions --

	#[test]
	fn diff_versions_works() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "line1\nline2", None, false)
			.unwrap();
		vfs.write_file("vfs:///f.txt", "line1\nline2\nline3", None, false)
			.unwrap();
		let d = vfs.diff_versions("vfs:///f.txt", 1, None, 3).unwrap();
		assert_eq!(d.additions, 1);
		assert_eq!(d.deletions, 0);
	}

	// -- snapshot / restore --

	#[test]
	fn snapshot_and_restore() {
		let mut vfs = new_vfs();
		vfs.mkdir("vfs:///dir", false).unwrap();
		vfs.write_file("vfs:///dir/f.txt", "data", None, false)
			.unwrap();

		let snap = vfs.snapshot();
		vfs.clear();
		assert!(!vfs.exists("vfs:///dir").unwrap());

		vfs.restore(snap).unwrap();
		assert!(vfs.exists("vfs:///dir").unwrap());
		let r = vfs.read_file("vfs:///dir/f.txt").unwrap();
		assert_eq!(r.text.unwrap(), "data");
	}

	// -- clear --

	#[test]
	fn clear_resets_to_root() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "x", None, false).unwrap();
		vfs.clear();
		assert!(!vfs.exists("vfs:///f.txt").unwrap());
		assert!(vfs.exists("vfs:///").unwrap());
		let m = vfs.metrics();
		assert_eq!(m.file_count, 0);
		assert_eq!(m.directory_count, 1);
	}

	// -- transaction --

	#[test]
	fn transaction_commits_on_success() {
		let mut vfs = new_vfs();
		vfs.transaction(vec![
			TransactionOp::Mkdir {
				path: "vfs:///dir".to_string(),
			},
			TransactionOp::WriteFile {
				path: "vfs:///dir/f.txt".to_string(),
				content: "hello".to_string(),
			},
		])
		.unwrap();
		assert!(vfs.exists("vfs:///dir/f.txt").unwrap());
	}

	#[test]
	fn transaction_rolls_back_on_error() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///existing.txt", "keep me", None, false)
			.unwrap();

		let err = vfs.transaction(vec![
			TransactionOp::WriteFile {
				path: "vfs:///new.txt".to_string(),
				content: "new data".to_string(),
			},
			// This should fail — writing to root
			TransactionOp::WriteFile {
				path: "vfs:///".to_string(),
				content: "bad".to_string(),
			},
		]);

		assert!(err.is_err());
		// new.txt should not exist after rollback
		assert!(!vfs.exists("vfs:///new.txt").unwrap());
		// existing.txt should still be there
		let r = vfs.read_file("vfs:///existing.txt").unwrap();
		assert_eq!(r.text.unwrap(), "keep me");
	}

	// -- events --

	#[test]
	fn events_are_emitted() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///f.txt", "x", None, false).unwrap();
		vfs.mkdir("vfs:///dir", false).unwrap();
		vfs.delete_file("vfs:///f.txt").unwrap();

		let events = vfs.drain_events();
		assert_eq!(events.len(), 3);
		assert!(matches!(&events[0], VfsEvent::Write { .. }));
		assert!(matches!(&events[1], VfsEvent::Mkdir { .. }));
		assert!(matches!(&events[2], VfsEvent::Delete { .. }));

		// Second drain should be empty
		let events2 = vfs.drain_events();
		assert!(events2.is_empty());
	}

	// -- metrics --

	#[test]
	fn metrics_track_sizes() {
		let mut vfs = new_vfs();
		vfs.write_file("vfs:///a.txt", "aaa", None, false)
			.unwrap();
		vfs.mkdir("vfs:///dir", false).unwrap();
		vfs.write_file("vfs:///dir/b.txt", "bb", None, false)
			.unwrap();

		let m = vfs.metrics();
		assert_eq!(m.total_size, 5); // 3 + 2
		assert_eq!(m.file_count, 2);
		assert_eq!(m.directory_count, 2); // root + dir
		assert_eq!(m.node_count, 4); // root + dir + a.txt + b.txt
	}

	// -- limits --

	#[test]
	fn file_size_limit_enforced() {
		let limits = VfsLimits {
			max_file_size: 10,
			..VfsLimits::default()
		};
		let mut vfs = VirtualFs::new(limits, 50);
		let err = vfs
			.write_file("vfs:///big.txt", "12345678901", None, false)
			.unwrap_err();
		assert!(matches!(err, VfsError::LimitExceeded(_)));
	}

	#[test]
	fn total_size_limit_enforced() {
		let limits = VfsLimits {
			max_total_size: 10,
			..VfsLimits::default()
		};
		let mut vfs = VirtualFs::new(limits, 50);
		vfs.write_file("vfs:///a.txt", "12345", None, false)
			.unwrap();
		let err = vfs
			.write_file("vfs:///b.txt", "123456", None, false)
			.unwrap_err();
		assert!(matches!(err, VfsError::LimitExceeded(_)));
	}

	#[test]
	fn node_count_limit_enforced() {
		let limits = VfsLimits {
			max_node_count: 3, // root + 2 nodes max
			..VfsLimits::default()
		};
		let mut vfs = VirtualFs::new(limits, 50);
		vfs.write_file("vfs:///a.txt", "a", None, false).unwrap();
		vfs.write_file("vfs:///b.txt", "b", None, false).unwrap();
		let err = vfs
			.write_file("vfs:///c.txt", "c", None, false)
			.unwrap_err();
		assert!(matches!(err, VfsError::LimitExceeded(_)));
	}
}
