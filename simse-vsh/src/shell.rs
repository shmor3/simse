use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::backend::ShellBackend;
use crate::error::VshError;
use crate::executor::ExecResult;
use crate::sandbox::SandboxConfig;

fn now_ms() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64
}

/// A single shell session with its own state.
#[derive(Debug)]
pub struct ShellSession {
	pub id: String,
	pub name: Option<String>,
	pub cwd: PathBuf,
	pub env: HashMap<String, String>,
	pub aliases: HashMap<String, String>,
	pub history: Vec<HistoryEntry>,
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

/// The VirtualShell manages all sessions and delegates execution.
pub struct VirtualShell {
	sessions: HashMap<String, ShellSession>,
	sandbox: SandboxConfig,
	shell: String,
	backend: Box<dyn ShellBackend>,
	total_commands: u64,
	total_errors: u64,
}

impl VirtualShell {
	pub fn new(sandbox: SandboxConfig, shell: String, backend: Box<dyn ShellBackend>) -> Self {
		Self {
			sessions: HashMap::new(),
			sandbox,
			shell,
			backend,
			total_commands: 0,
			total_errors: 0,
		}
	}

	// -- Session CRUD ---------------------------------------------------------

	pub fn create_session(
		&mut self,
		name: Option<String>,
		cwd: Option<String>,
		env: Option<HashMap<String, String>>,
	) -> Result<&ShellSession, VshError> {
		if self.sessions.len() >= self.sandbox.max_sessions {
			return Err(VshError::LimitExceeded(format!(
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
		let session = ShellSession {
			id: id.clone(),
			name,
			cwd: resolved_cwd,
			env: env.unwrap_or_default(),
			aliases: HashMap::new(),
			history: Vec::new(),
			created_at: now,
			last_active_at: now,
		};

		self.sessions.insert(id.clone(), session);
		Ok(self.sessions.get(&id).unwrap())
	}

	pub fn get_session(&self, id: &str) -> Result<&ShellSession, VshError> {
		self.sessions
			.get(id)
			.ok_or_else(|| VshError::SessionNotFound(id.to_string()))
	}

	pub fn list_sessions(&self) -> Vec<&ShellSession> {
		self.sessions.values().collect()
	}

	pub fn delete_session(&mut self, id: &str) -> Result<bool, VshError> {
		if self.sessions.remove(id).is_some() {
			Ok(true)
		} else {
			Err(VshError::SessionNotFound(id.to_string()))
		}
	}

	// -- Execution ------------------------------------------------------------

	pub async fn exec_in_session(
		&mut self,
		session_id: &str,
		command: &str,
		timeout_ms: Option<u64>,
		max_output_bytes: Option<usize>,
		stdin: Option<&str>,
	) -> Result<ExecResult, VshError> {
		// Validate command against sandbox
		self.sandbox.check_command(command)?;

		let session = self
			.sessions
			.get(session_id)
			.ok_or_else(|| VshError::SessionNotFound(session_id.to_string()))?;

		let cwd = session.cwd.clone();
		let env = session.env.clone();
		let timeout = timeout_ms.unwrap_or(self.sandbox.default_timeout_ms);
		let max_out = max_output_bytes.unwrap_or(self.sandbox.max_output_bytes);
		let shell = self.shell.clone();

		// Resolve aliases
		let resolved_command = self.resolve_alias(session_id, command);

		let result = self
			.backend
			.execute_command(&resolved_command, &cwd, &env, &shell, timeout, max_out, stdin)
			.await;

		self.record_history(session_id, command, &result);
		result
	}

	pub async fn exec_git_in_session(
		&mut self,
		session_id: &str,
		args: &[String],
		timeout_ms: Option<u64>,
		max_output_bytes: Option<usize>,
	) -> Result<ExecResult, VshError> {
		// Validate git command against blocked patterns
		let command_str = format!("git {}", args.join(" "));
		self.sandbox.check_command(&command_str)?;

		let session = self
			.sessions
			.get(session_id)
			.ok_or_else(|| VshError::SessionNotFound(session_id.to_string()))?;

		let cwd = session.cwd.clone();
		let env = session.env.clone();
		let timeout = timeout_ms.unwrap_or(self.sandbox.default_timeout_ms);
		let max_out = max_output_bytes.unwrap_or(self.sandbox.max_output_bytes);

		let result = self.backend.execute_git(args, &cwd, &env, timeout, max_out).await;

		self.record_history(session_id, &command_str, &result);
		result
	}

	/// Execute a command without a session (stateless).
	#[allow(clippy::too_many_arguments)]
	pub async fn exec_raw(
		&mut self,
		command: &str,
		cwd: Option<&str>,
		env: Option<&HashMap<String, String>>,
		shell: Option<&str>,
		timeout_ms: Option<u64>,
		max_output_bytes: Option<usize>,
		stdin: Option<&str>,
	) -> Result<ExecResult, VshError> {
		self.sandbox.check_command(command)?;

		let resolved_cwd = match cwd {
			Some(p) => self.sandbox.validate_cwd(&PathBuf::from(p))?,
			None => self.sandbox.root_directory.clone(),
		};

		let empty_env = HashMap::new();
		let env = env.unwrap_or(&empty_env);
		let shell = shell.unwrap_or(&self.shell);
		let timeout = timeout_ms.unwrap_or(self.sandbox.default_timeout_ms);
		let max_out = max_output_bytes.unwrap_or(self.sandbox.max_output_bytes);

		self.total_commands += 1;

		let result = self
			.backend
			.execute_command(command, &resolved_cwd, env, shell, timeout, max_out, stdin)
			.await;

		if result.is_err() {
			self.total_errors += 1;
		}

		result
	}

	// -- Env ------------------------------------------------------------------

	pub fn set_env(
		&mut self,
		session_id: &str,
		key: &str,
		value: &str,
	) -> Result<(), VshError> {
		let session = self
			.sessions
			.get_mut(session_id)
			.ok_or_else(|| VshError::SessionNotFound(session_id.to_string()))?;
		session.env.insert(key.to_string(), value.to_string());
		session.last_active_at = now_ms();
		Ok(())
	}

	pub fn get_env(&self, session_id: &str, key: &str) -> Result<Option<String>, VshError> {
		let session = self.get_session(session_id)?;
		Ok(session.env.get(key).cloned())
	}

	pub fn list_env(&self, session_id: &str) -> Result<&HashMap<String, String>, VshError> {
		let session = self.get_session(session_id)?;
		Ok(&session.env)
	}

	pub fn delete_env(&mut self, session_id: &str, key: &str) -> Result<bool, VshError> {
		let session = self
			.sessions
			.get_mut(session_id)
			.ok_or_else(|| VshError::SessionNotFound(session_id.to_string()))?;
		session.last_active_at = now_ms();
		Ok(session.env.remove(key).is_some())
	}

	// -- Shell state ----------------------------------------------------------

	pub fn set_cwd(&mut self, session_id: &str, cwd: &str) -> Result<(), VshError> {
		let validated = self.sandbox.validate_cwd(&PathBuf::from(cwd))?;
		let session = self
			.sessions
			.get_mut(session_id)
			.ok_or_else(|| VshError::SessionNotFound(session_id.to_string()))?;
		session.cwd = validated;
		session.last_active_at = now_ms();
		Ok(())
	}

	pub fn get_cwd(&self, session_id: &str) -> Result<String, VshError> {
		let session = self.get_session(session_id)?;
		Ok(session.cwd.display().to_string())
	}

	pub fn set_alias(
		&mut self,
		session_id: &str,
		name: &str,
		command: &str,
	) -> Result<(), VshError> {
		let session = self
			.sessions
			.get_mut(session_id)
			.ok_or_else(|| VshError::SessionNotFound(session_id.to_string()))?;
		session
			.aliases
			.insert(name.to_string(), command.to_string());
		session.last_active_at = now_ms();
		Ok(())
	}

	pub fn get_aliases(
		&self,
		session_id: &str,
	) -> Result<&HashMap<String, String>, VshError> {
		let session = self.get_session(session_id)?;
		Ok(&session.aliases)
	}

	pub fn get_history(&self, session_id: &str) -> Result<&[HistoryEntry], VshError> {
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

	/// Record a command execution in the session's history and update metrics.
	fn record_history(
		&mut self,
		session_id: &str,
		command: &str,
		result: &Result<ExecResult, VshError>,
	) {
		self.total_commands += 1;
		match result {
			Ok(exec_result) => {
				let session = self.sessions.get_mut(session_id).unwrap();
				session.last_active_at = now_ms();
				session.history.push(HistoryEntry {
					command: command.to_string(),
					exit_code: exec_result.exit_code,
					timestamp: now_ms(),
					duration_ms: exec_result.duration_ms,
				});
			}
			Err(_) => {
				self.total_errors += 1;
				let session = self.sessions.get_mut(session_id).unwrap();
				session.last_active_at = now_ms();
				session.history.push(HistoryEntry {
					command: command.to_string(),
					exit_code: -1,
					timestamp: now_ms(),
					duration_ms: 0,
				});
			}
		}
	}

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
