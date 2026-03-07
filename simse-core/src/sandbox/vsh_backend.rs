use std::collections::HashMap;
use std::path::Path;

use crate::sandbox::error::SandboxError;
use crate::sandbox::ssh::shell::SshShell;
use crate::sandbox::vsh_executor::ExecResult;

/// Marker struct for local shell execution (delegates to `crate::vsh_executor`).
pub struct LocalShell;

/// Unified shell backend — dispatches to local OS process or SSH.
pub enum ShellImpl {
    Local(LocalShell),
    Ssh(SshShell),
}

impl ShellImpl {
    #[allow(clippy::too_many_arguments)]
    pub async fn execute_command(
        &self,
        command: &str,
        cwd: &Path,
        env: &HashMap<String, String>,
        shell: &str,
        timeout_ms: u64,
        max_output_bytes: usize,
        stdin_input: Option<&str>,
    ) -> Result<ExecResult, SandboxError> {
        match self {
            Self::Local(_) => {
                crate::sandbox::vsh_executor::execute_command(
                    command,
                    cwd,
                    env,
                    shell,
                    timeout_ms,
                    max_output_bytes,
                    stdin_input,
                )
                .await
            }
            Self::Ssh(ssh) => {
                ssh.execute_command(
                    command,
                    cwd,
                    env,
                    shell,
                    timeout_ms,
                    max_output_bytes,
                    stdin_input,
                )
                .await
            }
        }
    }

    pub async fn execute_git(
        &self,
        args: &[String],
        cwd: &Path,
        env: &HashMap<String, String>,
        timeout_ms: u64,
        max_output_bytes: usize,
    ) -> Result<ExecResult, SandboxError> {
        match self {
            Self::Local(_) => {
                crate::sandbox::vsh_executor::execute_git(args, cwd, env, timeout_ms, max_output_bytes)
                    .await
            }
            Self::Ssh(ssh) => {
                ssh.execute_git(args, cwd, env, timeout_ms, max_output_bytes)
                    .await
            }
        }
    }
}
