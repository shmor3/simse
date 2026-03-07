// ---------------------------------------------------------------------------
// Direct Rust API tests for VirtualShell (ported from simse-vsh integration tests)
//
// These tests call the VirtualShell API directly instead of going through
// JSON-RPC, exercising the same coverage as simse-vsh/tests/integration.rs.
//
// FP pattern: VirtualShell is pure state. Mutation methods take `self` by
// value and return `Self` (or `(Self, T)`). I/O uses the
// prepare/backend/record three-step flow with a separate backend.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use simse_sandbox_engine::error::SandboxError;
use simse_sandbox_engine::vsh_backend::{LocalShell, ShellImpl};
use simse_sandbox_engine::vsh_sandbox::SandboxConfig;
use simse_sandbox_engine::vsh_shell::VirtualShell;

/// Detect a working shell path (cross-platform: Linux, macOS, Windows/MINGW64).
fn shell_path() -> String {
	// Unix paths
	for candidate in &["/bin/sh", "/usr/bin/sh", "/usr/bin/bash"] {
		if std::path::Path::new(candidate).exists() {
			return candidate.to_string();
		}
	}
	// Windows: Git Bash / MSYS2 shell
	for candidate in &[
		r"C:\Program Files\Git\usr\bin\sh.exe",
		r"C:\Program Files\Git\usr\bin\bash.exe",
		r"C:\Program Files\Git\bin\sh.exe",
	] {
		if std::path::Path::new(candidate).exists() {
			return candidate.to_string();
		}
	}
	// Last resort — check PATH via `which`
	if let Ok(output) = std::process::Command::new("which").arg("sh").output() {
		if output.status.success() {
			let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
			if !path.is_empty() {
				return path;
			}
		}
	}
	"/bin/sh".to_string()
}

/// Create a VirtualShell with the given temp directory as root.
fn make_shell(root: &std::path::Path) -> VirtualShell {
	let sandbox = SandboxConfig {
		root_directory: root.to_path_buf(),
		allowed_paths: Vec::new(),
		blocked_patterns: Vec::new(),
		max_sessions: 32,
		default_timeout_ms: 30_000,
		max_output_bytes: 50_000,
	};
	VirtualShell::new(sandbox, shell_path())
}

/// Create a local shell backend for I/O.
fn make_backend() -> ShellImpl {
	ShellImpl::Local(LocalShell)
}

/// Helper: prepare + backend I/O + record for a session-bound command.
async fn run_in_session(
	shell: VirtualShell,
	backend: &ShellImpl,
	session_id: &str,
	command: &str,
	timeout_ms: Option<u64>,
	max_output_bytes: Option<usize>,
	stdin: Option<&str>,
) -> (VirtualShell, Result<simse_sandbox_engine::vsh_executor::ExecResult, SandboxError>) {
	let req = shell
		.prepare_exec(session_id, command, timeout_ms, max_output_bytes, stdin)
		.unwrap();

	let env: HashMap<String, String> =
		req.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
	let result = backend
		.execute_command(
			&req.resolved_command,
			&req.cwd,
			&env,
			&req.shell_cmd,
			req.timeout_ms,
			req.max_output_bytes,
			req.stdin.as_deref(),
		)
		.await;

	let shell = shell.record_exec(&req.session_id, &req.command, &result);
	(shell, result)
}

/// Helper: prepare + backend I/O + record for a raw (session-less) command.
async fn run_raw(
	shell: VirtualShell,
	backend: &ShellImpl,
	command: &str,
	cwd: Option<&str>,
	env: Option<&HashMap<String, String>>,
	shell_override: Option<&str>,
	timeout_ms: Option<u64>,
	max_output_bytes: Option<usize>,
	stdin: Option<&str>,
) -> (VirtualShell, Result<simse_sandbox_engine::vsh_executor::ExecResult, SandboxError>) {
	let req = shell
		.prepare_exec_raw(command, cwd, env, shell_override, timeout_ms, max_output_bytes, stdin)
		.unwrap();

	let result = backend
		.execute_command(
			&req.command,
			&req.cwd,
			&req.env,
			&req.shell_cmd,
			req.timeout_ms,
			req.max_output_bytes,
			req.stdin.as_deref(),
		)
		.await;

	let shell = shell.record_exec_raw(&result);
	(shell, result)
}

