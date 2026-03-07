use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use im::HashMap as ImHashMap;
use im::Vector as ImVector;

use crate::error::SandboxError;
use crate::vsh_executor::ExecResult;
use crate::vsh_sandbox::SandboxConfig;

fn now_ms() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64
}

/// A single shell session with its own state.
#[derive(Debug, Clone)]
pub struct ShellSession {
	pub id: String,
	pub name: Option<String>,
	pub cwd: PathBuf,
	pub env: ImHashMap<String, String>,
	pub aliases: ImHashMap<String, String>,
	pub history: ImVector<HistoryEntry>,
	pub created_at: u64,
	pub last_active_at: u64,
}

/// A single entry in command history.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
	pub command: String,
	pub exit_code: i32,
	pub timestamp: u64,
	pub duration_ms: u64,
}

// ── Prepare structs (pure data for I/O execution) ────────────────────────

/// Prepared data for executing a command in a session.
///
/// Returned by [`VirtualShell::prepare_exec`] — the caller performs actual
/// I/O with the backend, then feeds the result back via
/// [`VirtualShell::record_exec`].
#[derive(Debug, Clone)]
pub struct ExecRequest {
	pub session_id: String,
	pub command: String,
	pub resolved_command: String,
	pub cwd: PathBuf,
	pub env: ImHashMap<String, String>,
	pub shell_cmd: String,
	pub timeout_ms: u64,
	pub max_output_bytes: usize,
	pub stdin: Option<String>,
}

/// Prepared data for executing a git command in a session.
///
/// Returned by [`VirtualShell::prepare_exec_git`] — the caller performs
/// actual I/O with the backend, then feeds the result back via
/// [`VirtualShell::record_exec`].
#[derive(Debug, Clone)]
pub struct GitExecRequest {
	pub session_id: String,
	pub command_str: String,
	pub args: Vec<String>,
	pub cwd: PathBuf,
	pub env: ImHashMap<String, String>,
	pub timeout_ms: u64,
	pub max_output_bytes: usize,
}

/// Prepared data for executing a raw (session-less) command.
///
/// Returned by [`VirtualShell::prepare_exec_raw`] — the caller performs
/// actual I/O with the backend, then feeds the result back via
/// [`VirtualShell::record_exec_raw`].
#[derive(Debug, Clone)]
pub struct RawExecRequest {
	pub command: String,
	pub cwd: PathBuf,
	pub env: HashMap<String, String>,
	pub shell_cmd: String,
	pub timeout_ms: u64,
	pub max_output_bytes: usize,
	pub stdin: Option<String>,
}

// ── VirtualShell (pure functional core) ──────────────────────────────────

/// The VirtualShell manages all sessions as pure state.
///
/// Methods that modify state take `self` by value and return `Self` (or
/// `(Self, T)`), enabling functional-style state transitions with
/// structural sharing via `im` persistent data structures.
///
/// I/O (command execution) is **not** performed here. Instead, `prepare_*`
/// methods produce request structs that the caller dispatches to a backend,
/// then feeds results back via `record_*` methods.
#[derive(Debug, Clone)]
pub struct VirtualShell {
	sessions: ImHashMap<String, ShellSession>,
	sandbox: SandboxConfig,
	shell_cmd: String,
	total_commands: u64,
	total_errors: u64,
}

impl VirtualShell {
	pub fn new(sandbox: SandboxConfig, shell_cmd: String) -> Self {
		Self {
			sessions: ImHashMap::new(),
			sandbox,
			shell_cmd,
			total_commands: 0,
			total_errors: 0,
		}
	}

	// -- Session CRUD ---------------------------------------------------------

	pub fn create_session(
		self,
		name: Option<String>,
		cwd: Option<String>,
		env: Option<HashMap<String, String>>,
	) -> Result<(Self, ShellSession), SandboxError> {
		if self.sessions.len() >= self.sandbox.max_sessions {
			return Err(SandboxError::VshLimitExceeded(format!(
				"Maximum sessions ({}) reached",
				self.sandbox.max_sessions,
			)));
		}

		let id = uuid::Uuid::new_v4().to_string();
		let resolved_cwd = match cwd {
			Some(p) => self.sandbox.validate_cwd(&PathBuf::from(&p))?,
			None => self.sandbox.root_directory.clone(),
		};

		let now = now_ms();
		let im_env: ImHashMap<String, String> = env
			.unwrap_or_default()
			.into_iter()
			.collect();

		let session = ShellSession {
			id: id.clone(),
			name,
			cwd: resolved_cwd,
			env: im_env,
			aliases: ImHashMap::new(),
			history: ImVector::new(),
			created_at: now,
			last_active_at: now,
		};

		let mut new_self = self;
		new_self.sessions = new_self.sessions.update(id, session.clone());
		Ok((new_self, session))
	}

	pub fn get_session(&self, id: &str) -> Result<&ShellSession, SandboxError> {
		self.sessions
			.get(id)
			.ok_or_else(|| SandboxError::VshSessionNotFound(id.to_string()))
	}

