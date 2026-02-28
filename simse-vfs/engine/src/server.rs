use std::io::{self, BufRead};

use crate::error::VfsError;
use crate::path::VfsLimits;
use crate::protocol::*;
use crate::transport::NdjsonTransport;
use crate::vfs::{
	DiffResultOutput, SearchOpts, SearchOutput, TransactionOp as VfsTransactionOp, VfsEvent,
	VirtualFs,
};

/// VFS JSON-RPC server — dispatches incoming requests to VFS operations.
pub struct VfsServer {
	transport: NdjsonTransport,
	vfs: Option<VirtualFs>,
}

impl VfsServer {
	/// Create a new VFS server with the given transport.
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			vfs: None,
		}
	}

	/// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
	pub fn run(&mut self) -> Result<(), VfsError> {
		let stdin = io::stdin();
		let reader = stdin.lock();

		for line_result in reader.lines() {
			let line = line_result?;
			if line.trim().is_empty() {
				continue;
			}

			let request: JsonRpcRequest = match serde_json::from_str(&line) {
				Ok(r) => r,
				Err(e) => {
					tracing::error!("Failed to parse request: {}", e);
					continue;
				}
			};

			self.dispatch(request);
		}

		Ok(())
	}

	// ── Dispatch ──────────────────────────────────────────────────────────

	fn dispatch(&mut self, req: JsonRpcRequest) {
		let result = match req.method.as_str() {
			"initialize" => self.handle_initialize(req.params),
			"vfs/readFile" => self.with_vfs(|vfs| handle_read_file(vfs, req.params)),
			"vfs/writeFile" => self.with_vfs_mut(|vfs| handle_write_file(vfs, req.params)),
			"vfs/appendFile" => self.with_vfs_mut(|vfs| handle_append_file(vfs, req.params)),
			"vfs/deleteFile" => self.with_vfs_mut(|vfs| handle_delete_file(vfs, req.params)),
			"vfs/mkdir" => self.with_vfs_mut(|vfs| handle_mkdir(vfs, req.params)),
			"vfs/readdir" => self.with_vfs(|vfs| handle_readdir(vfs, req.params)),
			"vfs/rmdir" => self.with_vfs_mut(|vfs| handle_rmdir(vfs, req.params)),
			"vfs/stat" => self.with_vfs(|vfs| handle_stat(vfs, req.params)),
			"vfs/exists" => self.with_vfs(|vfs| handle_exists(vfs, req.params)),
			"vfs/rename" => self.with_vfs_mut(|vfs| handle_rename(vfs, req.params)),
			"vfs/copy" => self.with_vfs_mut(|vfs| handle_copy(vfs, req.params)),
			"vfs/glob" => self.with_vfs(|vfs| handle_glob(vfs, req.params)),
			"vfs/tree" => self.with_vfs(|vfs| handle_tree(vfs, req.params)),
			"vfs/du" => self.with_vfs(|vfs| handle_du(vfs, req.params)),
			"vfs/search" => self.with_vfs(|vfs| handle_search(vfs, req.params)),
			"vfs/history" => self.with_vfs(|vfs| handle_history(vfs, req.params)),
			"vfs/diff" => self.with_vfs(|vfs| handle_diff(vfs, req.params)),
			"vfs/diffVersions" => self.with_vfs(|vfs| handle_diff_versions(vfs, req.params)),
			"vfs/checkout" => self.with_vfs_mut(|vfs| handle_checkout(vfs, req.params)),
			"vfs/snapshot" => self.with_vfs(|vfs| handle_snapshot(vfs)),
			"vfs/restore" => self.with_vfs_mut(|vfs| handle_restore(vfs, req.params)),
			"vfs/clear" => self.with_vfs_mut(|vfs| handle_clear(vfs)),
			"vfs/transaction" => self.with_vfs_mut(|vfs| handle_transaction(vfs, req.params)),
			"vfs/metrics" => self.with_vfs(|vfs| handle_metrics(vfs)),
			_ => {
				self.transport.write_error(
					req.id,
					METHOD_NOT_FOUND,
					format!("Unknown method: {}", req.method),
					None,
				);
				return;
			}
		};

		// Drain events after every dispatch
		if let Some(ref mut vfs) = self.vfs {
			for event in vfs.drain_events() {
				let params = match &event {
					VfsEvent::Write { path, size, content_type, is_new } => {
						serde_json::json!({"type": "write", "path": path, "size": size, "contentType": content_type, "isNew": is_new})
					}
					VfsEvent::Delete { path } => {
						serde_json::json!({"type": "delete", "path": path})
					}
					VfsEvent::Rename { old_path, new_path } => {
						serde_json::json!({"type": "rename", "oldPath": old_path, "newPath": new_path})
					}
					VfsEvent::Mkdir { path } => {
						serde_json::json!({"type": "mkdir", "path": path})
					}
				};
				self.transport.write_notification("vfs/event", params);
			}
		}

		match result {
			Ok(value) => self.transport.write_response(req.id, value),
			Err(e) => self.transport.write_error(
				req.id,
				VFS_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			),
		}
	}

	// ── VFS accessors ────────────────────────────────────────────────────

	fn with_vfs<F>(&self, f: F) -> Result<serde_json::Value, VfsError>
	where
		F: FnOnce(&VirtualFs) -> Result<serde_json::Value, VfsError>,
	{
		match &self.vfs {
			Some(vfs) => f(vfs),
			None => Err(VfsError::InvalidOperation("Not initialized".to_string())),
		}
	}

	fn with_vfs_mut<F>(&mut self, f: F) -> Result<serde_json::Value, VfsError>
	where
		F: FnOnce(&mut VirtualFs) -> Result<serde_json::Value, VfsError>,
	{
		match &mut self.vfs {
			Some(vfs) => f(vfs),
			None => Err(VfsError::InvalidOperation("Not initialized".to_string())),
		}
	}

	// ── Initialize ───────────────────────────────────────────────────────

	fn handle_initialize(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, VfsError> {
		let init_params: InitializeParams = serde_json::from_value(params)?;

		let mut limits = VfsLimits::default();
		if let Some(lp) = init_params.limits {
			if let Some(v) = lp.max_file_size {
				limits.max_file_size = v;
			}
			if let Some(v) = lp.max_total_size {
				limits.max_total_size = v;
			}
			if let Some(v) = lp.max_path_depth {
				limits.max_path_depth = v;
			}
			if let Some(v) = lp.max_name_length {
				limits.max_name_length = v;
			}
			if let Some(v) = lp.max_node_count {
				limits.max_node_count = v;
			}
			if let Some(v) = lp.max_path_length {
				limits.max_path_length = v;
			}
			if let Some(v) = lp.max_diff_lines {
				limits.max_diff_lines = v;
			}
		}

		let max_history = init_params
			.history
			.and_then(|h| h.max_entries_per_file)
			.unwrap_or(50);

		self.vfs = Some(VirtualFs::new(limits, max_history));

		Ok(serde_json::json!({ "ok": true }))
	}
}