// ---------------------------------------------------------------------------
// Test 1: create session and verify it exists
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_session_and_verify() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	assert!(!id.is_empty());
	assert!(session.created_at > 0);
	assert!(session.history.is_empty());

	// Verify it exists via get_session
	let fetched = shell.get_session(&id).unwrap();
	assert_eq!(fetched.id, id);
}

// ---------------------------------------------------------------------------
// Test 2: run command in session (echo)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_echo() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	let (_, result) =
		run_in_session(shell, &backend, &id, "echo hello", None, None, None).await;
	let result = result.unwrap();

	assert_eq!(result.exit_code, 0);
	assert!(
		result.stdout.contains("hello"),
		"stdout should contain 'hello', got: {}",
		result.stdout
	);
}

// ---------------------------------------------------------------------------
// Test 3: run command with env vars
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_with_env() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Set an env var
	let shell = shell.set_env(&id, "MY_VAR", "hello_env").unwrap();

	// Verify via get_env
	let val = shell.get_env(&id, "MY_VAR").unwrap();
	assert_eq!(val, Some("hello_env".to_string()));

	// Verify via get_session that env is present
	let session_info = shell.get_session(&id).unwrap();
	assert_eq!(session_info.env.get("MY_VAR").unwrap(), "hello_env");

	// Verify running a command still works in this session
	let (_, result) =
		run_in_session(shell, &backend, &id, "echo works", None, None, None).await;
	let result = result.unwrap();
	assert_eq!(result.exit_code, 0);
	assert!(result.stdout.contains("works"));
}

// ---------------------------------------------------------------------------
// Test 4: raw stateless run
// ---------------------------------------------------------------------------

#[tokio::test]
async fn raw_stateless() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (_, result) =
		run_raw(shell, &backend, "echo raw_output", None, None, None, None, None, None).await;
	let result = result.unwrap();

	assert_eq!(result.exit_code, 0);
	assert!(
		result.stdout.contains("raw_output"),
		"stdout should contain 'raw_output', got: {}",
		result.stdout
	);
}

// ---------------------------------------------------------------------------
// Test 5: session working directory changes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_cwd_changes() {
	let dir = tempfile::tempdir().unwrap();
	let subdir = dir.path().join("subdir");
	std::fs::create_dir(&subdir).unwrap();

	let shell = make_shell(dir.path());

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Change cwd
	let shell = shell.set_cwd(&id, subdir.to_str().unwrap()).unwrap();

	// Verify cwd
	let cwd = shell.get_cwd(&id).unwrap();
	assert!(
		cwd.contains("subdir"),
		"cwd should contain 'subdir', got: {}",
		cwd
	);
}

// ---------------------------------------------------------------------------
// Test 6: alias resolution
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alias_resolution() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Set alias
	let shell = shell
		.set_alias(&id, "greet", "echo hello from alias")
		.unwrap();

	// Verify aliases
	let aliases = shell.get_aliases(&id).unwrap();
	assert_eq!(aliases.get("greet").unwrap(), "echo hello from alias");

	// Run aliased command
	let (_, result) =
		run_in_session(shell, &backend, &id, "greet", None, None, None).await;
	let result = result.unwrap();

	assert_eq!(result.exit_code, 0);
	assert!(
		result.stdout.contains("hello from alias"),
		"stdout should contain alias expansion, got: {}",
		result.stdout
	);
}

// ---------------------------------------------------------------------------
// Test 7: command history tracking
// ---------------------------------------------------------------------------

#[tokio::test]
async fn command_history_tracking() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Run a few commands
	let (shell, _) =
		run_in_session(shell, &backend, &id, "echo one", None, None, None).await;
	let (shell, _) =
		run_in_session(shell, &backend, &id, "echo two", None, None, None).await;

	// Get history
	let history = shell.get_history(&id).unwrap();
	assert_eq!(history.len(), 2);
	assert_eq!(history[0].command, "echo one");
	assert_eq!(history[1].command, "echo two");
	assert_eq!(history[0].exit_code, 0);
}

