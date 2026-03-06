//! Host Filesystem Tools — 9 tools for file operations.
//!
//! Ports `src/ai/tools/host/filesystem.ts` to Rust.
//! All paths are sandboxed to a configured working directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use regex::Regex;
use serde_json::Value;

use crate::error::{SimseError, ToolErrorCode};
use crate::tools::host::fuzzy_edit::fuzzy_match;
use crate::tools::registry::ToolRegistry;
use crate::tools::types::{
	ToolAnnotations, ToolCategory, ToolDefinition, ToolHandler, ToolParameter,
};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Configuration for filesystem tool registration.
pub struct FilesystemToolOptions {
	/// The base directory; all paths are resolved relative to this.
	pub working_directory: PathBuf,
	/// Optional list of additional allowed paths beyond the working directory.
	pub allowed_paths: Option<Vec<PathBuf>>,
}

// ---------------------------------------------------------------------------
// Path sandboxing
// ---------------------------------------------------------------------------

/// Resolve `input_path` relative to `working_directory` and ensure it doesn't
/// escape the sandbox. Returns the canonicalized absolute path.
pub fn resolve_sandboxed(
	working_directory: &Path,
	input_path: &str,
	allowed_paths: Option<&[PathBuf]>,
) -> Result<PathBuf, SimseError> {
	let resolved = working_directory.join(input_path);

	// Normalize the path (not canonicalize — the file may not exist yet)
	let resolved = normalize_path(&resolved);

	// Check it starts with the working directory
	let wd_normalized = normalize_path(working_directory);
	if !resolved.starts_with(&wd_normalized) {
		// Check allowed_paths
		if let Some(allowed) = allowed_paths {
			let in_allowed = allowed.iter().any(|ap| {
				let ap_normalized = normalize_path(ap);
				resolved.starts_with(&ap_normalized)
			});
			if in_allowed {
				return Ok(resolved);
			}
		}
		return Err(SimseError::tool(
			ToolErrorCode::ExecutionFailed,
			format!(
				"Path \"{}\" escapes the working directory \"{}\"",
				input_path,
				working_directory.display()
			),
		));
	}

	Ok(resolved)
}

