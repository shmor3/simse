use std::time::Duration;

use russh::client::Msg;
use russh::{Channel, ChannelMsg};

use crate::error::SandboxError;

/// Output from executing a command over an SSH channel.
#[derive(Debug, Clone)]
pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<u32>,
}

/// Read all output from a channel until EOF or timeout.
///
/// Collects stdout (channel data) and stderr (extended data, ext=1)
/// into separate buffers, captures the exit code, and returns the
/// result. Truncates output at `max_bytes` per stream.
pub async fn read_channel_output(
    channel: &mut Channel<Msg>,
    timeout_ms: u64,
    max_bytes: usize,
) -> Result<ExecOutput, SandboxError> {
    let duration = Duration::from_millis(timeout_ms);

    let result = tokio::time::timeout(duration, read_until_eof(channel, max_bytes)).await;

    match result {
        Ok(output) => output,
        Err(_) => Err(SandboxError::Timeout(format!(
            "channel read timed out after {timeout_ms}ms"
        ))),
    }
}

/// Internal: read channel messages until EOF/Close.
async fn read_until_eof(
    channel: &mut Channel<Msg>,
    max_bytes: usize,
) -> Result<ExecOutput, SandboxError> {
    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let mut exit_code: Option<u32> = None;

    loop {
        match channel.wait().await {
            Some(ChannelMsg::Data { ref data }) => {
                let remaining = max_bytes.saturating_sub(stdout_buf.len());
                if remaining > 0 {
                    let take = remaining.min(data.len());
                    stdout_buf.extend_from_slice(&data[..take]);
                }
            }
            Some(ChannelMsg::ExtendedData { ref data, ext: 1 }) => {
                let remaining = max_bytes.saturating_sub(stderr_buf.len());
                if remaining > 0 {
                    let take = remaining.min(data.len());
                    stderr_buf.extend_from_slice(&data[..take]);
                }
            }
            Some(ChannelMsg::ExitStatus { exit_status }) => {
                exit_code = Some(exit_status);
            }
            Some(ChannelMsg::Eof | ChannelMsg::Close) => {
                break;
            }
            Some(_) => {
                // Ignore other channel messages (e.g., WindowAdjusted)
            }
            None => {
                // Channel dropped / sender closed
                break;
            }
        }
    }

    Ok(ExecOutput {
        stdout: String::from_utf8_lossy(&stdout_buf).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_buf).into_owned(),
        exit_code,
    })
}