// ---------------------------------------------------------------------------
// Test 8: session list and delete
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_list_and_delete() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());

	// Create two sessions
	let (shell, s1) = shell.create_session(None, None, None).unwrap();
	let s1_id = s1.id.clone();
	let (shell, s2) = shell.create_session(None, None, None).unwrap();
	let s2_id = s2.id.clone();

	// List sessions
	let sessions = shell.list_sessions();
	assert_eq!(sessions.len(), 2);

	// Delete one
	let (shell, deleted) = shell.delete_session(&s1_id).unwrap();
	assert!(deleted);

	// List again
	let sessions = shell.list_sessions();
	assert_eq!(sessions.len(), 1);
	assert_eq!(sessions[0].id, s2_id);
}

// ---------------------------------------------------------------------------
// Test 9: sandbox violation (path outside allowed)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sandbox_violation() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Try to set cwd outside sandbox root
	let err = shell.set_cwd(&id, "/tmp/totally-outside").unwrap_err();

	match &err {
		SandboxError::VshSandboxViolation(msg) => {
			assert!(
				msg.contains("outside"),
				"error message should mention 'outside', got: {}",
				msg
			);
		}
		other => panic!(
			"expected VshSandboxViolation, got: {:?}",
			other
		),
	}

	assert_eq!(err.code(), "SANDBOX_VSH_SANDBOX_VIOLATION");
}

// ---------------------------------------------------------------------------
// Test 10: command timeout
// ---------------------------------------------------------------------------

#[tokio::test]
async fn command_timeout() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Run a long sleep with a very short timeout
	let req = shell
		.prepare_exec(&id, "sleep 10", Some(500), None, None)
		.unwrap();

	let env: HashMap<String, String> =
		req.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
	let result = backend
		.execute_command(
			&req.resolved_command,
			&req.cwd,
			&env,
			&req.shell_cmd,
			req.timeout_ms,
			req.max_output_bytes,
			req.stdin.as_deref(),
		)
		.await;

	// The result should be a timeout error
	let err = result.unwrap_err();

	match &err {
		SandboxError::VshTimeout(msg) => {
			assert!(
				msg.contains("timed out"),
				"error message should mention 'timed out', got: {}",
				msg
			);
		}
		other => panic!("expected VshTimeout, got: {:?}", other),
	}

	assert_eq!(err.code(), "SANDBOX_VSH_TIMEOUT");
}

// ---------------------------------------------------------------------------
// Test 11: environment variable CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn env_operations() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Set env var
	let shell = shell.set_env(&id, "TEST_KEY", "test_value").unwrap();

	// Get env var
	let val = shell.get_env(&id, "TEST_KEY").unwrap();
	assert_eq!(val, Some("test_value".to_string()));

	// List env
	let env = shell.list_env(&id).unwrap();
	assert_eq!(env.get("TEST_KEY").unwrap(), "test_value");

	// Delete env var
	let (shell, deleted) = shell.delete_env(&id, "TEST_KEY").unwrap();
	assert!(deleted);

	// Verify it's gone
	let val = shell.get_env(&id, "TEST_KEY").unwrap();
	assert!(val.is_none());
}

// ---------------------------------------------------------------------------
// Test 12: shell metrics
// ---------------------------------------------------------------------------

#[tokio::test]
async fn shell_metrics() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Run a command
	let (shell, _) =
		run_in_session(shell, &backend, &id, "echo metric_test", None, None, None).await;

	// Check metrics
	assert_eq!(shell.session_count(), 1);
	assert!(shell.total_commands() >= 1);
}

// ---------------------------------------------------------------------------
// Test 13: session not found error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_not_found() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());

	let err = shell
		.prepare_exec("nonexistent", "echo hello", None, None, None)
		.unwrap_err();

	match &err {
		SandboxError::VshSessionNotFound(sid) => {
			assert_eq!(sid, "nonexistent");
		}
		other => panic!("expected VshSessionNotFound, got: {:?}", other),
	}

	assert_eq!(err.code(), "SANDBOX_VSH_SESSION_NOT_FOUND");
}