/// Normalize a path by resolving `.` and `..` components without touching
/// the filesystem (so the file need not exist).
fn normalize_path(path: &Path) -> PathBuf {
	let mut components = Vec::new();
	for component in path.components() {
		match component {
			std::path::Component::ParentDir => {
				components.pop();
			}
			std::path::Component::CurDir => {}
			other => components.push(other),
		}
	}
	components.iter().collect()
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
// Recursive directory listing
// ---------------------------------------------------------------------------

struct DirEntry {
	name: String,
	entry_type: &'static str,
}

fn list_dir_recursive(
	dir: PathBuf,
	base_dir: PathBuf,
	max_depth: usize,
	current_depth: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<DirEntry>, SimseError>> + Send>>
{
	Box::pin(async move {
		if current_depth > max_depth {
			return Ok(Vec::new());
		}

		let mut entries = Vec::new();
		let mut read_dir = tokio::fs::read_dir(&dir).await?;

		while let Some(item) = read_dir.next_entry().await? {
			let file_type = item.file_type().await?;
			let item_path = item.path();
			let rel_path = item_path
				.strip_prefix(&base_dir)
				.unwrap_or(&item_path)
				.to_string_lossy()
				.replace('\\', "/");

			if file_type.is_dir() {
				entries.push(DirEntry {
					name: rel_path,
					entry_type: "directory",
				});
				if current_depth < max_depth {
					let sub_entries = list_dir_recursive(
						item_path,
						base_dir.clone(),
						max_depth,
						current_depth + 1,
					)
					.await?;
					entries.extend(sub_entries);
				}
			} else {
				entries.push(DirEntry {
					name: rel_path,
					entry_type: "file",
				});
			}
		}

		Ok(entries)
	})
}

// ---------------------------------------------------------------------------
// Public registration
// ---------------------------------------------------------------------------

/// Register 9 filesystem tools on the given registry.
pub fn register_filesystem_tools(registry: &mut ToolRegistry, options: FilesystemToolOptions) {
	let wd = Arc::new(options.working_directory);
	let allowed = Arc::new(options.allowed_paths);

	// Helper to create a sandboxing closure
	let sandbox = {
		let wd = Arc::clone(&wd);
		let allowed = Arc::clone(&allowed);
		move |input: &str| -> Result<PathBuf, SimseError> {
			resolve_sandboxed(&wd, input, allowed.as_deref())
		}
	};

	// -------------------------------------------------------------------
	// 1. fs_read — read file with optional line range
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param("string", "File path (relative to working directory)", true),
		);
		parameters.insert(
			"offset".to_string(),
			param("number", "Starting line number (1-based, default: 1)", false),
		);
		parameters.insert(
			"limit".to_string(),
			param("number", "Maximum number of lines to return", false),
		);

		let definition = ToolDefinition {
			name: "fs_read".to_string(),
			description: "Read a file from the filesystem. Supports optional offset and limit for reading specific line ranges.".to_string(),
			parameters,
			category: ToolCategory::Read,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let sandbox = sandbox.clone();
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let sandbox = sandbox.clone();
			Box::pin(async move {
				let path_str = args
					.get("path")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let file_path = sandbox(path_str)?;
				let content = tokio::fs::read_to_string(&file_path).await.map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Failed to read {}: {}", path_str, e),
					)
				})?;

				let lines: Vec<&str> = content.split('\n').collect();

				let offset = args
					.get("offset")
					.and_then(|v| v.as_u64())
					.map(|v| v.max(1) as usize)
					.unwrap_or(1);
				let limit = args
					.get("limit")
					.and_then(|v| v.as_u64())
					.map(|v| v as usize)
					.unwrap_or(lines.len());

				let start = offset - 1; // Convert to 0-based
				let end = (start + limit).min(lines.len());
				let sliced = &lines[start..end];

				let formatted: Vec<String> = sliced
					.iter()
					.enumerate()
					.map(|(i, line)| format!("{:>6}\t{}", start + i + 1, line))
					.collect();

				Ok(formatted.join("\n"))
			})
		});

		registry.register_mut(definition, handler);
	}

	// -------------------------------------------------------------------
	// 2. fs_write — create or overwrite a file
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param("string", "File path (relative to working directory)", true),
		);
		parameters.insert(
			"content".to_string(),
			param("string", "The content to write", true),
		);

		let definition = ToolDefinition {
			name: "fs_write".to_string(),
			description:
				"Write content to a file. Creates parent directories automatically. Overwrites existing files."
					.to_string(),
			parameters,
			category: ToolCategory::Edit,
			annotations: Some(ToolAnnotations {
				destructive: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let sandbox = sandbox.clone();
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let sandbox = sandbox.clone();
			Box::pin(async move {
				let path_str = args
					.get("path")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let content = args
					.get("content")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let file_path = sandbox(path_str)?;

				// Auto-create parent directories
				if let Some(parent) = file_path.parent() {
					tokio::fs::create_dir_all(parent).await.map_err(|e| {
						SimseError::tool(
							ToolErrorCode::ExecutionFailed,
							format!("Failed to create directories: {}", e),
						)
					})?;
				}

				let bytes = content.len();
				tokio::fs::write(&file_path, content).await.map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Failed to write {}: {}", path_str, e),
					)
				})?;

				Ok(format!("Wrote {} bytes to {}", bytes, path_str))
			})
		});

		registry.register_mut(definition, handler);
	}

	// -------------------------------------------------------------------
	// 3. fs_edit — fuzzy edit using fuzzy_match
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param("string", "File path (relative to working directory)", true),
		);
		parameters.insert(
			"old_string".to_string(),
			param("string", "The text to find and replace", true),
		);
		parameters.insert(
			"new_string".to_string(),
			param("string", "The replacement text", true),
		);

		let definition = ToolDefinition {
			name: "fs_edit".to_string(),
			description: "Edit a file by replacing text. Uses 5-strategy fuzzy matching: exact, line-trimmed, whitespace-normalized, indentation-flexible, and block-anchor with Levenshtein distance.".to_string(),
			parameters,
			category: ToolCategory::Edit,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let sandbox = sandbox.clone();
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let sandbox = sandbox.clone();
			Box::pin(async move {
				let path_str = args
					.get("path")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let old_string = args
					.get("old_string")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let new_string = args
					.get("new_string")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let file_path = sandbox(path_str)?;

				let content = tokio::fs::read_to_string(&file_path).await.map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Failed to read {}: {}", path_str, e),
					)
				})?;

				let result = fuzzy_match(&content, old_string, new_string).ok_or_else(|| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						"No match found for the provided old_string in the file. Ensure the text exists in the file.".to_string(),
					)
				})?;

				tokio::fs::write(&file_path, &result.replaced)
					.await
					.map_err(|e| {
						SimseError::tool(
							ToolErrorCode::ExecutionFailed,
							format!("Failed to write {}: {}", path_str, e),
						)
					})?;

				Ok(format!(
					"Edited {} using strategy: {}",
					path_str, result.strategy
				))
			})
		});

		registry.register_mut(definition, handler);
	}

	// -------------------------------------------------------------------
	// 4. fs_glob — find files using glob
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"pattern".to_string(),
			param(
				"string",
				"Glob pattern (e.g. \"**/*.ts\", \"src/**/*.json\")",
				true,
			),
		);

		let definition = ToolDefinition {
			name: "fs_glob".to_string(),
			description: "Find files matching a glob pattern within the working directory. Returns up to 1000 results.".to_string(),
			parameters,
			category: ToolCategory::Search,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let wd = Arc::clone(&wd);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move {
				let pattern = args
					.get("pattern")
					.and_then(|v| v.as_str())
					.unwrap_or("**/*");

				let full_pattern = wd.join(pattern);
				let full_pattern_str = full_pattern.to_string_lossy().replace('\\', "/");

				let mut matches: Vec<String> = Vec::new();
				let limit = 1000;

				for entry in glob::glob(&full_pattern_str).map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Invalid glob pattern: {}", e),
					)
				})? {
					if matches.len() >= limit {
						break;
					}
					if let Ok(path) = entry
						&& let Ok(rel) = path.strip_prefix(&*wd) {
							let normalized = rel.to_string_lossy().replace('\\', "/");
							matches.push(normalized);
						}
				}

				if matches.is_empty() {
					return Ok("No files found matching the pattern.".to_string());
				}

				matches.sort();

				let mut output = matches.join("\n");
				if matches.len() >= limit {
					output.push_str(&format!("\n\n(Results limited to {} entries)", limit));
				}
				Ok(output)
			})
		});

		registry.register_mut(definition, handler);
	}

	// -------------------------------------------------------------------
	// 5. fs_grep — regex search file contents
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"pattern".to_string(),
			param("string", "Regular expression pattern to search for", true),
		);
		parameters.insert(
			"path".to_string(),
			param(
				"string",
				"File or directory path to search in (default: working directory)",
				false,
			),
		);
		parameters.insert(
			"glob".to_string(),
			param(
				"string",
				"Glob pattern to filter files (e.g. \"*.ts\")",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "fs_grep".to_string(),
			description: "Search file contents using a regex pattern. Searches all files in the working directory or a specific path. Returns up to 500 results.".to_string(),
			parameters,
			category: ToolCategory::Search,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let wd = Arc::clone(&wd);
		let sandbox = sandbox.clone();
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			let sandbox = sandbox.clone();
			Box::pin(async move {
				let search_pattern = args
					.get("pattern")
					.and_then(|v| v.as_str())
					.unwrap_or("");

				let regex = Regex::new(search_pattern).map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Invalid regex pattern: {}", e),
					)
				})?;

				let search_path = match args.get("path").and_then(|v| v.as_str()) {
					Some(p) => sandbox(p)?,
					None => (*wd).clone(),
				};

				let file_glob = args
					.get("glob")
					.and_then(|v| v.as_str())
					.unwrap_or("**/*");

				let full_pattern = search_path.join(file_glob);
				let full_pattern_str = full_pattern.to_string_lossy().replace('\\', "/");

				let limit = 500;
				let mut results: Vec<String> = Vec::new();

				for entry in glob::glob(&full_pattern_str).map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Invalid glob pattern: {}", e),
					)
				})? {
					if results.len() >= limit {
						break;
					}

					let path = match entry {
						Ok(p) => p,
						Err(_) => continue,
					};

					// Get metadata, skip non-files and large files
					let metadata = match tokio::fs::metadata(&path).await {
						Ok(m) => m,
						Err(_) => continue,
					};
					if !metadata.is_file() {
						continue;
					}
					if metadata.len() > 1_024 * 1_024 {
						continue;
					}

					let content = match tokio::fs::read_to_string(&path).await {
						Ok(c) => c,
						Err(_) => continue,
					};

					let rel_path = path
						.strip_prefix(&search_path)
						.unwrap_or(&path)
						.to_string_lossy()
						.replace('\\', "/");

					for (i, line) in content.split('\n').enumerate() {
						if results.len() >= limit {
							break;
						}
						if regex.is_match(line) {
							results.push(format!("{}:{}: {}", rel_path, i + 1, line));
						}
					}
				}

				if results.is_empty() {
					return Ok("No matches found.".to_string());
				}

				let mut output = results.join("\n");
				if results.len() >= limit {
					output.push_str(&format!("\n\n(Results limited to {} entries)", limit));
				}
				Ok(output)
			})
		});

		registry.register_mut(definition, handler);
	}

	// -------------------------------------------------------------------
	// 6. fs_list — list directory with configurable depth
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param(
				"string",
				"Directory path (relative to working directory, default: \".\")",
				false,
			),
		);
		parameters.insert(
			"depth".to_string(),
			param(
				"number",
				"Maximum directory depth to recurse (default: 1)",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "fs_list".to_string(),
			description:
				"List files and directories in a path with configurable depth.".to_string(),
			parameters,
			category: ToolCategory::Read,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let wd = Arc::clone(&wd);
		let sandbox = sandbox.clone();
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			let sandbox = sandbox.clone();
			Box::pin(async move {
				let dir_path = match args.get("path").and_then(|v| v.as_str()) {
					Some(p) => sandbox(p)?,
					None => (*wd).clone(),
				};

				let max_depth = args
					.get("depth")
					.and_then(|v| v.as_u64())
					.unwrap_or(1) as usize;

				let entries =
					list_dir_recursive(dir_path.clone(), dir_path, max_depth, 0).await?;

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

		registry.register_mut(definition, handler);
	}

	// -------------------------------------------------------------------
	// 7. fs_stat — file metadata
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param(
				"string",
				"File or directory path (relative to working directory)",
				true,
			),
		);

		let definition = ToolDefinition {
			name: "fs_stat".to_string(),
			description: "Get file or directory metadata: size, modification time, type (file/directory/symlink), and permissions.".to_string(),
			parameters,
			category: ToolCategory::Read,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let sandbox = sandbox.clone();
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let sandbox = sandbox.clone();
			Box::pin(async move {
				let path_str = args
					.get("path")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let file_path = sandbox(path_str)?;

				// Use symlink_metadata to detect symlinks (metadata follows them)
				let metadata = tokio::fs::symlink_metadata(&file_path).await.map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Failed to stat {}: {}", path_str, e),
					)
				})?;

				let file_type = if metadata.is_symlink() {
					"symlink"
				} else if metadata.is_dir() {
					"directory"
				} else {
					"file"
				};

				let modified = metadata
					.modified()
					.ok()
					.map(|t| {
						let datetime: chrono::DateTime<chrono::Utc> = t.into();
						datetime.to_rfc3339()
					})
					.unwrap_or_else(|| "unknown".to_string());

				// Permissions: Unix-style octal
				#[cfg(unix)]
				let permissions = {
					use std::os::unix::fs::PermissionsExt;
					format!("0{:o}", metadata.permissions().mode() & 0o777)
				};
				#[cfg(not(unix))]
				let permissions = if metadata.permissions().readonly() {
					"readonly".to_string()
				} else {
					"read-write".to_string()
				};

				let result = serde_json::json!({
					"size": metadata.len(),
					"modified": modified,
					"type": file_type,
					"permissions": permissions,
				});

				serde_json::to_string_pretty(&result).map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Failed to serialize stat: {}", e),
					)
				})
			})
		});

		registry.register_mut(definition, handler);
	}

	// -------------------------------------------------------------------
	// 8. fs_delete — remove a file or directory
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"path".to_string(),
			param(
				"string",
				"File or directory path (relative to working directory)",
				true,
			),
		);
		parameters.insert(
			"recursive".to_string(),
			param(
				"boolean",
				"If true, recursively remove directory contents (default: false)",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "fs_delete".to_string(),
			description:
				"Remove a file or empty directory. Set recursive=true to remove non-empty directories."
					.to_string(),
			parameters,
			category: ToolCategory::Edit,
			annotations: Some(ToolAnnotations {
				destructive: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let sandbox = sandbox.clone();
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let sandbox = sandbox.clone();
			Box::pin(async move {
				let path_str = args
					.get("path")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let recursive = args
					.get("recursive")
					.and_then(|v| v.as_bool())
					.unwrap_or(false);
				let file_path = sandbox(path_str)?;

				let metadata = tokio::fs::metadata(&file_path).await.map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Failed to stat {}: {}", path_str, e),
					)
				})?;

				if metadata.is_dir() && !recursive {
					let mut read_dir = tokio::fs::read_dir(&file_path).await.map_err(|e| {
						SimseError::tool(
							ToolErrorCode::ExecutionFailed,
							format!("Failed to read directory {}: {}", path_str, e),
						)
					})?;
					if read_dir.next_entry().await?.is_some() {
						return Err(SimseError::tool(
							ToolErrorCode::ExecutionFailed,
							format!(
								"Directory \"{}\" is not empty. Use recursive=true to remove non-empty directories.",
								path_str
							),
						));
					}
				}

				if metadata.is_dir() {
					tokio::fs::remove_dir_all(&file_path).await
				} else {
					tokio::fs::remove_file(&file_path).await
				}
				.map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Failed to delete {}: {}", path_str, e),
					)
				})?;

				let suffix = if recursive { " (recursive)" } else { "" };
				Ok(format!("Deleted {}{}", path_str, suffix))
			})
		});

		registry.register_mut(definition, handler);
	}

	// -------------------------------------------------------------------
	// 9. fs_move — rename/move a file or directory
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"source".to_string(),
			param("string", "Source path (relative to working directory)", true),
		);
		parameters.insert(
			"destination".to_string(),
			param(
				"string",
				"Destination path (relative to working directory)",
				true,
			),
		);

		let definition = ToolDefinition {
			name: "fs_move".to_string(),
			description: "Move or rename a file or directory. Both source and destination are sandboxed to the working directory.".to_string(),
			parameters,
			category: ToolCategory::Edit,
			annotations: Some(ToolAnnotations {
				destructive: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let handler: ToolHandler = Arc::new(move |args: Value| {
			let sandbox = sandbox.clone();
			Box::pin(async move {
				let src_str = args
					.get("source")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let dst_str = args
					.get("destination")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let src_path = sandbox(src_str)?;
				let dst_path = sandbox(dst_str)?;

				// Auto-create parent directories for destination
				if let Some(parent) = dst_path.parent() {
					tokio::fs::create_dir_all(parent).await.map_err(|e| {
						SimseError::tool(
							ToolErrorCode::ExecutionFailed,
							format!("Failed to create directories: {}", e),
						)
					})?;
				}

				tokio::fs::rename(&src_path, &dst_path).await.map_err(|e| {
					SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!("Failed to move {} to {}: {}", src_str, dst_str, e),
					)
				})?;

				Ok(format!("Moved {} \u{2192} {}", src_str, dst_str))
			})
		});

		registry.register_mut(definition, handler);
	}
}
