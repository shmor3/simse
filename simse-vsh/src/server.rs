use std::io::{self, BufRead};

use crate::error::VshError;
use crate::executor::default_shell;
use crate::protocol::*;
use crate::sandbox::SandboxConfig;
use crate::shell::VirtualShell;
use crate::transport::NdjsonTransport;

/// VSH JSON-RPC server -- dispatches incoming requests to shell operations.
pub struct VshServer {
	transport: NdjsonTransport,
	shell: Option<VirtualShell>,
}

impl VshServer {
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			shell: None,
		}
	}

	/// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
	pub async fn run(&mut self) -> Result<(), VshError> {
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

			self.dispatch(request).await;
		}

		Ok(())
	}

	// -- Dispatch -------------------------------------------------------------

	async fn dispatch(&mut self, req: JsonRpcRequest) {
		let result = match req.method.as_str() {
			"initialize" => self.handle_initialize(req.params),

			// Session
			"session/create" => {
				self.with_shell_mut(handle_session_create, req.params)
			}
			"session/get" => self.with_shell(handle_session_get, req.params),
			"session/list" => self.with_shell(handle_session_list, req.params),
			"session/delete" => self.with_shell_mut(handle_session_delete, req.params),

			// Exec (async)
			"exec/run" => self.handle_exec_run(req.params).await,
			"exec/runRaw" => self.handle_exec_run_raw(req.params).await,
			"exec/git" => self.handle_exec_git(req.params).await,
			"exec/script" => self.handle_exec_script(req.params).await,

			// Env
			"env/set" => self.with_shell_mut(handle_env_set, req.params),
			"env/get" => self.with_shell(handle_env_get, req.params),
			"env/list" => self.with_shell(handle_env_list, req.params),
			"env/delete" => self.with_shell_mut(handle_env_delete, req.params),

			// Shell
			"shell/setCwd" => self.with_shell_mut(handle_shell_set_cwd, req.params),
			"shell/getCwd" => self.with_shell(handle_shell_get_cwd, req.params),
			"shell/setAlias" => self.with_shell_mut(handle_shell_set_alias, req.params),
			"shell/getAliases" => self.with_shell(handle_shell_get_aliases, req.params),
			"shell/history" => self.with_shell(handle_shell_history, req.params),
			"shell/metrics" => self.with_shell(handle_shell_metrics, req.params),

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

		match result {
			Ok(value) => self.transport.write_response(req.id, value),
			Err(e) => self.transport.write_error(
				req.id,
				VSH_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			),
		}
	}

	// -- Shell accessors ------------------------------------------------------

	fn with_shell<F>(
		&self,
		f: F,
		params: serde_json::Value,
	) -> Result<serde_json::Value, VshError>
	where
		F: FnOnce(&VirtualShell, serde_json::Value) -> Result<serde_json::Value, VshError>,
	{
		match &self.shell {
			Some(sh) => f(sh, params),
			None => Err(VshError::NotInitialized),
		}
	}

	fn with_shell_mut<F>(
		&mut self,
		f: F,
		params: serde_json::Value,
	) -> Result<serde_json::Value, VshError>
	where
		F: FnOnce(&mut VirtualShell, serde_json::Value) -> Result<serde_json::Value, VshError>,
	{
		match &mut self.shell {
			Some(sh) => f(sh, params),
			None => Err(VshError::NotInitialized),
		}
	}

	// -- Initialize -----------------------------------------------------------

	fn handle_initialize(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, VshError> {
		let p: InitializeParams = parse_params(params)?;

		let sandbox = SandboxConfig {
			root_directory: std::path::PathBuf::from(&p.root_directory),
			allowed_paths: p
				.allowed_paths
				.unwrap_or_default()
				.into_iter()
				.map(std::path::PathBuf::from)
				.collect(),
			blocked_patterns: p.blocked_patterns.unwrap_or_default(),
			max_sessions: p.max_sessions.unwrap_or(32),
			default_timeout_ms: p.default_timeout_ms.unwrap_or(120_000),
			max_output_bytes: p.max_output_bytes.unwrap_or(50_000),
		};

		let shell = p.shell.unwrap_or_else(default_shell);
		self.shell = Some(VirtualShell::new(sandbox, shell));

		Ok(serde_json::json!({ "ok": true }))
	}

	// -- Async exec handlers --------------------------------------------------
	// These need &mut self + .await, so they can't use with_shell_mut.

	async fn handle_exec_run(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, VshError> {
		let p: ExecRunParams = parse_params(params)?;
		let sh = self.shell.as_mut().ok_or(VshError::NotInitialized)?;

		let result = sh
			.exec_in_session(
				&p.session_id,
				&p.command,
				p.timeout_ms,
				p.max_output_bytes,
				p.stdin.as_deref(),
			)
			.await?;

		Ok(serde_json::to_value(ExecResultResponse {
			stdout: result.stdout,
			stderr: result.stderr,
			exit_code: result.exit_code,
			duration_ms: result.duration_ms,
		})?)
	}

	async fn handle_exec_run_raw(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, VshError> {
		let p: ExecRunRawParams = parse_params(params)?;
		let sh = self.shell.as_mut().ok_or(VshError::NotInitialized)?;

		let result = sh
			.exec_raw(
				&p.command,
				p.cwd.as_deref(),
				p.env.as_ref(),
				p.shell.as_deref(),
				p.timeout_ms,
				p.max_output_bytes,
				p.stdin.as_deref(),
			)
			.await?;

		Ok(serde_json::to_value(ExecResultResponse {
			stdout: result.stdout,
			stderr: result.stderr,
			exit_code: result.exit_code,
			duration_ms: result.duration_ms,
		})?)
	}

	async fn handle_exec_git(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, VshError> {
		let p: ExecGitParams = parse_params(params)?;
		let sh = self.shell.as_mut().ok_or(VshError::NotInitialized)?;

		let result = sh
			.exec_git_in_session(&p.session_id, &p.args, p.timeout_ms)
			.await?;

		Ok(serde_json::to_value(ExecResultResponse {
			stdout: result.stdout,
			stderr: result.stderr,
			exit_code: result.exit_code,
			duration_ms: result.duration_ms,
		})?)
	}

	async fn handle_exec_script(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, VshError> {
		let p: ExecScriptParams = parse_params(params)?;
		let sh = self.shell.as_mut().ok_or(VshError::NotInitialized)?;

		// Script execution: pass the entire script as a single -c argument
		let result = sh
			.exec_in_session(
				&p.session_id,
				&p.script,
				p.timeout_ms,
				p.max_output_bytes,
				None,
			)
			.await?;

		Ok(serde_json::to_value(ExecResultResponse {
			stdout: result.stdout,
			stderr: result.stderr,
			exit_code: result.exit_code,
			duration_ms: result.duration_ms,
		})?)
	}
}

// -- Free-standing sync handler functions -------------------------------------

fn parse_params<T: serde::de::DeserializeOwned>(
	params: serde_json::Value,
) -> Result<T, VshError> {
	serde_json::from_value(params).map_err(|e| VshError::InvalidParams(e.to_string()))
}

fn handle_session_create(
	sh: &mut VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: SessionCreateParams = parse_params(params)?;
	let session = sh.create_session(p.name, p.cwd, p.env)?;
	Ok(serde_json::to_value(SessionInfo {
		id: session.id.clone(),
		name: session.name.clone(),
		cwd: session.cwd.display().to_string(),
		env: session.env.clone(),
		aliases: session.aliases.clone(),
		created_at: session.created_at,
		last_active_at: session.last_active_at,
		command_count: session.history.len(),
	})?)
}

fn handle_session_get(
	sh: &VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: SessionIdParams = parse_params(params)?;
	let session = sh.get_session(&p.session_id)?;
	Ok(serde_json::to_value(SessionInfo {
		id: session.id.clone(),
		name: session.name.clone(),
		cwd: session.cwd.display().to_string(),
		env: session.env.clone(),
		aliases: session.aliases.clone(),
		created_at: session.created_at,
		last_active_at: session.last_active_at,
		command_count: session.history.len(),
	})?)
}

fn handle_session_list(sh: &VirtualShell, _params: serde_json::Value) -> Result<serde_json::Value, VshError> {
	let sessions: Vec<SessionListItem> = sh
		.list_sessions()
		.into_iter()
		.map(|s| SessionListItem {
			id: s.id.clone(),
			name: s.name.clone(),
			cwd: s.cwd.display().to_string(),
			command_count: s.history.len(),
			last_active_at: s.last_active_at,
		})
		.collect();
	Ok(serde_json::json!({ "sessions": sessions }))
}

fn handle_session_delete(
	sh: &mut VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: SessionIdParams = parse_params(params)?;
	let deleted = sh.delete_session(&p.session_id)?;
	Ok(serde_json::json!({ "deleted": deleted }))
}

fn handle_env_set(
	sh: &mut VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: EnvSetParams = parse_params(params)?;
	sh.set_env(&p.session_id, &p.key, &p.value)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_env_get(
	sh: &VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: EnvGetParams = parse_params(params)?;
	let value = sh.get_env(&p.session_id, &p.key)?;
	Ok(serde_json::json!({ "value": value }))
}

fn handle_env_list(
	sh: &VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: SessionIdParams = parse_params(params)?;
	let env = sh.list_env(&p.session_id)?;
	Ok(serde_json::json!({ "env": env }))
}

fn handle_env_delete(
	sh: &mut VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: EnvDeleteParams = parse_params(params)?;
	let deleted = sh.delete_env(&p.session_id, &p.key)?;
	Ok(serde_json::json!({ "deleted": deleted }))
}

fn handle_shell_set_cwd(
	sh: &mut VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: ShellSetCwdParams = parse_params(params)?;
	sh.set_cwd(&p.session_id, &p.cwd)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_shell_get_cwd(
	sh: &VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: SessionIdParams = parse_params(params)?;
	let cwd = sh.get_cwd(&p.session_id)?;
	Ok(serde_json::json!({ "cwd": cwd }))
}

fn handle_shell_set_alias(
	sh: &mut VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: ShellSetAliasParams = parse_params(params)?;
	sh.set_alias(&p.session_id, &p.name, &p.command)?;
	Ok(serde_json::json!({ "ok": true }))
}

fn handle_shell_get_aliases(
	sh: &VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: SessionIdParams = parse_params(params)?;
	let aliases = sh.get_aliases(&p.session_id)?;
	Ok(serde_json::json!({ "aliases": aliases }))
}

fn handle_shell_history(
	sh: &VirtualShell,
	params: serde_json::Value,
) -> Result<serde_json::Value, VshError> {
	let p: SessionIdParams = parse_params(params)?;
	let history = sh.get_history(&p.session_id)?;
	let entries: Vec<HistoryEntryResponse> = history
		.iter()
		.map(|h| HistoryEntryResponse {
			command: h.command.clone(),
			exit_code: h.exit_code,
			timestamp: h.timestamp,
			duration_ms: h.duration_ms,
		})
		.collect();
	Ok(serde_json::json!({ "history": entries }))
}

fn handle_shell_metrics(sh: &VirtualShell, _params: serde_json::Value) -> Result<serde_json::Value, VshError> {
	Ok(serde_json::to_value(MetricsResponse {
		session_count: sh.session_count(),
		total_commands: sh.total_commands(),
		total_errors: sh.total_errors(),
	})?)
}