// ---------------------------------------------------------------------------
// Test 14: blocked command pattern
// ---------------------------------------------------------------------------

#[tokio::test]
async fn blocked_command_pattern() {
	let dir = tempfile::tempdir().unwrap();

	let sandbox = SandboxConfig {
		root_directory: dir.path().to_path_buf(),
		allowed_paths: Vec::new(),
		blocked_patterns: vec!["rm -rf /".to_string()],
		max_sessions: 32,
		default_timeout_ms: 30_000,
		max_output_bytes: 50_000,
	};
	let shell = VirtualShell::new(sandbox, shell_path());

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	let err = shell
		.prepare_exec(&id, "rm -rf /", None, None, None)
		.unwrap_err();

	match &err {
		SandboxError::VshSandboxViolation(msg) => {
			assert!(
				msg.contains("blocked"),
				"error should mention 'blocked', got: {}",
				msg
			);
		}
		other => panic!("expected VshSandboxViolation, got: {:?}", other),
	}
}

// ---------------------------------------------------------------------------
// Test 15: session max limit
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_max_limit() {
	let dir = tempfile::tempdir().unwrap();

	let sandbox = SandboxConfig {
		root_directory: dir.path().to_path_buf(),
		allowed_paths: Vec::new(),
		blocked_patterns: Vec::new(),
		max_sessions: 2,
		default_timeout_ms: 30_000,
		max_output_bytes: 50_000,
	};
	let shell = VirtualShell::new(sandbox, shell_path());

	// Create two sessions (max)
	let (shell, _) = shell.create_session(None, None, None).unwrap();
	let (shell, _) = shell.create_session(None, None, None).unwrap();

	// Third should fail
	let err = shell.create_session(None, None, None).unwrap_err();

	match &err {
		SandboxError::VshLimitExceeded(msg) => {
			assert!(
				msg.contains("Maximum sessions"),
				"error should mention max sessions, got: {}",
				msg
			);
		}
		other => panic!("expected VshLimitExceeded, got: {:?}", other),
	}
}

// ---------------------------------------------------------------------------
// Test 16: create session with initial cwd and env
// ---------------------------------------------------------------------------

#[tokio::test]
async fn create_session_with_cwd_and_env() {
	let dir = tempfile::tempdir().unwrap();
	let subdir = dir.path().join("mydir");
	std::fs::create_dir(&subdir).unwrap();

	let shell = make_shell(dir.path());

	let mut initial_env = HashMap::new();
	initial_env.insert("FOO".to_string(), "bar".to_string());

	let (_, session) = shell
		.create_session(
			Some("my-session".to_string()),
			Some(subdir.to_str().unwrap().to_string()),
			Some(initial_env),
		)
		.unwrap();

	assert_eq!(session.name.as_deref(), Some("my-session"));
	assert!(session.cwd.to_str().unwrap().contains("mydir"));
	assert_eq!(session.env.get("FOO").unwrap(), "bar");
}

// ---------------------------------------------------------------------------
// Test 17: raw run with custom cwd
// ---------------------------------------------------------------------------

#[tokio::test]
async fn raw_with_custom_cwd() {
	let dir = tempfile::tempdir().unwrap();
	let subdir = dir.path().join("rawdir");
	std::fs::create_dir(&subdir).unwrap();

	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (_, result) = run_raw(
		shell,
		&backend,
		"pwd",
		Some(subdir.to_str().unwrap()),
		None,
		None,
		None,
		None,
		None,
	)
	.await;
	let result = result.unwrap();

	assert_eq!(result.exit_code, 0);
	assert!(
		result.stdout.contains("rawdir"),
		"stdout should contain 'rawdir', got: {}",
		result.stdout
	);
}

// ---------------------------------------------------------------------------
// Test 18: delete nonexistent env var returns false
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_nonexistent_env_var() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	let (_, deleted) = shell.delete_env(&id, "DOES_NOT_EXIST").unwrap();
	assert!(!deleted);
}

