use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use crate::error::RemoteError;

/// Routes JSON-RPC requests to a local simse-core process.
///
/// This struct holds I/O handles (child process, pipe reader) that are
/// inherently mutable and non-clonable. Methods that perform I/O use
/// `&mut self` with `// PERF: async I/O` annotations.
pub struct LocalRouter {
    child: Option<Child>,
    reader: Option<BufReader<std::process::ChildStdout>>,
}

impl Default for LocalRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalRouter {
    pub fn new() -> Self {
        Self {
            child: None,
            reader: None,
        }
    }

    /// Spawn a local simse-core-engine process.
    pub fn spawn(&mut self, binary_path: &str) -> Result<(), RemoteError> {
        // PERF: async I/O — spawns child process with piped stdin/stdout
        let mut child = Command::new(binary_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                RemoteError::ConnectionFailed(format!(
                    "Failed to spawn {binary_path}: {e}"
                ))
            })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            RemoteError::ConnectionFailed("No stdout from child process".into())
        })?;
        self.reader = Some(BufReader::new(stdout));
        self.child = Some(child);
        tracing::info!("Local simse-core spawned: {binary_path}");
        Ok(())
    }

    /// Forward a raw JSON-RPC request string to the local process and return the response.
    pub fn forward(&mut self, request: &str) -> Result<String, RemoteError> {
        // PERF: async I/O — writes to child stdin, reads from child stdout
        let child = self
            .child
            .as_mut()
            .ok_or(RemoteError::NotInitialized)?;
        let reader = self
            .reader
            .as_mut()
            .ok_or(RemoteError::NotInitialized)?;

        let stdin = child.stdin.as_mut().ok_or(RemoteError::NotInitialized)?;

        // Write request to child stdin
        stdin
            .write_all(request.as_bytes())
            .map_err(RemoteError::Io)?;
        if !request.ends_with('\n') {
            stdin.write_all(b"\n").map_err(RemoteError::Io)?;
        }
        stdin.flush().map_err(RemoteError::Io)?;

        // Read response line
        let mut line = String::new();
        loop {
            line.clear();
            let n = reader.read_line(&mut line).map_err(RemoteError::Io)?;
            if n == 0 {
                return Err(RemoteError::ConnectionFailed(
                    "Child process closed stdout".into(),
                ));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Skip notifications (no id field)
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                if parsed.get("id").is_some() {
                    return Ok(trimmed.to_string());
                }
            }
        }
    }

    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    /// Stop the local process.
    pub fn stop(&mut self) {
        // PERF: async I/O — closes child stdin, waits for process exit
        if let Some(mut child) = self.child.take() {
            drop(child.stdin.take());
            let _ = child.wait();
        }
        self.reader = None;
        tracing::info!("Local simse-core stopped");
    }
}

impl Drop for LocalRouter {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_router_is_not_running() {
        let router = LocalRouter::new();
        assert!(!router.is_running());
    }

    #[test]
    fn forward_fails_when_not_spawned() {
        let mut router = LocalRouter::new();
        let result = router.forward(r#"{"id":1,"method":"health"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn spawn_fails_with_bad_binary() {
        let mut router = LocalRouter::new();
        let result = router.spawn("/nonexistent/binary");
        assert!(result.is_err());
    }
}