	pub fn list_sessions(&self) -> Vec<&ShellSession> {
		self.sessions.values().collect()
	}

	pub fn delete_session(self, id: &str) -> Result<(Self, bool), SandboxError> {
		if !self.sessions.contains_key(id) {
			return Err(SandboxError::VshSessionNotFound(id.to_string()));
		}
		let mut new_self = self;
		new_self.sessions = new_self.sessions.without(id);
		Ok((new_self, true))
	}

	// -- Prepare (pure — no I/O) ----------------------------------------------

	/// Prepare a command for execution within a session.
	///
	/// Validates the command against the sandbox, resolves aliases,
	/// and returns an [`ExecRequest`] with all data needed for execution.
	/// The caller must run the command via a backend and then call
	/// [`record_exec`] with the result.
	pub fn prepare_exec(
		&self,
		session_id: &str,
		command: &str,
		timeout_ms: Option<u64>,
		max_output_bytes: Option<usize>,
		stdin: Option<&str>,
	) -> Result<ExecRequest, SandboxError> {
		self.sandbox.check_command(command)?;

		let session = self
			.sessions
			.get(session_id)
			.ok_or_else(|| SandboxError::VshSessionNotFound(session_id.to_string()))?;

		let resolved_command = self.resolve_alias(session_id, command);
		let timeout = timeout_ms.unwrap_or(self.sandbox.default_timeout_ms);
		let max_out = max_output_bytes.unwrap_or(self.sandbox.max_output_bytes);

		Ok(ExecRequest {
			session_id: session_id.to_string(),
			command: command.to_string(),
			resolved_command,
			cwd: session.cwd.clone(),
			env: session.env.clone(),
			shell_cmd: self.shell_cmd.clone(),
			timeout_ms: timeout,
			max_output_bytes: max_out,
			stdin: stdin.map(String::from),
		})
	}

	/// Prepare a git command for execution within a session.
	///
	/// Validates the command against blocked patterns and returns a
	/// [`GitExecRequest`] with all data needed for execution.
	/// The caller must run the command via a backend and then call
	/// [`record_exec`] with the result.
	pub fn prepare_exec_git(
		&self,
		session_id: &str,
		args: &[String],
		timeout_ms: Option<u64>,
		max_output_bytes: Option<usize>,
	) -> Result<GitExecRequest, SandboxError> {
		let command_str = format!("git {}", args.join(" "));
		self.sandbox.check_command(&command_str)?;

		let session = self
			.sessions
			.get(session_id)
			.ok_or_else(|| SandboxError::VshSessionNotFound(session_id.to_string()))?;

		let timeout = timeout_ms.unwrap_or(self.sandbox.default_timeout_ms);
		let max_out = max_output_bytes.unwrap_or(self.sandbox.max_output_bytes);

		Ok(GitExecRequest {
			session_id: session_id.to_string(),
			command_str,
			args: args.to_vec(),
			cwd: session.cwd.clone(),
			env: session.env.clone(),
			timeout_ms: timeout,
			max_output_bytes: max_out,
		})
	}

	/// Prepare a raw (session-less) command for execution.
	///
	/// Validates the command and cwd against the sandbox, and returns a
	/// [`RawExecRequest`] with all data needed for execution.
	/// The caller must run the command via a backend and then call
	/// [`record_exec_raw`] with the result.
	#[allow(clippy::too_many_arguments)]
	pub fn prepare_exec_raw(
		&self,
		command: &str,
		cwd: Option<&str>,
		env: Option<&HashMap<String, String>>,
		shell: Option<&str>,
		timeout_ms: Option<u64>,
		max_output_bytes: Option<usize>,
		stdin: Option<&str>,
	) -> Result<RawExecRequest, SandboxError> {
		self.sandbox.check_command(command)?;

		let resolved_cwd = match cwd {
			Some(p) => self.sandbox.validate_cwd(&PathBuf::from(p))?,
			None => self.sandbox.root_directory.clone(),
		};

		let empty_env = HashMap::new();
		let env = env.unwrap_or(&empty_env);
		let shell_cmd = shell.unwrap_or(&self.shell_cmd);
		let timeout = timeout_ms.unwrap_or(self.sandbox.default_timeout_ms);
		let max_out = max_output_bytes.unwrap_or(self.sandbox.max_output_bytes);

		Ok(RawExecRequest {
			command: command.to_string(),
			cwd: resolved_cwd,
			env: env.clone(),
			shell_cmd: shell_cmd.to_string(),
			timeout_ms: timeout,
			max_output_bytes: max_out,
			stdin: stdin.map(String::from),
		})
	}

	// -- Record results (pure state update) -----------------------------------

	/// Record the result of a session-bound command execution.
	///
	/// Updates the session's history and the global metrics counters.
	pub fn record_exec(
		self,
		session_id: &str,
		command: &str,
		result: &Result<ExecResult, SandboxError>,
	) -> Self {
		let mut new_self = self;
		new_self.total_commands += 1;

		let (exit_code, duration_ms) = match result {
			Ok(exec_result) => (exec_result.exit_code, exec_result.duration_ms),
			Err(_) => {
				new_self.total_errors += 1;
				(-1, 0)
			}
		};

		if let Some(session) = new_self.sessions.get(session_id) {
			let mut session = session.clone();
			session.last_active_at = now_ms();
			session.history.push_back(HistoryEntry {
				command: command.to_string(),
				exit_code,
				timestamp: now_ms(),
				duration_ms,
			});
			new_self.sessions = new_self.sessions.update(session_id.to_string(), session);
		}

		new_self
	}

