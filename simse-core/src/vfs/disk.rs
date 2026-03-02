//! VfsDisk — commit VFS snapshots to disk and load disk directories into VFS.
//!
//! Ports `src/ai/vfs/vfs-disk.ts` to Rust. Provides:
//! - `commit()` — snapshot the VFS and write files to a target directory
//! - `load()` — scan a disk directory and write files into the VFS
//! - Binary extension detection for 60+ known binary file extensions

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use crate::error::SimseError;

use super::validators::{ValidationResult, VfsValidator, validate_snapshot};
use super::vfs::{VirtualFs, WriteOptions};

// ---------------------------------------------------------------------------
// Base64 encode/decode (inline, avoids adding a crate dependency)
// ---------------------------------------------------------------------------

const B64_CHARS: &[u8; 64] =
	b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64_encode(data: &[u8]) -> String {
	let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
	for chunk in data.chunks(3) {
		let b0 = chunk[0] as u32;
		let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
		let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
		let triple = (b0 << 16) | (b1 << 8) | b2;
		out.push(B64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
		out.push(B64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
		if chunk.len() > 1 {
			out.push(B64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
		} else {
			out.push('=');
		}
		if chunk.len() > 2 {
			out.push(B64_CHARS[(triple & 0x3F) as usize] as char);
		} else {
			out.push('=');
		}
	}
	out
}

fn b64_decode(input: &str) -> Result<Vec<u8>, String> {
	let input = input.trim_end_matches('=');
	let mut out = Vec::with_capacity(input.len() * 3 / 4);
	let mut buf: u32 = 0;
	let mut bits: u32 = 0;
	for c in input.bytes() {
		let val = match c {
			b'A'..=b'Z' => c - b'A',
			b'a'..=b'z' => c - b'a' + 26,
			b'0'..=b'9' => c - b'0' + 52,
			b'+' => 62,
			b'/' => 63,
			b'\n' | b'\r' | b' ' | b'\t' => continue,
			_ => return Err(format!("Invalid base64 character: {}", c as char)),
		};
		buf = (buf << 6) | val as u32;
		bits += 6;
		if bits >= 8 {
			bits -= 8;
			out.push((buf >> bits) as u8);
			buf &= (1 << bits) - 1;
		}
	}
	Ok(out)
}

// ---------------------------------------------------------------------------
// Binary extension detection
// ---------------------------------------------------------------------------

/// Known binary file extensions — files with these extensions are treated as
/// binary and stored as base64 in the VFS.
static BINARY_EXTENSIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
	[
		// Images
		"png", "jpg", "jpeg", "gif", "bmp", "ico", "tiff", "tif", "webp", "avif", "heic",
		"heif", "psd", "raw", "cr2", "nef",
		// Audio
		"mp3", "wav", "ogg", "flac", "aac", "wma", "m4a", "opus",
		// Video
		"mp4", "avi", "mkv", "mov", "wmv", "flv", "webm", "m4v", "3gp",
		// Archives
		"zip", "tar", "gz", "bz2", "xz", "7z", "rar", "zst", "lz4",
		// Executables & libraries
		"exe", "dll", "so", "dylib", "bin", "msi", "deb", "rpm", "apk", "dmg", "app",
		// Fonts
		"woff", "woff2", "ttf", "eot", "otf",
		// Documents (binary)
		"pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp",
		// Databases
		"db", "sqlite", "sqlite3",
		// WebAssembly
		"wasm",
		// Other
		"class", "pyc", "pyo", "o", "obj", "a", "lib",
	]
	.into_iter()
	.collect()
});

/// Check if a file extension indicates a binary file.
pub fn is_binary_extension(ext: &str) -> bool {
	BINARY_EXTENSIONS.contains(ext.to_lowercase().as_str())
}

/// Extract file extension from a path string (without the dot).
fn get_extension(path: &str) -> Option<String> {
	Path::new(path)
		.extension()
		.and_then(|e| e.to_str())
		.map(|e| e.to_lowercase())
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Path filter predicate type alias.
type PathFilter = Box<dyn Fn(&str) -> bool + Send>;

/// Options for committing VFS content to disk.
#[derive(Default)]
pub struct CommitOptions {
	/// Overwrite existing files on disk (default: false).
	pub overwrite: bool,
	/// Dry run: compute what would be written without actually writing.
	pub dry_run: bool,
	/// Optional filter: only files whose VFS path passes this predicate are committed.
	pub filter: Option<PathFilter>,
	/// Run validators before committing (default: false).
	pub validate: bool,
}

/// Options for loading disk files into the VFS.
#[derive(Default)]
pub struct LoadOptions {
	/// Overwrite existing VFS files (default: false).
	pub overwrite: bool,
	/// Optional filter: only files whose relative path passes this predicate are loaded.
	pub filter: Option<PathFilter>,
	/// Maximum file size in bytes (skip files larger than this).
	pub max_file_size: Option<u64>,
}

/// Result of a commit or load operation.
#[derive(Debug, Clone)]
pub struct CommitResult {
	/// Number of files written.
	pub files_written: usize,
	/// Number of directories created.
	pub directories_created: usize,
	/// Total bytes written.
	pub bytes_written: u64,
	/// Detailed list of operations performed.
	pub operations: Vec<CommitOperation>,
	/// Validation results (only present if `validate` was true on commit).
	pub validation: Option<ValidationResult>,
}

/// A single operation in a commit or load.
#[derive(Debug, Clone)]
pub struct CommitOperation {
	/// Type of operation performed.
	pub op_type: CommitOpType,
	/// VFS path of the file/directory.
	pub path: String,
	/// Corresponding disk path.
	pub disk_path: PathBuf,
	/// File size in bytes (for files).
	pub size: Option<u64>,
	/// Reason for skipping (if op_type is Skip).
	pub reason: Option<String>,
}

/// Type of commit/load operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitOpType {
	Write,
	Mkdir,
	Skip,
}

// ---------------------------------------------------------------------------
// VfsDisk
// ---------------------------------------------------------------------------

/// Manages committing VFS content to disk and loading disk content into VFS.
pub struct VfsDisk {
	vfs: Arc<VirtualFs>,
	base_dir: PathBuf,
}

impl VfsDisk {
	/// Create a new VfsDisk targeting the given base directory.
	pub fn new(vfs: Arc<VirtualFs>, base_dir: PathBuf) -> Self {
		Self { vfs, base_dir }
	}

	/// Get a reference to the base directory.
	pub fn base_dir(&self) -> &Path {
		&self.base_dir
	}

	/// Commit (write) VFS content to a target directory on disk.
	///
	/// Takes a snapshot, then writes each file/directory to disk. Binary files
	/// are decoded from base64 before writing. Returns a `CommitResult` with
	/// statistics and operation details.
	pub fn commit(
		&self,
		target_dir: &Path,
		options: CommitOptions,
		validators: Option<&[Box<dyn VfsValidator>]>,
	) -> Result<CommitResult, SimseError> {
		let snap = self.vfs.snapshot();

		// Optional validation
		let validation = if options.validate {
			validators.map(|vals| validate_snapshot(&snap, vals))
		} else {
			None
		};

		let mut result = CommitResult {
			files_written: 0,
			directories_created: 0,
			bytes_written: 0,
			operations: Vec::new(),
			validation,
		};

		// Create directories first (sorted by path depth)
		let mut dirs = snap.directories.clone();
		dirs.sort_by_key(|d| d.path.matches('/').count());

		for dir in &dirs {
			let relative = vfs_path_to_relative(&dir.path);
			if let Some(ref filter) = options.filter {
				if !filter(&dir.path) {
					continue;
				}
			}

			let disk_path = target_dir.join(&relative);

			if !options.dry_run {
				std::fs::create_dir_all(&disk_path)?;
			}

			result.directories_created += 1;
			result.operations.push(CommitOperation {
				op_type: CommitOpType::Mkdir,
				path: dir.path.clone(),
				disk_path,
				size: None,
				reason: None,
			});
		}

		// Write files
		for file in &snap.files {
			if let Some(ref filter) = options.filter {
				if !filter(&file.path) {
					result.operations.push(CommitOperation {
						op_type: CommitOpType::Skip,
						path: file.path.clone(),
						disk_path: PathBuf::new(),
						size: None,
						reason: Some("filtered out".to_string()),
					});
					continue;
				}
			}

			let relative = vfs_path_to_relative(&file.path);
			let disk_path = target_dir.join(&relative);

			// Check overwrite
			if disk_path.exists() && !options.overwrite {
				result.operations.push(CommitOperation {
					op_type: CommitOpType::Skip,
					path: file.path.clone(),
					disk_path,
					size: None,
					reason: Some("already exists and overwrite is false".to_string()),
				});
				continue;
			}

			if !options.dry_run {
				// Ensure parent directory exists
				if let Some(parent) = disk_path.parent() {
					std::fs::create_dir_all(parent)?;
				}

				if file.content_type == "binary" {
					if let Some(ref b64) = file.base64 {
						let data = b64_decode(b64).map_err(|e| {
							SimseError::other(format!(
								"Failed to decode base64 for {}: {}",
								file.path, e
							))
						})?;
						let size = data.len() as u64;
						std::fs::write(&disk_path, &data)?;
						result.bytes_written += size;
						result.files_written += 1;
						result.operations.push(CommitOperation {
							op_type: CommitOpType::Write,
							path: file.path.clone(),
							disk_path,
							size: Some(size),
							reason: None,
						});
					}
				} else {
					let text = file.text.as_deref().unwrap_or("");
					let size = text.len() as u64;
					std::fs::write(&disk_path, text)?;
					result.bytes_written += size;
					result.files_written += 1;
					result.operations.push(CommitOperation {
						op_type: CommitOpType::Write,
						path: file.path.clone(),
						disk_path,
						size: Some(size),
						reason: None,
					});
				}
			} else {
				// Dry run: estimate size
				let size = if file.content_type == "binary" {
					file.base64
						.as_ref()
						.map(|b| (b.len() as f64 * 0.75) as u64)
						.unwrap_or(0)
				} else {
					file.text.as_deref().unwrap_or("").len() as u64
				};

				result.bytes_written += size;
				result.files_written += 1;
				result.operations.push(CommitOperation {
					op_type: CommitOpType::Write,
					path: file.path.clone(),
					disk_path,
					size: Some(size),
					reason: None,
				});
			}
		}

		Ok(result)
	}

	/// Load files from a source directory on disk into the VFS.
	///
	/// Recursively walks the source directory, reads files, and writes them to
	/// the VFS with `vfs:///` paths. Binary files (detected by extension) are
	/// base64-encoded before storage.
	pub fn load(
		&self,
		source_dir: &Path,
		options: LoadOptions,
	) -> Result<CommitResult, SimseError> {
		let mut result = CommitResult {
			files_written: 0,
			directories_created: 0,
			bytes_written: 0,
			operations: Vec::new(),
			validation: None,
		};

		self.load_recursive(source_dir, source_dir, &options, &mut result)?;

		Ok(result)
	}

	fn load_recursive(
		&self,
		root: &Path,
		current: &Path,
		options: &LoadOptions,
		result: &mut CommitResult,
	) -> Result<(), SimseError> {
		let entries = std::fs::read_dir(current)?;

		for entry in entries {
			let entry = entry?;
			let path = entry.path();
			let metadata = entry.metadata()?;

			// Build relative path and VFS path
			let relative = path
				.strip_prefix(root)
				.unwrap_or(&path)
				.to_string_lossy()
				.replace('\\', "/");

			let vfs_path = format!("vfs:///{}", relative);

			if metadata.is_dir() {
				if let Some(ref filter) = options.filter {
					if !filter(&vfs_path) {
						continue;
					}
				}

				// Create directory in VFS
				self.vfs.mkdir(&vfs_path, true)?;
				result.directories_created += 1;
				result.operations.push(CommitOperation {
					op_type: CommitOpType::Mkdir,
					path: vfs_path,
					disk_path: path.clone(),
					size: None,
					reason: None,
				});

				// Recurse into subdirectory
				self.load_recursive(root, &path, options, result)?;
			} else if metadata.is_file() {
				if let Some(ref filter) = options.filter {
					if !filter(&vfs_path) {
						result.operations.push(CommitOperation {
							op_type: CommitOpType::Skip,
							path: vfs_path,
							disk_path: path,
							size: None,
							reason: Some("filtered out".to_string()),
						});
						continue;
					}
				}

				// Check max file size
				let file_size = metadata.len();
				if let Some(max_size) = options.max_file_size {
					if file_size > max_size {
						result.operations.push(CommitOperation {
							op_type: CommitOpType::Skip,
							path: vfs_path,
							disk_path: path,
							size: Some(file_size),
							reason: Some(format!(
								"exceeds max file size ({})",
								max_size
							)),
						});
						continue;
					}
				}

				// Check if file already exists and overwrite is off
				if !options.overwrite {
					let exists = self.vfs.exists(&vfs_path).unwrap_or(false);
					if exists {
						result.operations.push(CommitOperation {
							op_type: CommitOpType::Skip,
							path: vfs_path,
							disk_path: path,
							size: Some(file_size),
							reason: Some(
								"already exists and overwrite is false".to_string(),
							),
						});
						continue;
					}
				}

				let ext = get_extension(&relative);
				let is_binary = ext.as_deref().is_some_and(is_binary_extension);

				let write_opts = super::vfs::WriteOptions {
					content_type: if is_binary {
						Some("binary".to_string())
					} else {
						None
					},
					create_parents: true,
				};

				if is_binary {
					let data = std::fs::read(&path)?;
					let b64 = b64_encode(&data);
					self.vfs
						.write_file(&vfs_path, &b64, Some(write_opts))?;
				} else {
					// Try text first; fall back to binary if not valid UTF-8
					match std::fs::read_to_string(&path) {
						Ok(text) => {
							self.vfs
								.write_file(&vfs_path, &text, Some(write_opts))?;
						}
						Err(_) => {
							let write_opts_bin = WriteOptions {
								content_type: Some("binary".to_string()),
								create_parents: true,
							};
							let data = std::fs::read(&path)?;
							let b64 = b64_encode(&data);
							self.vfs
								.write_file(&vfs_path, &b64, Some(write_opts_bin))?;
						}
					}
				}

				result.bytes_written += file_size;
				result.files_written += 1;
				result.operations.push(CommitOperation {
					op_type: CommitOpType::Write,
					path: vfs_path,
					disk_path: path,
					size: Some(file_size),
					reason: None,
				});
			}
		}

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a VFS path (`vfs:///foo/bar.txt`) to a relative path (`foo/bar.txt`).
fn vfs_path_to_relative(vfs_path: &str) -> String {
	let stripped = vfs_path
		.strip_prefix("vfs:///")
		.unwrap_or(vfs_path.strip_prefix("vfs://").unwrap_or(vfs_path));
	stripped.to_string()
}