// ── Free-standing handler functions ─────────────────────────────────────────
//
// Using free functions instead of methods avoids borrow-checker issues:
// `with_vfs` / `with_vfs_mut` already borrow `self.vfs`, so the handler
// cannot also borrow `self`.

fn parse_params<T: serde::de::DeserializeOwned>(params: serde_json::Value) -> Result<T, VfsError> {
	serde_json::from_value(params).map_err(VfsError::from)
}

fn handle_read_file(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: ReadFileParams = parse_params(params)?;
	let r = vfs.read_file(&p.path)?;
	Ok(serde_json::to_value(ReadFileResult {
		content_type: r.content_type,
		text: r.text,
		data: r.data_base64,
		size: r.size,
	})?)
}

fn handle_write_file(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: WriteFileParams = parse_params(params)?;
	vfs.write_file(
		&p.path,
		&p.content,
		p.content_type.as_deref(),
		p.create_parents.unwrap_or(false),
	)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_append_file(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: AppendFileParams = parse_params(params)?;
	vfs.append_file(&p.path, &p.content)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_delete_file(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: PathParams = parse_params(params)?;
	let deleted = vfs.delete_file(&p.path)?;
	Ok(serde_json::json!({ "deleted": deleted }))
}

fn handle_mkdir(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: MkdirParams = parse_params(params)?;
	vfs.mkdir(&p.path, p.recursive.unwrap_or(false))?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_readdir(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: ReaddirParams = parse_params(params)?;
	let entries = vfs.readdir(&p.path, p.recursive.unwrap_or(false))?;
	let result: Vec<DirEntry> = entries
		.into_iter()
		.map(|e| DirEntry {
			name: e.name,
			node_type: e.node_type,
		})
		.collect();
	Ok(serde_json::json!({ "entries": result }))
}

fn handle_rmdir(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: RmdirParams = parse_params(params)?;
	let deleted = vfs.rmdir(&p.path, p.recursive.unwrap_or(false))?;
	Ok(serde_json::json!({ "deleted": deleted }))
}

fn handle_stat(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: PathParams = parse_params(params)?;
	let s = vfs.stat(&p.path)?;
	Ok(serde_json::to_value(StatResult {
		path: s.path,
		node_type: s.node_type,
		size: s.size,
		created_at: s.created_at,
		modified_at: s.modified_at,
	})?)
}

fn handle_exists(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: PathParams = parse_params(params)?;
	let exists = vfs.exists(&p.path)?;
	Ok(serde_json::json!({ "exists": exists }))
}

fn handle_rename(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: RenameParams = parse_params(params)?;
	vfs.rename(&p.old_path, &p.new_path)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_copy(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: CopyParams = parse_params(params)?;
	vfs.copy(
		&p.src,
		&p.dest,
		p.overwrite.unwrap_or(false),
		p.recursive.unwrap_or(false),
	)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_glob(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: GlobParams = parse_params(params)?;
	let patterns: Vec<String> = match p.pattern {
		serde_json::Value::String(s) => vec![s],
		serde_json::Value::Array(arr) => arr
			.into_iter()
			.filter_map(|v| v.as_str().map(String::from))
			.collect(),
		_ => {
			return Err(VfsError::InvalidOperation(
				"pattern must be string or array".into(),
			))
		}
	};
	let results = vfs.glob(patterns);
	Ok(serde_json::json!({ "matches": results }))
}

fn handle_tree(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: OptionalPathParams = parse_params(params)?;
	let tree = vfs.tree(p.path.as_deref())?;
	Ok(serde_json::json!({ "tree": tree }))
}

fn handle_du(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: PathParams = parse_params(params)?;
	let size = vfs.du(&p.path)?;
	Ok(serde_json::json!({ "size": size }))
}

fn handle_search(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: SearchParams = parse_params(params)?;
	let opts = SearchOpts {
		glob: p.glob,
		max_results: p.max_results.unwrap_or(100),
		mode: p.mode.unwrap_or_else(|| "substring".to_string()),
		context_before: p.context_before.unwrap_or(0),
		context_after: p.context_after.unwrap_or(0),
		count_only: p.count_only.unwrap_or(false),
	};
	let output = vfs.search(&p.query, opts)?;
	match output {
		SearchOutput::Results(matches) => {
			let results: Vec<SearchResult> = matches
				.into_iter()
				.map(|m| SearchResult {
					path: m.path,
					line: m.line,
					column: m.column,
					match_text: m.match_text,
					context_before: m.context_before,
					context_after: m.context_after,
				})
				.collect();
			Ok(serde_json::json!({ "results": results }))
		}
		SearchOutput::Count(count) => Ok(serde_json::json!({ "count": count })),
	}
}

fn handle_history(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: PathParams = parse_params(params)?;
	let entries = vfs.history(&p.path)?;
	let result: Vec<HistoryEntry> = entries
		.into_iter()
		.map(|e| HistoryEntry {
			version: e.version,
			content_type: e.content_type,
			text: e.text,
			base64: e.base64,
			size: e.size,
			timestamp: e.timestamp,
		})
		.collect();
	Ok(serde_json::json!({ "entries": result }))
}

fn convert_diff_output(d: DiffResultOutput) -> DiffResult {
	DiffResult {
		old_path: d.old_path,
		new_path: d.new_path,
		additions: d.additions,
		deletions: d.deletions,
		hunks: d
			.hunks
			.into_iter()
			.map(|h| DiffHunk {
				old_start: h.old_start,
				old_count: h.old_count,
				new_start: h.new_start,
				new_count: h.new_count,
				lines: h
					.lines
					.into_iter()
					.map(|l| DiffLineResult {
						line_type: l.line_type.as_str().to_string(),
						text: l.text,
						old_line: l.old_line,
						new_line: l.new_line,
					})
					.collect(),
			})
			.collect(),
	}
}

fn handle_diff(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: DiffParams = parse_params(params)?;
	let d = vfs.diff(&p.old_path, &p.new_path, p.context.unwrap_or(3))?;
	Ok(serde_json::to_value(convert_diff_output(d))?)
}

fn handle_diff_versions(
	vfs: &VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: DiffVersionsParams = parse_params(params)?;
	let d = vfs.diff_versions(&p.path, p.old_version, p.new_version, p.context.unwrap_or(3))?;
	Ok(serde_json::to_value(convert_diff_output(d))?)
}

fn handle_checkout(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: CheckoutParams = parse_params(params)?;
	vfs.checkout(&p.path, p.version)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_snapshot(vfs: &VirtualFs) -> Result<serde_json::Value, VfsError> {
	let snap = vfs.snapshot();
	let result = SnapshotData {
		files: snap
			.files
			.into_iter()
			.map(|f| SnapshotFile {
				path: f.path,
				content_type: f.content_type,
				text: f.text,
				base64: f.base64,
				created_at: f.created_at,
				modified_at: f.modified_at,
			})
			.collect(),
		directories: snap
			.directories
			.into_iter()
			.map(|d| SnapshotDir {
				path: d.path,
				created_at: d.created_at,
				modified_at: d.modified_at,
			})
			.collect(),
	};
	Ok(serde_json::to_value(result)?)
}

fn handle_restore(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let wrapper: RestoreParams = parse_params(params)?;
	let snap = wrapper.snapshot;
	// Convert protocol SnapshotData to vfs SnapshotData
	let vfs_snap = crate::vfs::SnapshotData {
		files: snap
			.files
			.into_iter()
			.map(|f| crate::vfs::SnapshotFile {
				path: f.path,
				content_type: f.content_type,
				text: f.text,
				base64: f.base64,
				created_at: f.created_at,
				modified_at: f.modified_at,
			})
			.collect(),
		directories: snap
			.directories
			.into_iter()
			.map(|d| crate::vfs::SnapshotDir {
				path: d.path,
				created_at: d.created_at,
				modified_at: d.modified_at,
			})
			.collect(),
	};
	vfs.restore(vfs_snap)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_clear(vfs: &mut VirtualFs) -> Result<serde_json::Value, VfsError> {
	vfs.clear();
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_transaction(
	vfs: &mut VirtualFs,
	params: serde_json::Value,
) -> Result<serde_json::Value, VfsError> {
	let p: TransactionParams = parse_params(params)?;
	let ops: Vec<VfsTransactionOp> = p
		.ops
		.into_iter()
		.map(|op| match op {
			TransactionOp::WriteFile { path, content } => {
				VfsTransactionOp::WriteFile { path, content }
			}
			TransactionOp::DeleteFile { path } => VfsTransactionOp::DeleteFile { path },
			TransactionOp::Mkdir { path } => VfsTransactionOp::Mkdir { path },
			TransactionOp::Rmdir { path } => VfsTransactionOp::Rmdir { path },
			TransactionOp::Rename { old_path, new_path } => {
				VfsTransactionOp::Rename { old_path, new_path }
			}
			TransactionOp::Copy { src, dest } => VfsTransactionOp::Copy { src, dest },
		})
		.collect();
	vfs.transaction(ops)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_metrics(vfs: &VirtualFs) -> Result<serde_json::Value, VfsError> {
	let m = vfs.metrics();
	Ok(serde_json::to_value(MetricsResult {
		total_size: m.total_size,
		node_count: m.node_count,
		file_count: m.file_count,
		directory_count: m.directory_count,
	})?)
}