// ---------------------------------------------------------------------------
// Test 19: delete nonexistent session returns error
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_nonexistent_session() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());

	let err = shell.delete_session("no-such-id").unwrap_err();
	match &err {
		SandboxError::VshSessionNotFound(sid) => {
			assert_eq!(sid, "no-such-id");
		}
		other => panic!("expected VshSessionNotFound, got: {:?}", other),
	}
}

// ---------------------------------------------------------------------------
// Test 20: alias with arguments
// ---------------------------------------------------------------------------

#[tokio::test]
async fn alias_with_arguments() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Set alias that takes additional arguments
	let shell = shell.set_alias(&id, "say", "echo").unwrap();

	let (_, result) =
		run_in_session(shell, &backend, &id, "say hello world", None, None, None).await;
	let result = result.unwrap();

	assert_eq!(result.exit_code, 0);
	assert!(
		result.stdout.contains("hello world"),
		"stdout should contain 'hello world', got: {}",
		result.stdout
	);
}

// ---------------------------------------------------------------------------
// Test 21: history records non-zero exit codes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn history_records_nonzero_exit() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Run a command that succeeds
	let (shell, _) =
		run_in_session(shell, &backend, &id, "echo ok", None, None, None).await;

	// Run a command that fails (non-zero exit code)
	let (shell, result) =
		run_in_session(shell, &backend, &id, "false", None, None, None).await;
	let result = result.unwrap();
	assert_ne!(result.exit_code, 0);

	// History should have 2 entries
	let history = shell.get_history(&id).unwrap();
	assert_eq!(history.len(), 2);
	assert_eq!(history[0].command, "echo ok");
	assert_eq!(history[0].exit_code, 0);
	assert_eq!(history[1].command, "false");
	assert_ne!(history[1].exit_code, 0);
}

// ---------------------------------------------------------------------------
// Test 21b: blocked commands do not record history (early rejection)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn blocked_command_skips_history() {
	let dir = tempfile::tempdir().unwrap();

	let sandbox = SandboxConfig {
		root_directory: dir.path().to_path_buf(),
		allowed_paths: Vec::new(),
		blocked_patterns: vec!["forbidden_cmd".to_string()],
		max_sessions: 32,
		default_timeout_ms: 30_000,
		max_output_bytes: 50_000,
	};
	let shell = VirtualShell::new(sandbox, shell_path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	// Run a successful command
	let (shell, _) =
		run_in_session(shell, &backend, &id, "echo ok", None, None, None).await;

	// Blocked command is rejected before I/O, so no history entry
	let err = shell
		.prepare_exec(&id, "forbidden_cmd", None, None, None)
		.unwrap_err();
	assert_eq!(err.code(), "SANDBOX_VSH_SANDBOX_VIOLATION");

	// Only the successful command should be in history
	let history = shell.get_history(&id).unwrap();
	assert_eq!(history.len(), 1);
	assert_eq!(history[0].command, "echo ok");
}

// ---------------------------------------------------------------------------
// Test 22: metrics track raw commands and errors
// ---------------------------------------------------------------------------

#[tokio::test]
async fn metrics_track_raw() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	// raw run tracks total_commands independently
	let (shell, _) =
		run_raw(shell, &backend, "echo one", None, None, None, None, None, None).await;
	let (shell, _) =
		run_raw(shell, &backend, "echo two", None, None, None, None, None, None).await;

	assert!(
		shell.total_commands() >= 2,
		"total_commands should be >= 2, got: {}",
		shell.total_commands()
	);
}

// ---------------------------------------------------------------------------
// Test 23: metrics track session commands
// ---------------------------------------------------------------------------

#[tokio::test]
async fn metrics_track_session() {
	let dir = tempfile::tempdir().unwrap();
	let shell = make_shell(dir.path());
	let backend = make_backend();

	let (shell, session) = shell.create_session(None, None, None).unwrap();
	let id = session.id.clone();

	let (shell, _) =
		run_in_session(shell, &backend, &id, "echo a", None, None, None).await;
	let (shell, _) =
		run_in_session(shell, &backend, &id, "echo b", None, None, None).await;

	// run_in_session increments total_commands via record_exec
	assert!(
		shell.total_commands() >= 2,
		"total_commands should be >= 2, got: {}",
		shell.total_commands()
	);
}
