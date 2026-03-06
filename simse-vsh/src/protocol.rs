use serde::{Deserialize, Serialize};

// -- JSON-RPC 2.0 error codes ------------------------------------------------

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const VSH_ERROR: i32 = -32000;

// -- Incoming request ---------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
	pub id: u64,
	pub method: String,
	#[serde(default)]
	pub params: serde_json::Value,
}

// -- Initialize ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
	pub root_directory: String,
	pub shell: Option<String>,
	pub default_timeout_ms: Option<u64>,
	pub max_output_bytes: Option<usize>,
	pub max_sessions: Option<usize>,
	pub allowed_paths: Option<Vec<String>>,
	pub blocked_patterns: Option<Vec<String>>,
	#[allow(dead_code)]
	pub base_env: Option<std::collections::HashMap<String, String>>,
}

// -- Session ------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreateParams {
	pub name: Option<String>,
	pub cwd: Option<String>,
	pub env: Option<std::collections::HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdParams {
	pub session_id: String,
}

// -- Exec ---------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecRunParams {
	pub session_id: String,
	pub command: String,
	pub timeout_ms: Option<u64>,
	pub max_output_bytes: Option<usize>,
	pub stdin: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecRunRawParams {
	pub command: String,
	pub cwd: Option<String>,
	pub env: Option<std::collections::HashMap<String, String>>,
	pub timeout_ms: Option<u64>,
	pub max_output_bytes: Option<usize>,
	pub stdin: Option<String>,
	pub shell: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecGitParams {
	pub session_id: String,
	pub args: Vec<String>,
	pub timeout_ms: Option<u64>,
	pub max_output_bytes: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecScriptParams {
	pub session_id: String,
	pub script: String,
	pub timeout_ms: Option<u64>,
	pub max_output_bytes: Option<usize>,
}

// -- Env ----------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvSetParams {
	pub session_id: String,
	pub key: String,
	pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvGetParams {
	pub session_id: String,
	pub key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvDeleteParams {
	pub session_id: String,
	pub key: String,
}

// -- Shell --------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSetCwdParams {
	pub session_id: String,
	pub cwd: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSetAliasParams {
	pub session_id: String,
	pub name: String,
	pub command: String,
}

// -- Result types -------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecResultResponse {
	pub stdout: String,
	pub stderr: String,
	pub exit_code: i32,
	pub duration_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
	pub id: String,
	pub name: Option<String>,
	pub cwd: String,
	pub env: std::collections::HashMap<String, String>,
	pub aliases: std::collections::HashMap<String, String>,
	pub created_at: u64,
	pub last_active_at: u64,
	pub command_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListItem {
	pub id: String,
	pub name: Option<String>,
	pub cwd: String,
	pub command_count: usize,
	pub last_active_at: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryResponse {
	pub command: String,
	pub exit_code: i32,
	pub timestamp: u64,
	pub duration_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsResponse {
	pub session_count: usize,
	pub total_commands: u64,
	pub total_errors: u64,
}