	/// Record the result of a raw (session-less) command execution.
	///
	/// Updates only the global metrics counters (no session history).
	pub fn record_exec_raw(self, result: &Result<ExecResult, SandboxError>) -> Self {
		let mut new_self = self;
		new_self.total_commands += 1;
		if result.is_err() {
			new_self.total_errors += 1;
		}
		new_self
	}

	// -- Env ------------------------------------------------------------------

	pub fn set_env(self, session_id: &str, key: &str, value: &str) -> Result<Self, SandboxError> {
		let session = self
			.sessions
			.get(session_id)
			.ok_or_else(|| SandboxError::VshSessionNotFound(session_id.to_string()))?;

		let mut session = session.clone();
		session.env = session.env.update(key.to_string(), value.to_string());
		session.last_active_at = now_ms();

		let mut new_self = self;
		new_self.sessions = new_self.sessions.update(session_id.to_string(), session);
		Ok(new_self)
	}

	pub fn get_env(&self, session_id: &str, key: &str) -> Result<Option<String>, SandboxError> {
		let session = self.get_session(session_id)?;
		Ok(session.env.get(key).cloned())
	}

	pub fn list_env(
		&self,
		session_id: &str,
	) -> Result<&ImHashMap<String, String>, SandboxError> {
		let session = self.get_session(session_id)?;
		Ok(&session.env)
	}

	pub fn delete_env(self, session_id: &str, key: &str) -> Result<(Self, bool), SandboxError> {
		let session = self
			.sessions
			.get(session_id)
			.ok_or_else(|| SandboxError::VshSessionNotFound(session_id.to_string()))?;

		let existed = session.env.contains_key(key);
		let mut session = session.clone();
		session.env = session.env.without(key);
		session.last_active_at = now_ms();

		let mut new_self = self;
		new_self.sessions = new_self.sessions.update(session_id.to_string(), session);
		Ok((new_self, existed))
	}

	// -- Shell state ----------------------------------------------------------

	pub fn set_cwd(self, session_id: &str, cwd: &str) -> Result<Self, SandboxError> {
		let validated = self.sandbox.validate_cwd(&PathBuf::from(cwd))?;

		let session = self
			.sessions
			.get(session_id)
			.ok_or_else(|| SandboxError::VshSessionNotFound(session_id.to_string()))?;

		let mut session = session.clone();
		session.cwd = validated;
		session.last_active_at = now_ms();

		let mut new_self = self;
		new_self.sessions = new_self.sessions.update(session_id.to_string(), session);
		Ok(new_self)
	}

	pub fn get_cwd(&self, session_id: &str) -> Result<String, SandboxError> {
		let session = self.get_session(session_id)?;
		Ok(session.cwd.display().to_string())
	}

	pub fn set_alias(
		self,
		session_id: &str,
		name: &str,
		command: &str,
	) -> Result<Self, SandboxError> {
		let session = self
			.sessions
			.get(session_id)
			.ok_or_else(|| SandboxError::VshSessionNotFound(session_id.to_string()))?;

		let mut session = session.clone();
		session.aliases = session
			.aliases
			.update(name.to_string(), command.to_string());
		session.last_active_at = now_ms();

		let mut new_self = self;
		new_self.sessions = new_self.sessions.update(session_id.to_string(), session);
		Ok(new_self)
	}

	pub fn get_aliases(
		&self,
		session_id: &str,
	) -> Result<&ImHashMap<String, String>, SandboxError> {
		let session = self.get_session(session_id)?;
		Ok(&session.aliases)
	}

	pub fn get_history(
		&self,
		session_id: &str,
	) -> Result<&ImVector<HistoryEntry>, SandboxError> {
		let session = self.get_session(session_id)?;
		Ok(&session.history)
	}

	// -- Metrics --------------------------------------------------------------

	pub fn session_count(&self) -> usize {
		self.sessions.len()
	}

	pub fn total_commands(&self) -> u64 {
		self.total_commands
	}

	pub fn total_errors(&self) -> u64 {
		self.total_errors
	}

	// -- Private helpers ------------------------------------------------------

	fn resolve_alias(&self, session_id: &str, command: &str) -> String {
		if let Some(session) = self.sessions.get(session_id) {
			let first_word = command.split_whitespace().next().unwrap_or("");
			if let Some(alias_cmd) = session.aliases.get(first_word) {
				let rest = command
					.strip_prefix(first_word)
					.unwrap_or("")
					.trim_start();
				if rest.is_empty() {
					return alias_cmd.clone();
				}
				return format!("{} {}", alias_cmd, rest);
			}
		}
		command.to_string()
	}
}
