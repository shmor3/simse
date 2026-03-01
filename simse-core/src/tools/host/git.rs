//! Git Tools — 9 tools for git operations.
//!
//! Ports `src/ai/tools/host/git.ts` to Rust.
//! All commands run via `tokio::process::Command` in a configurable working
//! directory.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;

use crate::error::{SimseError, ToolErrorCode};
use crate::tools::registry::ToolRegistry;
use crate::tools::types::{
	ToolAnnotations, ToolCategory, ToolDefinition, ToolHandler, ToolParameter,
};

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Configuration for git tool registration.
pub struct GitToolOptions {
	/// The working directory for git commands.
	pub working_directory: PathBuf,
}

// ---------------------------------------------------------------------------
// Internal helper
// ---------------------------------------------------------------------------

/// Run a git command and return stdout on success or an error on failure.
async fn run_git(args: &[&str], cwd: &std::path::Path) -> Result<String, SimseError> {
	let output = tokio::process::Command::new("git")
		.args(args)
		.current_dir(cwd)
		.output()
		.await
		.map_err(|e| {
			SimseError::tool(
				ToolErrorCode::ExecutionFailed,
				format!("Failed to spawn git: {}", e),
			)
		})?;

	let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
	let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

	if !output.status.success() {
		let msg = if stderr.is_empty() {
			format!(
				"git {} failed with exit code {}",
				args.first().unwrap_or(&""),
				output.status.code().unwrap_or(-1)
			)
		} else {
			stderr
		};
		return Err(SimseError::tool(ToolErrorCode::ExecutionFailed, msg));
	}

	Ok(stdout)
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
// Public registration
// ---------------------------------------------------------------------------

/// Register 9 git tools on the given registry.
pub fn register_git_tools(registry: &mut ToolRegistry, options: GitToolOptions) {
	let wd = Arc::new(options.working_directory);

	// -------------------------------------------------------------------
	// 1. git_status
	// -------------------------------------------------------------------
	{
		let definition = ToolDefinition {
			name: "git_status".to_string(),
			description: "Show the working tree status of the git repository.".to_string(),
			parameters: HashMap::new(),
			category: ToolCategory::Read,
			annotations: Some(ToolAnnotations {
				read_only: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let wd = Arc::clone(&wd);
		let handler: ToolHandler = Arc::new(move |_args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move { run_git(&["status"], &wd).await })
		});

		registry.register(definition, handler);
	}

	// -------------------------------------------------------------------
	// 2. git_diff
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"staged".to_string(),
			param(
				"boolean",
				"If true, show staged changes (--cached)",
				false,
			),
		);
		parameters.insert(
			"path".to_string(),
			param("string", "Limit diff to a specific file path", false),
		);

		let definition = ToolDefinition {
			name: "git_diff".to_string(),
			description: "Show changes in the working tree or staging area. Use staged=true for staged changes.".to_string(),
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
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move {
				let mut git_args: Vec<String> = vec!["diff".to_string()];
				if args.get("staged").and_then(|v| v.as_bool()).unwrap_or(false) {
					git_args.push("--cached".to_string());
				}
				if let Some(path) = args.get("path").and_then(|v| v.as_str()) {
					if !path.is_empty() {
						git_args.push("--".to_string());
						git_args.push(path.to_string());
					}
				}
				let arg_refs: Vec<&str> = git_args.iter().map(|s| s.as_str()).collect();
				run_git(&arg_refs, &wd).await
			})
		});

		registry.register(definition, handler);
	}

	// -------------------------------------------------------------------
	// 3. git_log
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"count".to_string(),
			param("number", "Number of commits to show (default: 10)", false),
		);
		parameters.insert(
			"oneline".to_string(),
			param("boolean", "Use oneline format (default: true)", false),
		);

		let definition = ToolDefinition {
			name: "git_log".to_string(),
			description:
				"Show commit log history. Defaults to 10 commits in oneline format.".to_string(),
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
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move {
				let count = args
					.get("count")
					.and_then(|v| v.as_u64())
					.unwrap_or(10);
				let oneline = args
					.get("oneline")
					.and_then(|v| v.as_bool())
					.unwrap_or(true);

				let mut git_args: Vec<String> =
					vec!["log".to_string(), format!("-{}", count)];
				if oneline {
					git_args.push("--oneline".to_string());
				}
				let arg_refs: Vec<&str> = git_args.iter().map(|s| s.as_str()).collect();
				run_git(&arg_refs, &wd).await
			})
		});

		registry.register(definition, handler);
	}

	// -------------------------------------------------------------------
	// 4. git_commit
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"message".to_string(),
			param("string", "The commit message", true),
		);

		let definition = ToolDefinition {
			name: "git_commit".to_string(),
			description:
				"Create a git commit with the staged changes and the given message.".to_string(),
			parameters,
			category: ToolCategory::Execute,
			annotations: Some(ToolAnnotations {
				destructive: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let wd = Arc::clone(&wd);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move {
				let message = args
					.get("message")
					.and_then(|v| v.as_str())
					.unwrap_or("");

				if message.is_empty() {
					return Err(SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						"Commit message is required",
					));
				}

				run_git(&["commit", "-m", message], &wd).await
			})
		});

		registry.register(definition, handler);
	}

	// -------------------------------------------------------------------
	// 5. git_branch
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"name".to_string(),
			param("string", "Branch name to create or switch to", false),
		);
		parameters.insert(
			"create".to_string(),
			param(
				"boolean",
				"If true, create a new branch with the given name",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "git_branch".to_string(),
			description: "List, create, or switch branches. With no name, lists branches. With create=true, creates a new branch. Otherwise, switches to the named branch.".to_string(),
			parameters,
			category: ToolCategory::Execute,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let wd = Arc::clone(&wd);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move {
				let name = args
					.get("name")
					.and_then(|v| v.as_str())
					.unwrap_or("");
				let create = args
					.get("create")
					.and_then(|v| v.as_bool())
					.unwrap_or(false);

				if name.is_empty() {
					return run_git(&["branch"], &wd).await;
				}

				if create {
					return run_git(&["branch", name], &wd).await;
				}

				run_git(&["checkout", name], &wd).await
			})
		});

		registry.register(definition, handler);
	}

	// -------------------------------------------------------------------
	// 6. git_add
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"paths".to_string(),
			param(
				"string",
				"Space-separated file paths to stage (relative to working directory)",
				false,
			),
		);
		parameters.insert(
			"all".to_string(),
			param("boolean", "If true, stage all changes (git add -A)", false),
		);

		let definition = ToolDefinition {
			name: "git_add".to_string(),
			description: "Stage files for the next commit. Provide specific paths or use all=true to stage everything.".to_string(),
			parameters,
			category: ToolCategory::Execute,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let wd = Arc::clone(&wd);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move {
				if args
					.get("all")
					.and_then(|v| v.as_bool())
					.unwrap_or(false)
				{
					return run_git(&["add", "-A"], &wd).await;
				}

				let paths = args
					.get("paths")
					.and_then(|v| v.as_str())
					.unwrap_or("")
					.trim()
					.to_string();

				if paths.is_empty() {
					return Err(SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						"Either provide paths to stage or set all=true",
					));
				}

				let path_list: Vec<&str> = paths.split_whitespace().collect();
				let mut git_args: Vec<&str> = vec!["add"];
				git_args.extend(path_list);
				run_git(&git_args, &wd).await
			})
		});

		registry.register(definition, handler);
	}

	// -------------------------------------------------------------------
	// 7. git_stash
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"action".to_string(),
			param(
				"string",
				"Stash action: save, pop, apply, or list (default: save)",
				false,
			),
		);
		parameters.insert(
			"message".to_string(),
			param(
				"string",
				"Optional message for the stash (only for save)",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "git_stash".to_string(),
			description:
				"Save, apply, pop, or list stashed changes. Default action is save.".to_string(),
			parameters,
			category: ToolCategory::Execute,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let wd = Arc::clone(&wd);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move {
				let action = args
					.get("action")
					.and_then(|v| v.as_str())
					.unwrap_or("save");

				match action {
					"save" => {
						let message = args
							.get("message")
							.and_then(|v| v.as_str())
							.unwrap_or("");
						if message.is_empty() {
							run_git(&["stash"], &wd).await
						} else {
							run_git(&["stash", "-m", message], &wd).await
						}
					}
					"pop" => run_git(&["stash", "pop"], &wd).await,
					"apply" => run_git(&["stash", "apply"], &wd).await,
					"list" => run_git(&["stash", "list"], &wd).await,
					_ => Err(SimseError::tool(
						ToolErrorCode::ExecutionFailed,
						format!(
							"Unknown stash action: \"{}\". Use save, pop, apply, or list.",
							action
						),
					)),
				}
			})
		});

		registry.register(definition, handler);
	}

	// -------------------------------------------------------------------
	// 8. git_push
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"remote".to_string(),
			param("string", "Remote name (default: origin)", false),
		);
		parameters.insert(
			"branch".to_string(),
			param("string", "Branch to push (default: current branch)", false),
		);
		parameters.insert(
			"setUpstream".to_string(),
			param(
				"boolean",
				"If true, set upstream tracking (-u flag)",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "git_push".to_string(),
			description: "Push commits to a remote repository. Defaults to origin.".to_string(),
			parameters,
			category: ToolCategory::Execute,
			annotations: Some(ToolAnnotations {
				destructive: Some(true),
				..Default::default()
			}),
			timeout_ms: None,
			max_output_chars: None,
		};

		let wd = Arc::clone(&wd);
		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move {
				let remote = args
					.get("remote")
					.and_then(|v| v.as_str())
					.unwrap_or("origin");

				let mut git_args: Vec<String> = vec!["push".to_string()];

				if args
					.get("setUpstream")
					.and_then(|v| v.as_bool())
					.unwrap_or(false)
				{
					git_args.push("-u".to_string());
				}

				git_args.push(remote.to_string());

				if let Some(branch) = args.get("branch").and_then(|v| v.as_str()) {
					if !branch.is_empty() {
						git_args.push(branch.to_string());
					}
				}

				let arg_refs: Vec<&str> = git_args.iter().map(|s| s.as_str()).collect();
				run_git(&arg_refs, &wd).await
			})
		});

		registry.register(definition, handler);
	}

	// -------------------------------------------------------------------
	// 9. git_pull
	// -------------------------------------------------------------------
	{
		let mut parameters = HashMap::new();
		parameters.insert(
			"remote".to_string(),
			param("string", "Remote name (default: origin)", false),
		);
		parameters.insert(
			"branch".to_string(),
			param(
				"string",
				"Branch to pull (default: current branch tracking)",
				false,
			),
		);

		let definition = ToolDefinition {
			name: "git_pull".to_string(),
			description: "Pull changes from a remote repository. Defaults to origin.".to_string(),
			parameters,
			category: ToolCategory::Execute,
			annotations: None,
			timeout_ms: None,
			max_output_chars: None,
		};

		let handler: ToolHandler = Arc::new(move |args: Value| {
			let wd = Arc::clone(&wd);
			Box::pin(async move {
				let remote = args
					.get("remote")
					.and_then(|v| v.as_str())
					.unwrap_or("origin");

				let mut git_args: Vec<String> = vec!["pull".to_string(), remote.to_string()];

				if let Some(branch) = args.get("branch").and_then(|v| v.as_str()) {
					if !branch.is_empty() {
						git_args.push(branch.to_string());
					}
				}

				let arg_refs: Vec<&str> = git_args.iter().map(|s| s.as_str()).collect();
				run_git(&arg_refs, &wd).await
			})
		});

		registry.register(definition, handler);
	}
}
