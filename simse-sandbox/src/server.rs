use std::collections::HashMap;

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::vfs_diff::DiffOutput;
use crate::vfs_disk::{DiskSearchMode, DiskSearchOptions, DiskSearchResult};
use crate::vfs_store::{
    DiffResultOutput, SearchOpts, SearchOutput, SnapshotData as VfsSnapshotData,
    SnapshotDir as VfsSnapshotDir, SnapshotFile as VfsSnapshotFile,
    TransactionOp as VfsTransactionOp, VfsEvent,
};
use crate::vnet_mock_store::MockResponse;
use crate::vnet_session::{Scheme as VnetScheme, SessionType as VnetSessionType};

use crate::config::BackendConfig;
use crate::error::SandboxError;
use crate::protocol::*;
use crate::sandbox::{InitConfig, Sandbox, VfsInitConfig, VnetInitConfig, VshInitConfig};
use crate::transport::NdjsonTransport;

// ── Scheme detection (VFS) ──────────────────────────────────────────────────

enum VfsScheme {
    Vfs,
    Disk,
}

fn detect_vfs_scheme(params: &serde_json::Value) -> VfsScheme {
    if let Some(path) = params.get("path").and_then(|v| v.as_str()) {
        if path.starts_with("file://") {
            return VfsScheme::Disk;
        }
    }
    VfsScheme::Vfs
}

fn detect_vfs_scheme_dual(
    params: &serde_json::Value,
    key1: &str,
    key2: &str,
) -> Result<VfsScheme, SandboxError> {
    let path1 = params.get(key1).and_then(|v| v.as_str()).unwrap_or("");
    let path2 = params.get(key2).and_then(|v| v.as_str()).unwrap_or("");
    let is_disk1 = path1.starts_with("file://");
    let is_disk2 = path2.starts_with("file://");
    match (is_disk1, is_disk2) {
        (false, false) => Ok(VfsScheme::Vfs),
        (true, true) => Ok(VfsScheme::Disk),
        _ => Err(SandboxError::InvalidParams(
            "Cannot mix vfs:// and file:// paths".into(),
        )),
    }
}

// ── Parse helper ────────────────────────────────────────────────────────────

fn parse_params<T: serde::de::DeserializeOwned>(
    params: serde_json::Value,
) -> Result<T, SandboxError> {
    serde_json::from_value(params).map_err(|e| SandboxError::InvalidParams(e.to_string()))
}

// ── Param types ─────────────────────────────────────────────────────────────
//
// We re-define lightweight param structs here rather than importing from each
// engine crate, since the engine protocol types have their own JsonRpcRequest
// and other baggage. camelCase rename ensures JSON compatibility.

// VFS params

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadFileParams {
    path: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteFileParams {
    path: String,
    content: String,
    content_type: Option<String>,
    create_parents: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppendFileParams {
    path: String,
    content: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathParams {
    path: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OptionalPathParams {
    path: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MkdirParams {
    path: String,
    recursive: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReaddirParams {
    path: String,
    recursive: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RmdirParams {
    path: String,
    recursive: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameParams {
    old_path: String,
    new_path: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopyParams {
    src: String,
    dest: String,
    overwrite: Option<bool>,
    recursive: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlobParams {
    pattern: serde_json::Value,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParams {
    query: String,
    glob: Option<String>,
    max_results: Option<usize>,
    mode: Option<String>,
    context_before: Option<usize>,
    context_after: Option<usize>,
    count_only: Option<bool>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiffParams {
    old_path: String,
    new_path: String,
    context: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiffVersionsParams {
    path: String,
    old_version: usize,
    new_version: Option<usize>,
    context: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckoutParams {
    path: String,
    version: usize,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotFileProto {
    path: String,
    content_type: String,
    text: Option<String>,
    base64: Option<String>,
    created_at: u64,
    modified_at: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotDirProto {
    path: String,
    created_at: u64,
    modified_at: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotDataProto {
    files: Vec<SnapshotFileProto>,
    directories: Vec<SnapshotDirProto>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestoreParams {
    snapshot: SnapshotDataProto,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransactionParams {
    ops: Vec<TransactionOp>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum TransactionOp {
    #[serde(rename = "writeFile")]
    WriteFile { path: String, content: String },
    #[serde(rename = "deleteFile")]
    DeleteFile { path: String },
    #[serde(rename = "mkdir")]
    Mkdir { path: String },
    #[serde(rename = "rmdir")]
    Rmdir { path: String },
    #[serde(rename = "rename")]
    Rename {
        old_path: String,
        new_path: String,
    },
    #[serde(rename = "copy")]
    Copy { src: String, dest: String },
}

// VSH params

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionCreateParams {
    name: Option<String>,
    cwd: Option<String>,
    env: Option<HashMap<String, String>>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionIdParams {
    session_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecRunParams {
    session_id: String,
    command: String,
    timeout_ms: Option<u64>,
    max_output_bytes: Option<usize>,
    stdin: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecRunRawParams {
    command: String,
    cwd: Option<String>,
    env: Option<HashMap<String, String>>,
    timeout_ms: Option<u64>,
    max_output_bytes: Option<usize>,
    stdin: Option<String>,
    shell: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecGitParams {
    session_id: String,
    args: Vec<String>,
    timeout_ms: Option<u64>,
    max_output_bytes: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecScriptParams {
    session_id: String,
    script: String,
    timeout_ms: Option<u64>,
    max_output_bytes: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnvSetParams {
    session_id: String,
    key: String,
    value: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnvGetParams {
    session_id: String,
    key: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnvDeleteParams {
    session_id: String,
    key: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShellSetCwdParams {
    session_id: String,
    cwd: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShellSetAliasParams {
    session_id: String,
    name: String,
    command: String,
}

// VNet params

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HttpRequestParams {
    url: String,
    method: Option<String>,
    headers: Option<HashMap<String, String>>,
    body: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WsConnectParams {
    url: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct WsMessageParams {
    session_id: String,
    data: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VnetSessionIdParam {
    session_id: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TcpConnectParams {
    host: String,
    port: u16,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TcpSendParams {
    session_id: String,
    data: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UdpSendParams {
    host: String,
    port: u16,
    #[allow(dead_code)]
    data: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveParams {
    hostname: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MockRegisterParams {
    method: Option<String>,
    url_pattern: String,
    response: MockResponseParam,
    times: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MockResponseParam {
    status: u16,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(default)]
    body: String,
    #[serde(default = "default_body_type")]
    body_type: String,
    delay_ms: Option<u64>,
}

fn default_body_type() -> String {
    "text".into()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MockIdParam {
    id: String,
}

// ── Initialize params ───────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SandboxInitializeParams {
    backend: Option<BackendParams>,
    vfs: Option<VfsInitParams>,
    vsh: Option<VshInitParams>,
    vnet: Option<VnetInitParams>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VfsInitParams {
    root_directory: Option<String>,
    allowed_paths: Option<Vec<String>>,
    max_history: Option<usize>,
    max_file_size: Option<u64>,
    max_total_size: Option<u64>,
    max_path_depth: Option<usize>,
    max_name_length: Option<usize>,
    max_node_count: Option<usize>,
    max_path_length: Option<usize>,
    max_diff_lines: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VshInitParams {
    root_directory: Option<String>,
    allowed_paths: Option<Vec<String>>,
    blocked_patterns: Option<Vec<String>>,
    shell: Option<String>,
    default_timeout_ms: Option<u64>,
    max_output_bytes: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct VnetInitParams {
    allowed_hosts: Option<Vec<String>>,
    allowed_ports: Option<Vec<PortRangeParam>>,
    allowed_protocols: Option<Vec<String>>,
    default_timeout_ms: Option<u64>,
    max_response_bytes: Option<u64>,
    max_connections: Option<usize>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortRangeParam {
    start: u16,
    end: u16,
}

// ── SandboxServer ───────────────────────────────────────────────────────────

/// JSON-RPC server for the unified sandbox.
///
/// Reads NDJSON requests from stdin, dispatches them to the appropriate engine
/// through the [`Sandbox`] orchestrator, and writes responses to stdout.
pub struct SandboxServer {
    transport: NdjsonTransport,
    sandbox: Sandbox,
}

impl SandboxServer {
    /// Create a new server with the given transport.
    pub fn new(transport: NdjsonTransport) -> Self {
        Self {
            transport,
            sandbox: Sandbox::new(),
        }
    }

    /// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
    pub async fn run(&mut self) {
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
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
    }

    // ── Response helpers ────────────────────────────────────────────────

    fn write_result(&self, id: u64, result: Result<serde_json::Value, SandboxError>) {
        match result {
            Ok(value) => self.transport.write_response(id, value),
            Err(e) => self.transport.write_error(
                id,
                SANDBOX_ERROR,
                e.to_string(),
                Some(e.to_json_rpc_error()),
            ),
        }
    }

    // ── Dispatch ────────────────────────────────────────────────────────

    async fn dispatch(&mut self, req: JsonRpcRequest) {
        let id = req.id;
        let method = req.method.as_str();

        // Route based on method prefix
        let result = match method {
            // Sandbox lifecycle
            "sandbox/initialize" => self.handle_initialize(req.params).await,
            "sandbox/dispose" => self.handle_dispose().await,
            "sandbox/health" => self.handle_health(),
            "sandbox/switchBackend" => self.handle_switch_backend(req.params).await,

            // VFS methods (sandbox/vfs/*)
            m if m.starts_with("sandbox/vfs/") => {
                let vfs_method = &m["sandbox/".len()..]; // strip "sandbox/" to get "vfs/*"
                let result = self.dispatch_vfs(vfs_method, req.params).await;
                // Drain VFS events after every VFS dispatch
                self.drain_vfs_events();
                self.write_result(id, result);
                return;
            }

            // VSH methods (sandbox/session/*, sandbox/exec/*, sandbox/env/*, sandbox/shell/*)
            m if m.starts_with("sandbox/session/")
                || m.starts_with("sandbox/exec/")
                || m.starts_with("sandbox/env/")
                || m.starts_with("sandbox/shell/") =>
            {
                let vsh_method = &m["sandbox/".len()..]; // strip "sandbox/" to get e.g. "session/create"
                self.dispatch_vsh(vsh_method, req.params).await
            }

            // VNet methods (sandbox/net/*, sandbox/mock/*)
            m if m.starts_with("sandbox/net/") || m.starts_with("sandbox/mock/") => {
                let vnet_method = &m["sandbox/".len()..]; // strip "sandbox/" to get e.g. "net/httpRequest"
                self.dispatch_vnet(vnet_method, req.params).await
            }

            // VNet session methods (sandbox/netSession/*)
            m if m.starts_with("sandbox/netSession/") => {
                let sub = &m["sandbox/netSession/".len()..];
                self.dispatch_vnet_session(sub, req.params)
            }

            _ => {
                self.transport.write_error(
                    id,
                    METHOD_NOT_FOUND,
                    format!("Unknown method: {}", method),
                    None,
                );
                return;
            }
        };

        self.write_result(id, result);
    }

    // ── VFS event drain ─────────────────────────────────────────────────

    fn drain_vfs_events(&mut self) {
        if let Ok(vfs) = self.sandbox.vfs_take() {
            let (vfs, events) = vfs.drain_events();
            self.sandbox.vfs_set(vfs);
            for event in events {
                let params = match &event {
                    VfsEvent::Write {
                        path,
                        size,
                        content_type,
                        is_new,
                    } => {
                        serde_json::json!({
                            "type": "write",
                            "path": path,
                            "size": size,
                            "contentType": content_type,
                            "isNew": is_new
                        })
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
                self.transport
                    .write_notification("sandbox/vfs/event", params);
            }
        }
    }

    // ── Sandbox lifecycle handlers ──────────────────────────────────────

    async fn handle_initialize(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SandboxInitializeParams = parse_params(params)?;

        let backend = match p.backend {
            Some(ref bp) => BackendConfig::from_params(bp)?,
            None => BackendConfig::Local,
        };

        let vfs_config = p.vfs.map(|v| {
            let mut limits = crate::vfs_path::VfsLimits::default();
            if let Some(val) = v.max_file_size {
                limits.max_file_size = val;
            }
            if let Some(val) = v.max_total_size {
                limits.max_total_size = val;
            }
            if let Some(val) = v.max_path_depth {
                limits.max_path_depth = val;
            }
            if let Some(val) = v.max_name_length {
                limits.max_name_length = val;
            }
            if let Some(val) = v.max_node_count {
                limits.max_node_count = val;
            }
            if let Some(val) = v.max_path_length {
                limits.max_path_length = val;
            }
            if let Some(val) = v.max_diff_lines {
                limits.max_diff_lines = val;
            }
            VfsInitConfig {
                root_directory: v.root_directory.unwrap_or_else(|| ".".to_string()),
                allowed_paths: v.allowed_paths.unwrap_or_default(),
                max_history: v.max_history.unwrap_or(50),
                limits,
            }
        });

        let vsh_config = p.vsh.map(|v| VshInitConfig {
            root_directory: v.root_directory.unwrap_or_else(|| ".".to_string()),
            allowed_paths: v.allowed_paths.unwrap_or_default(),
            blocked_patterns: v.blocked_patterns.unwrap_or_default(),
            shell: v.shell.unwrap_or_else(|| "sh".to_string()),
            default_timeout_ms: v.default_timeout_ms.unwrap_or(120_000),
            max_output_bytes: v.max_output_bytes.unwrap_or(50_000),
        });

        let vnet_config = p.vnet.map(|v| VnetInitConfig {
            allowed_hosts: v
                .allowed_hosts
                .unwrap_or_else(|| vec!["*".to_string()]),
            allowed_ports: v
                .allowed_ports
                .unwrap_or_default()
                .into_iter()
                .map(|pr| (pr.start, pr.end))
                .collect(),
            allowed_protocols: v.allowed_protocols.unwrap_or_else(|| {
                vec![
                    "http".to_string(),
                    "https".to_string(),
                    "ws".to_string(),
                    "wss".to_string(),
                    "tcp".to_string(),
                    "udp".to_string(),
                ]
            }),
            default_timeout_ms: v.default_timeout_ms.unwrap_or(30_000),
            max_response_bytes: v.max_response_bytes.unwrap_or(10 * 1024 * 1024),
            max_connections: v.max_connections.unwrap_or(100),
        });

        let init_config = InitConfig {
            backend,
            vfs: vfs_config,
            vsh: vsh_config,
            vnet: vnet_config,
        };

        self.sandbox.initialize(init_config).await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    async fn handle_dispose(&mut self) -> Result<serde_json::Value, SandboxError> {
        self.sandbox.dispose().await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    fn handle_health(&self) -> Result<serde_json::Value, SandboxError> {
        self.sandbox.health()
    }

    async fn handle_switch_backend(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SwitchBackendParams = parse_params(params)?;
        let config = BackendConfig::from_params(&p.backend)?;
        self.sandbox.switch_backend(config).await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    // ════════════════════════════════════════════════════════════════════
    // VFS dispatch
    // ════════════════════════════════════════════════════════════════════

    async fn dispatch_vfs(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        match method {
            // Dual-backend methods (scheme detection)
            "vfs/readFile" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_read_file(params),
                VfsScheme::Disk => self.disk_read_file(params).await,
            },
            "vfs/writeFile" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_write_file(params),
                VfsScheme::Disk => self.disk_write_file(params).await,
            },
            "vfs/appendFile" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_append_file(params),
                VfsScheme::Disk => self.disk_append_file(params).await,
            },
            "vfs/deleteFile" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_delete_file(params),
                VfsScheme::Disk => self.disk_delete_file(params).await,
            },
            "vfs/mkdir" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_mkdir(params),
                VfsScheme::Disk => self.disk_mkdir(params).await,
            },
            "vfs/readdir" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_readdir(params),
                VfsScheme::Disk => self.disk_readdir(params).await,
            },
            "vfs/rmdir" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_rmdir(params),
                VfsScheme::Disk => self.disk_rmdir(params).await,
            },
            "vfs/stat" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_stat(params),
                VfsScheme::Disk => self.disk_stat(params).await,
            },
            "vfs/exists" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_exists(params),
                VfsScheme::Disk => self.disk_exists(params).await,
            },
            "vfs/rename" => match detect_vfs_scheme_dual(&params, "oldPath", "newPath")? {
                VfsScheme::Vfs => self.vfs_rename(params),
                VfsScheme::Disk => self.disk_rename(params).await,
            },
            "vfs/copy" => match detect_vfs_scheme_dual(&params, "src", "dest")? {
                VfsScheme::Vfs => self.vfs_copy(params),
                VfsScheme::Disk => self.disk_copy(params).await,
            },
            "vfs/glob" => {
                let scheme = if let Some(pattern) = params.get("pattern") {
                    let first = match pattern {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Array(arr) => arr
                            .first()
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        _ => String::new(),
                    };
                    if first.starts_with("file://") {
                        VfsScheme::Disk
                    } else {
                        VfsScheme::Vfs
                    }
                } else {
                    VfsScheme::Vfs
                };
                match scheme {
                    VfsScheme::Vfs => self.vfs_glob(params),
                    VfsScheme::Disk => self.disk_glob(params).await,
                }
            }
            "vfs/tree" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_tree(params),
                VfsScheme::Disk => self.disk_tree(params).await,
            },
            "vfs/du" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_du(params),
                VfsScheme::Disk => self.disk_du(params).await,
            },
            "vfs/search" => {
                let scheme = if let Some(glob) = params.get("glob").and_then(|v| v.as_str()) {
                    if glob.starts_with("file://") {
                        VfsScheme::Disk
                    } else {
                        VfsScheme::Vfs
                    }
                } else {
                    VfsScheme::Vfs
                };
                match scheme {
                    VfsScheme::Vfs => self.vfs_search(params),
                    VfsScheme::Disk => self.disk_search(params).await,
                }
            }
            "vfs/history" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_history(params),
                VfsScheme::Disk => self.disk_history(params).await,
            },
            "vfs/diff" => match detect_vfs_scheme_dual(&params, "oldPath", "newPath")? {
                VfsScheme::Vfs => self.vfs_diff(params),
                VfsScheme::Disk => self.disk_diff(params).await,
            },
            "vfs/diffVersions" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_diff_versions(params),
                VfsScheme::Disk => self.disk_diff_versions(params).await,
            },
            "vfs/checkout" => match detect_vfs_scheme(&params) {
                VfsScheme::Vfs => self.vfs_checkout(params),
                VfsScheme::Disk => self.disk_checkout(params).await,
            },

            // VFS-only methods
            "vfs/snapshot" => self.vfs_snapshot(),
            "vfs/restore" => self.vfs_restore(params),
            "vfs/clear" => self.vfs_clear(),
            "vfs/transaction" => self.vfs_transaction(params),
            "vfs/metrics" => self.vfs_metrics(),

            _ => Err(SandboxError::InvalidParams(format!(
                "Unknown VFS method: {}",
                method
            ))),
        }
    }

    // ── VFS in-memory handlers ──────────────────────────────────────────

    fn vfs_read_file(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ReadFileParams = parse_params(params)?;
        let vfs = self.sandbox.vfs()?;
        let r = vfs.read_file(&p.path)?;
        Ok(serde_json::json!({
            "contentType": r.content_type,
            "text": r.text,
            "data": r.data_base64,
            "size": r.size,
        }))
    }

    fn vfs_write_file(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: WriteFileParams = parse_params(params)?;
        let vfs = self.sandbox.vfs_take()?;
        let vfs = vfs.write_file(
            &p.path,
            &p.content,
            p.content_type.as_deref(),
            p.create_parents.unwrap_or(false),
        )?;
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vfs_append_file(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: AppendFileParams = parse_params(params)?;
        let vfs = self.sandbox.vfs_take()?;
        let vfs = vfs.append_file(&p.path, &p.content)?;
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vfs_delete_file(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let vfs = self.sandbox.vfs_take()?;
        let (vfs, deleted) = vfs.delete_file(&p.path)?;
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "deleted": deleted }))
    }

    fn vfs_mkdir(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: MkdirParams = parse_params(params)?;
        let vfs = self.sandbox.vfs_take()?;
        let vfs = vfs.mkdir(&p.path, p.recursive.unwrap_or(false))?;
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vfs_readdir(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ReaddirParams = parse_params(params)?;
        let vfs = self.sandbox.vfs()?;
        let entries = vfs.readdir(&p.path, p.recursive.unwrap_or(false))?;
        let result: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|e| {
                serde_json::json!({
                    "name": e.name,
                    "type": e.node_type,
                })
            })
            .collect();
        Ok(serde_json::json!({ "entries": result }))
    }

    fn vfs_rmdir(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: RmdirParams = parse_params(params)?;
        let vfs = self.sandbox.vfs_take()?;
        let (vfs, deleted) = vfs.rmdir(&p.path, p.recursive.unwrap_or(false))?;
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "deleted": deleted }))
    }

    fn vfs_stat(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let vfs = self.sandbox.vfs()?;
        let s = vfs.stat(&p.path)?;
        Ok(serde_json::json!({
            "path": s.path,
            "type": s.node_type,
            "size": s.size,
            "createdAt": s.created_at,
            "modifiedAt": s.modified_at,
        }))
    }

    fn vfs_exists(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let vfs = self.sandbox.vfs()?;
        let exists = vfs.exists(&p.path)?;
        Ok(serde_json::json!({ "exists": exists }))
    }

    fn vfs_rename(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: RenameParams = parse_params(params)?;
        let vfs = self.sandbox.vfs_take()?;
        let vfs = vfs.rename(&p.old_path, &p.new_path)?;
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vfs_copy(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: CopyParams = parse_params(params)?;
        let vfs = self.sandbox.vfs_take()?;
        let vfs = vfs.copy(
            &p.src,
            &p.dest,
            p.overwrite.unwrap_or(false),
            p.recursive.unwrap_or(false),
        )?;
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vfs_glob(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: GlobParams = parse_params(params)?;
        let patterns: Vec<String> = match p.pattern {
            serde_json::Value::String(s) => vec![s],
            serde_json::Value::Array(arr) => arr
                .into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => {
                return Err(SandboxError::InvalidParams(
                    "pattern must be string or array".into(),
                ))
            }
        };
        let vfs = self.sandbox.vfs()?;
        let results = vfs.glob(patterns);
        Ok(serde_json::json!({ "matches": results }))
    }

    fn vfs_tree(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: OptionalPathParams = parse_params(params)?;
        let vfs = self.sandbox.vfs()?;
        let tree = vfs.tree(p.path.as_deref())?;
        Ok(serde_json::json!({ "tree": tree }))
    }

    fn vfs_du(&self, params: serde_json::Value) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let vfs = self.sandbox.vfs()?;
        let size = vfs.du(&p.path)?;
        Ok(serde_json::json!({ "size": size }))
    }

    fn vfs_search(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SearchParams = parse_params(params)?;
        let opts = SearchOpts {
            glob: p.glob,
            max_results: p.max_results.unwrap_or(100),
            mode: p.mode.unwrap_or_else(|| "substring".to_string()),
            context_before: p.context_before.unwrap_or(0),
            context_after: p.context_after.unwrap_or(0),
            count_only: p.count_only.unwrap_or(false),
        };
        let vfs = self.sandbox.vfs()?;
        let output = vfs.search(&p.query, opts)?;
        match output {
            SearchOutput::Results(matches) => {
                let results: Vec<serde_json::Value> = matches
                    .into_iter()
                    .map(|m| {
                        serde_json::json!({
                            "path": m.path,
                            "line": m.line,
                            "column": m.column,
                            "match": m.match_text,
                            "contextBefore": m.context_before,
                            "contextAfter": m.context_after,
                        })
                    })
                    .collect();
                Ok(serde_json::json!({ "results": results }))
            }
            SearchOutput::Count(count) => Ok(serde_json::json!({ "count": count })),
        }
    }

    fn vfs_history(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let vfs = self.sandbox.vfs()?;
        let entries = vfs.history(&p.path)?;
        let result: Vec<serde_json::Value> = entries
            .into_iter()
            .map(|e| {
                serde_json::json!({
                    "version": e.version,
                    "contentType": e.content_type,
                    "text": e.text,
                    "base64": e.base64,
                    "size": e.size,
                    "timestamp": e.timestamp,
                })
            })
            .collect();
        Ok(serde_json::json!({ "entries": result }))
    }

    fn vfs_diff(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: DiffParams = parse_params(params)?;
        let vfs = self.sandbox.vfs()?;
        let d = vfs.diff(&p.old_path, &p.new_path, p.context.unwrap_or(3))?;
        Ok(serde_json::to_value(convert_diff_output(d))?)
    }

    fn vfs_diff_versions(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: DiffVersionsParams = parse_params(params)?;
        let vfs = self.sandbox.vfs()?;
        let d = vfs.diff_versions(
            &p.path,
            p.old_version,
            p.new_version,
            p.context.unwrap_or(3),
        )?;
        Ok(serde_json::to_value(convert_diff_output(d))?)
    }

    fn vfs_checkout(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: CheckoutParams = parse_params(params)?;
        let vfs = self.sandbox.vfs_take()?;
        let vfs = vfs.checkout(&p.path, p.version)?;
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vfs_snapshot(&self) -> Result<serde_json::Value, SandboxError> {
        let vfs = self.sandbox.vfs()?;
        let snap = vfs.snapshot();
        let proto = SnapshotDataProto {
            files: snap
                .files
                .into_iter()
                .map(|f| SnapshotFileProto {
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
                .map(|d| SnapshotDirProto {
                    path: d.path,
                    created_at: d.created_at,
                    modified_at: d.modified_at,
                })
                .collect(),
        };
        Ok(serde_json::to_value(proto)?)
    }

    fn vfs_restore(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let wrapper: RestoreParams = parse_params(params)?;
        let snap = wrapper.snapshot;
        let vfs_snap = VfsSnapshotData {
            files: snap
                .files
                .into_iter()
                .map(|f| VfsSnapshotFile {
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
                .map(|d| VfsSnapshotDir {
                    path: d.path,
                    created_at: d.created_at,
                    modified_at: d.modified_at,
                })
                .collect(),
        };
        let vfs = self.sandbox.vfs_take()?;
        let vfs = vfs.restore(vfs_snap)?;
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vfs_clear(&mut self) -> Result<serde_json::Value, SandboxError> {
        let vfs = self.sandbox.vfs_take()?;
        let vfs = vfs.clear();
        self.sandbox.vfs_set(vfs);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vfs_transaction(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
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
        let vfs = self.sandbox.vfs_take()?;
        match vfs.transaction(ops) {
            Ok(vfs) => {
                self.sandbox.vfs_set(vfs);
                Ok(serde_json::json!({ "ok": true }))
            }
            Err((vfs, e)) => {
                self.sandbox.vfs_set(vfs);
                Err(e)
            }
        }
    }

    fn vfs_metrics(&self) -> Result<serde_json::Value, SandboxError> {
        let vfs = self.sandbox.vfs()?;
        let m = vfs.metrics();
        Ok(serde_json::json!({
            "totalSize": m.total_size,
            "nodeCount": m.node_count,
            "fileCount": m.file_count,
            "directoryCount": m.directory_count,
        }))
    }

    // ── VFS disk backend handlers ───────────────────────────────────────

    async fn disk_read_file(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ReadFileParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        let r = backend.read_file(&p.path).await?;
        Ok(serde_json::to_value(r)?)
    }

    async fn disk_write_file(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: WriteFileParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        backend
            .write_file(
                &p.path,
                &p.content,
                p.content_type.as_deref(),
                p.create_parents.unwrap_or(false),
            )
            .await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    async fn disk_append_file(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: AppendFileParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        backend.append_file(&p.path, &p.content).await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    async fn disk_delete_file(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        backend.delete_file(&p.path).await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    async fn disk_mkdir(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: MkdirParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        backend.mkdir(&p.path, p.recursive.unwrap_or(false)).await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    async fn disk_readdir(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ReaddirParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        let entries = backend
            .readdir(&p.path, p.recursive.unwrap_or(false))
            .await?;
        Ok(serde_json::json!({ "entries": entries }))
    }

    async fn disk_rmdir(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: RmdirParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        backend
            .rmdir(&p.path, p.recursive.unwrap_or(false))
            .await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    async fn disk_stat(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        let r = backend.stat(&p.path).await?;
        Ok(serde_json::to_value(r)?)
    }

    async fn disk_exists(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        let exists = backend.exists(&p.path).await?;
        Ok(serde_json::json!({ "exists": exists }))
    }

    async fn disk_rename(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: RenameParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        backend.rename(&p.old_path, &p.new_path).await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    async fn disk_copy(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: CopyParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        backend
            .copy(
                &p.src,
                &p.dest,
                p.overwrite.unwrap_or(false),
                p.recursive.unwrap_or(false),
            )
            .await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    async fn disk_glob(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: GlobParams = parse_params(params)?;
        let patterns: Vec<String> = match p.pattern {
            serde_json::Value::String(s) => vec![s],
            serde_json::Value::Array(arr) => arr
                .into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            _ => {
                return Err(SandboxError::InvalidParams(
                    "pattern must be string or array".into(),
                ))
            }
        };
        let backend = self.sandbox.fs_backend()?;
        let matches = backend.glob(&patterns).await?;
        Ok(serde_json::json!({ "matches": matches }))
    }

    async fn disk_tree(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        let tree = backend.tree(&p.path).await?;
        Ok(serde_json::json!({ "tree": tree }))
    }

    async fn disk_du(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        let size = backend.du(&p.path).await?;
        Ok(serde_json::json!({ "size": size }))
    }

    async fn disk_search(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SearchParams = parse_params(params)?;
        let opts = DiskSearchOptions {
            max_results: p.max_results.unwrap_or(100),
            mode: match p.mode.as_deref() {
                Some("regex") => DiskSearchMode::Regex,
                _ => DiskSearchMode::Substring,
            },
            context_before: p.context_before.unwrap_or(0),
            context_after: p.context_after.unwrap_or(0),
            count_only: p.count_only.unwrap_or(false),
            glob: p.glob,
        };
        let backend = self.sandbox.fs_backend()?;
        match backend.search("file:///", &p.query, &opts).await? {
            DiskSearchResult::Matches(matches) => {
                Ok(serde_json::json!({ "results": matches }))
            }
            DiskSearchResult::Count(count) => Ok(serde_json::json!({ "count": count })),
        }
    }

    async fn disk_history(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: PathParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        let entries = backend.history(&p.path).await?;
        Ok(serde_json::json!({ "entries": entries }))
    }

    async fn disk_diff(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: DiffParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        let d = backend
            .diff(&p.old_path, &p.new_path, p.context.unwrap_or(3))
            .await?;
        Ok(serde_json::to_value(convert_diff_output_from_raw(
            &p.old_path,
            &p.new_path,
            d,
        ))?)
    }

    async fn disk_diff_versions(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: DiffVersionsParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        let d = backend
            .diff_versions(
                &p.path,
                p.old_version,
                p.new_version,
                p.context.unwrap_or(3),
            )
            .await?;
        let old_label = format!("{}@v{}", p.path, p.old_version);
        let new_label = match p.new_version {
            Some(v) => format!("{}@v{}", p.path, v),
            None => format!("{} (current)", p.path),
        };
        Ok(serde_json::to_value(convert_diff_output_from_raw(
            &old_label,
            &new_label,
            d,
        ))?)
    }

    async fn disk_checkout(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: CheckoutParams = parse_params(params)?;
        let backend = self.sandbox.fs_backend()?;
        backend.checkout(&p.path, p.version).await?;
        Ok(serde_json::json!({ "ok": true }))
    }

    // ════════════════════════════════════════════════════════════════════
    // VSH dispatch
    // ════════════════════════════════════════════════════════════════════

    async fn dispatch_vsh(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        match method {
            // Session
            "session/create" => self.vsh_session_create(params),
            "session/get" => self.vsh_session_get(params),
            "session/list" => self.vsh_session_list(),
            "session/delete" => self.vsh_session_delete(params),

            // Exec (async)
            "exec/run" => self.vsh_exec_run(params).await,
            "exec/runRaw" => self.vsh_exec_run_raw(params).await,
            "exec/git" => self.vsh_exec_git(params).await,
            "exec/script" => self.vsh_exec_script(params).await,

            // Env
            "env/set" => self.vsh_env_set(params),
            "env/get" => self.vsh_env_get(params),
            "env/list" => self.vsh_env_list(params),
            "env/delete" => self.vsh_env_delete(params),

            // Shell
            "shell/setCwd" => self.vsh_shell_set_cwd(params),
            "shell/getCwd" => self.vsh_shell_get_cwd(params),
            "shell/setAlias" => self.vsh_shell_set_alias(params),
            "shell/getAliases" => self.vsh_shell_get_aliases(params),
            "shell/history" => self.vsh_shell_history(params),
            "shell/metrics" => self.vsh_shell_metrics(),

            _ => Err(SandboxError::InvalidParams(format!(
                "Unknown VSH method: {}",
                method
            ))),
        }
    }

    // ── VSH handlers ────────────────────────────────────────────────────

    fn vsh_session_create(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SessionCreateParams = parse_params(params)?;
        let vsh = self.sandbox.vsh_take()?;
        let (vsh, session) = vsh.create_session(p.name, p.cwd, p.env)?;
        self.sandbox.vsh_set(vsh);
        Ok(serde_json::to_value(vsh_session_to_json(&session))?)
    }

    fn vsh_session_get(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SessionIdParams = parse_params(params)?;
        let vsh = self.sandbox.vsh()?;
        let session = vsh.get_session(&p.session_id)?;
        Ok(serde_json::to_value(vsh_session_to_json(session))?)
    }

    fn vsh_session_list(&self) -> Result<serde_json::Value, SandboxError> {
        let vsh = self.sandbox.vsh()?;
        let sessions: Vec<serde_json::Value> = vsh
            .list_sessions()
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "cwd": s.cwd.display().to_string(),
                    "commandCount": s.history.len(),
                    "lastActiveAt": s.last_active_at,
                })
            })
            .collect();
        Ok(serde_json::json!({ "sessions": sessions }))
    }

    fn vsh_session_delete(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SessionIdParams = parse_params(params)?;
        let vsh = self.sandbox.vsh_take()?;
        let (vsh, deleted) = vsh.delete_session(&p.session_id)?;
        self.sandbox.vsh_set(vsh);
        Ok(serde_json::json!({ "deleted": deleted }))
    }

    async fn vsh_exec_run(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ExecRunParams = parse_params(params)?;
        let req = self.sandbox.vsh()?.prepare_exec(
            &p.session_id, &p.command, p.timeout_ms, p.max_output_bytes, p.stdin.as_deref(),
        )?;
        let env: std::collections::HashMap<String, String> =
            req.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let result = self.sandbox.shell_backend()?.execute_command(
            &req.resolved_command, &req.cwd, &env, &req.shell_cmd,
            req.timeout_ms, req.max_output_bytes, req.stdin.as_deref(),
        ).await;
        let vsh = self.sandbox.vsh_take()?;
        let vsh = vsh.record_exec(&req.session_id, &req.command, &result);
        self.sandbox.vsh_set(vsh);
        let result = result?;
        Ok(serde_json::json!({
            "stdout": result.stdout,
            "stderr": result.stderr,
            "exitCode": result.exit_code,
            "durationMs": result.duration_ms,
        }))
    }

    async fn vsh_exec_run_raw(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ExecRunRawParams = parse_params(params)?;
        let req = self.sandbox.vsh()?.prepare_exec_raw(
            &p.command, p.cwd.as_deref(), p.env.as_ref(), p.shell.as_deref(),
            p.timeout_ms, p.max_output_bytes, p.stdin.as_deref(),
        )?;
        let result = self.sandbox.shell_backend()?.execute_command(
            &req.command, &req.cwd, &req.env, &req.shell_cmd,
            req.timeout_ms, req.max_output_bytes, req.stdin.as_deref(),
        ).await;
        let vsh = self.sandbox.vsh_take()?;
        let vsh = vsh.record_exec_raw(&result);
        self.sandbox.vsh_set(vsh);
        let result = result?;
        Ok(serde_json::json!({
            "stdout": result.stdout,
            "stderr": result.stderr,
            "exitCode": result.exit_code,
            "durationMs": result.duration_ms,
        }))
    }

    async fn vsh_exec_git(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ExecGitParams = parse_params(params)?;
        let req = self.sandbox.vsh()?.prepare_exec_git(
            &p.session_id, &p.args, p.timeout_ms, p.max_output_bytes,
        )?;
        let env: std::collections::HashMap<String, String> =
            req.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let result = self.sandbox.shell_backend()?.execute_git(
            &req.args, &req.cwd, &env, req.timeout_ms, req.max_output_bytes,
        ).await;
        let vsh = self.sandbox.vsh_take()?;
        let vsh = vsh.record_exec(&req.session_id, &req.command_str, &result);
        self.sandbox.vsh_set(vsh);
        let result = result?;
        Ok(serde_json::json!({
            "stdout": result.stdout,
            "stderr": result.stderr,
            "exitCode": result.exit_code,
            "durationMs": result.duration_ms,
        }))
    }

    async fn vsh_exec_script(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ExecScriptParams = parse_params(params)?;
        let req = self.sandbox.vsh()?.prepare_exec(
            &p.session_id, &p.script, p.timeout_ms, p.max_output_bytes, None,
        )?;
        let env: std::collections::HashMap<String, String> =
            req.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let result = self.sandbox.shell_backend()?.execute_command(
            &req.resolved_command, &req.cwd, &env, &req.shell_cmd,
            req.timeout_ms, req.max_output_bytes, req.stdin.as_deref(),
        ).await;
        let vsh = self.sandbox.vsh_take()?;
        let vsh = vsh.record_exec(&req.session_id, &req.command, &result);
        self.sandbox.vsh_set(vsh);
        let result = result?;
        Ok(serde_json::json!({
            "stdout": result.stdout,
            "stderr": result.stderr,
            "exitCode": result.exit_code,
            "durationMs": result.duration_ms,
        }))
    }

    fn vsh_env_set(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: EnvSetParams = parse_params(params)?;
        let vsh = self.sandbox.vsh_take()?;
        let vsh = vsh.set_env(&p.session_id, &p.key, &p.value)?;
        self.sandbox.vsh_set(vsh);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vsh_env_get(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: EnvGetParams = parse_params(params)?;
        let vsh = self.sandbox.vsh()?;
        let value = vsh.get_env(&p.session_id, &p.key)?;
        Ok(serde_json::json!({ "value": value }))
    }

    fn vsh_env_list(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SessionIdParams = parse_params(params)?;
        let vsh = self.sandbox.vsh()?;
        let env = vsh.list_env(&p.session_id)?;
        Ok(serde_json::json!({ "env": env }))
    }

    fn vsh_env_delete(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: EnvDeleteParams = parse_params(params)?;
        let vsh = self.sandbox.vsh_take()?;
        let (vsh, deleted) = vsh.delete_env(&p.session_id, &p.key)?;
        self.sandbox.vsh_set(vsh);
        Ok(serde_json::json!({ "deleted": deleted }))
    }

    fn vsh_shell_set_cwd(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ShellSetCwdParams = parse_params(params)?;
        let vsh = self.sandbox.vsh_take()?;
        let vsh = vsh.set_cwd(&p.session_id, &p.cwd)?;
        self.sandbox.vsh_set(vsh);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vsh_shell_get_cwd(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SessionIdParams = parse_params(params)?;
        let vsh = self.sandbox.vsh()?;
        let cwd = vsh.get_cwd(&p.session_id)?;
        Ok(serde_json::json!({ "cwd": cwd }))
    }

    fn vsh_shell_set_alias(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ShellSetAliasParams = parse_params(params)?;
        let vsh = self.sandbox.vsh_take()?;
        let vsh = vsh.set_alias(&p.session_id, &p.name, &p.command)?;
        self.sandbox.vsh_set(vsh);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vsh_shell_get_aliases(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SessionIdParams = parse_params(params)?;
        let vsh = self.sandbox.vsh()?;
        let aliases = vsh.get_aliases(&p.session_id)?;
        Ok(serde_json::json!({ "aliases": aliases }))
    }

    fn vsh_shell_history(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: SessionIdParams = parse_params(params)?;
        let vsh = self.sandbox.vsh()?;
        let history = vsh.get_history(&p.session_id)?;
        let entries: Vec<serde_json::Value> = history
            .iter()
            .map(|h| {
                serde_json::json!({
                    "command": h.command,
                    "exitCode": h.exit_code,
                    "timestamp": h.timestamp,
                    "durationMs": h.duration_ms,
                })
            })
            .collect();
        Ok(serde_json::json!({ "history": entries }))
    }

    fn vsh_shell_metrics(&self) -> Result<serde_json::Value, SandboxError> {
        let vsh = self.sandbox.vsh()?;
        Ok(serde_json::json!({
            "sessionCount": vsh.session_count(),
            "totalCommands": vsh.total_commands(),
            "totalErrors": vsh.total_errors(),
        }))
    }

    // ════════════════════════════════════════════════════════════════════
    // VNet dispatch
    // ════════════════════════════════════════════════════════════════════

    async fn dispatch_vnet(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        match method {
            // Network -- async path for methods that may hit a real backend
            "net/httpRequest" => self.vnet_http_request(params).await,

            // Network -- sync paths
            "net/wsConnect" => self.vnet_ws_connect(params),
            "net/wsMessage" => self.vnet_ws_message(params),
            "net/wsClose" => self.vnet_ws_close(params),
            "net/tcpConnect" => self.vnet_tcp_connect(params),
            "net/tcpSend" => self.vnet_tcp_send(params),
            "net/tcpClose" => self.vnet_tcp_close(params),
            "net/udpSend" => self.vnet_udp_send(params),
            "net/resolve" => self.vnet_resolve(params),
            "net/metrics" => self.vnet_metrics(),

            // Mock
            "mock/register" => self.vnet_mock_register(params),
            "mock/unregister" => self.vnet_mock_unregister(params),
            "mock/list" => self.vnet_mock_list(),
            "mock/clear" => self.vnet_mock_clear(),
            "mock/history" => self.vnet_mock_history(),

            _ => Err(SandboxError::InvalidParams(format!(
                "Unknown VNet method: {}",
                method
            ))),
        }
    }

    fn dispatch_vnet_session(
        &self,
        sub: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        match sub {
            "list" => self.vnet_session_list(),
            "get" => self.vnet_session_get(params),
            "close" => {
                // Need mut access; this is a slight design limitation since
                // dispatch_vnet_session takes &self. We'll handle it via
                // a different route; but for the immutable list/get it works.
                // close needs to go through dispatch_vnet.
                Err(SandboxError::InvalidParams(
                    "Use sandbox/net/sessionClose for closing sessions".into(),
                ))
            }
            _ => Err(SandboxError::InvalidParams(format!(
                "Unknown VNet session method: netSession/{}",
                sub
            ))),
        }
    }

    // ── VNet handlers ───────────────────────────────────────────────────

    async fn vnet_http_request(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: HttpRequestParams = parse_params(params)?;
        let url = &p.url;

        if url.starts_with("mock://") {
            let sb = &mut self.sandbox;
            let vnet = sb.vnet_take()?;
            let (vnet, result) = vnet.mock_http_request(
                url,
                p.method.as_deref(),
                p.headers.as_ref(),
                p.body.as_deref(),
            );
            sb.vnet_set(vnet);
            Ok(serde_json::to_value(result?)?)
        } else if let Some(remainder) = url.strip_prefix("net://") {
            // strip "net://"
            let real_url = if remainder.starts_with("http://") || remainder.starts_with("https://") {
                remainder.to_string()
            } else {
                format!("https://{remainder}")
            };

            let method = p.method.as_deref().unwrap_or("GET");
            let headers = p.headers.unwrap_or_default();
            // PERF: hot-path I/O -- net_http_request needs &mut self for backend
            let vnet = self.sandbox.vnet_mut()?;
            let result = vnet
                .net_http_request(&real_url, method, &headers, p.body.as_deref(), p.timeout_ms)
                .await?;
            Ok(serde_json::to_value(result)?)
        } else {
            Err(SandboxError::InvalidParams(format!(
                "unsupported URL scheme: {} (use mock:// or net://)",
                url
            )))
        }
    }

    fn vnet_ws_connect(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: WsConnectParams = parse_params(params)?;
        let url = &p.url;

        if url.starts_with("mock://") {
            let target = url.strip_prefix("mock://").unwrap_or(url);
            let sb = &mut self.sandbox;
            let vnet = sb.vnet_take()?;
            vnet.check_connection_limit()?;
            let (vnet, session_id) =
                vnet.create_session(VnetSessionType::Ws, target, VnetScheme::Mock);
            sb.vnet_set(vnet);
            Ok(serde_json::json!({
                "sessionId": session_id,
                "status": "connected"
            }))
        } else if url.starts_with("net://") {
            Err(SandboxError::VnetConnectionFailed(
                "net:// WebSocket not yet implemented".to_string(),
            ))
        } else {
            Err(SandboxError::InvalidParams(format!(
                "unsupported URL scheme: {} (use mock:// or net://)",
                url
            )))
        }
    }

    fn vnet_ws_message(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: WsMessageParams = parse_params(params)?;
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;
        vnet.get_session(&p.session_id)?;
        let data_len = p.data.len() as u64;
        let vnet = vnet.record_session_activity(&p.session_id, data_len, 0);
        sb.vnet_set(vnet);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vnet_ws_close(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: VnetSessionIdParam = parse_params(params)?;
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;
        let (vnet, result) = vnet.close_session(&p.session_id);
        sb.vnet_set(vnet);
        result?;
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vnet_tcp_connect(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: TcpConnectParams = parse_params(params)?;
        let target = format!("{}:{}", p.host, p.port);
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;
        vnet.check_connection_limit()?;
        let (vnet, session_id) =
            vnet.create_session(VnetSessionType::Tcp, &target, VnetScheme::Mock);
        sb.vnet_set(vnet);
        Ok(serde_json::json!({
            "sessionId": session_id,
            "status": "connected"
        }))
    }

    fn vnet_tcp_send(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: TcpSendParams = parse_params(params)?;
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;
        vnet.get_session(&p.session_id)?;
        let bytes_written = p.data.len() as u64;
        let vnet = vnet.record_session_activity(&p.session_id, bytes_written, 0);
        sb.vnet_set(vnet);
        Ok(serde_json::json!({ "bytesWritten": bytes_written }))
    }

    fn vnet_tcp_close(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: VnetSessionIdParam = parse_params(params)?;
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;
        let (vnet, result) = vnet.close_session(&p.session_id);
        sb.vnet_set(vnet);
        result?;
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vnet_udp_send(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: UdpSendParams = parse_params(params)?;
        let mock_url = format!("mock://{}:{}", p.host, p.port);
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;
        let vnet = vnet.increment_requests();

        let (vnet, found) = vnet.mock_store_find_match(&mock_url, Some("udp"));
        sb.vnet_set(vnet);
        match found {
            Some(response) => {
                let bytes_received = response.body.len() as u64;
                Ok(serde_json::json!({
                    "response": response.body,
                    "bytesReceived": bytes_received
                }))
            }
            None => Ok(serde_json::json!({
                "response": null,
                "bytesReceived": 0
            })),
        }
    }

    fn vnet_resolve(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: ResolveParams = parse_params(params)?;
        let mock_url = format!("mock://dns/{}", p.hostname);
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;

        let (vnet, found) = vnet.mock_store_find_match(&mock_url, Some("dns"));
        sb.vnet_set(vnet);
        match found {
            Some(response) => {
                let addresses: Vec<String> =
                    serde_json::from_str(&response.body).unwrap_or_default();
                Ok(serde_json::json!({
                    "addresses": addresses,
                    "ttl": response.delay_ms,
                }))
            }
            None => Err(SandboxError::VnetDnsResolutionFailed(p.hostname)),
        }
    }

    fn vnet_metrics(&self) -> Result<serde_json::Value, SandboxError> {
        let vnet = self.sandbox.vnet()?;
        Ok(serde_json::to_value(vnet.metrics())?)
    }

    fn vnet_mock_register(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: MockRegisterParams = parse_params(params)?;
        let response = MockResponse {
            status: p.response.status,
            headers: p.response.headers,
            body: p.response.body,
            body_type: p.response.body_type,
            delay_ms: p.response.delay_ms,
        };
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;
        let (vnet, id) = vnet.register_mock(p.method, &p.url_pattern, response, p.times);
        sb.vnet_set(vnet);
        Ok(serde_json::json!({ "id": id }))
    }

    fn vnet_mock_unregister(
        &mut self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: MockIdParam = parse_params(params)?;
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;
        let (vnet, result) = vnet.unregister_mock(&p.id);
        sb.vnet_set(vnet);
        result?;
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vnet_mock_list(&self) -> Result<serde_json::Value, SandboxError> {
        let vnet = self.sandbox.vnet()?;
        let mocks = vnet.list_mocks();
        Ok(serde_json::to_value(mocks)?)
    }

    fn vnet_mock_clear(&mut self) -> Result<serde_json::Value, SandboxError> {
        let sb = &mut self.sandbox;
        let vnet = sb.vnet_take()?;
        let vnet = vnet.clear_mocks();
        sb.vnet_set(vnet);
        Ok(serde_json::json!({ "ok": true }))
    }

    fn vnet_mock_history(&self) -> Result<serde_json::Value, SandboxError> {
        let vnet = self.sandbox.vnet()?;
        let history = vnet.mock_history();
        Ok(serde_json::to_value(history)?)
    }

    fn vnet_session_list(&self) -> Result<serde_json::Value, SandboxError> {
        let vnet = self.sandbox.vnet()?;
        let sessions = vnet.list_sessions();
        Ok(serde_json::to_value(sessions)?)
    }

    fn vnet_session_get(
        &self,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, SandboxError> {
        let p: VnetSessionIdParam = parse_params(params)?;
        let vnet = self.sandbox.vnet()?;
        let info = vnet.get_session(&p.session_id)?;
        Ok(serde_json::to_value(info)?)
    }
}

// ── Diff conversion helpers ─────────────────────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffResultJson {
    old_path: String,
    new_path: String,
    hunks: Vec<DiffHunkJson>,
    additions: usize,
    deletions: usize,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffHunkJson {
    old_start: usize,
    old_count: usize,
    new_start: usize,
    new_count: usize,
    lines: Vec<DiffLineJson>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffLineJson {
    #[serde(rename = "type")]
    line_type: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_line: Option<usize>,
}

fn convert_diff_output(d: DiffResultOutput) -> DiffResultJson {
    DiffResultJson {
        old_path: d.old_path,
        new_path: d.new_path,
        additions: d.additions,
        deletions: d.deletions,
        hunks: d
            .hunks
            .into_iter()
            .map(|h| DiffHunkJson {
                old_start: h.old_start,
                old_count: h.old_count,
                new_start: h.new_start,
                new_count: h.new_count,
                lines: h
                    .lines
                    .into_iter()
                    .map(|l| DiffLineJson {
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

fn convert_diff_output_from_raw(
    old_path: &str,
    new_path: &str,
    d: DiffOutput,
) -> DiffResultJson {
    DiffResultJson {
        old_path: old_path.to_string(),
        new_path: new_path.to_string(),
        additions: d.additions,
        deletions: d.deletions,
        hunks: d
            .hunks
            .into_iter()
            .map(|h| DiffHunkJson {
                old_start: h.old_start,
                old_count: h.old_count,
                new_start: h.new_start,
                new_count: h.new_count,
                lines: h
                    .lines
                    .into_iter()
                    .map(|l| DiffLineJson {
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

// ── VSH helper ──────────────────────────────────────────────────────────────

fn vsh_session_to_json(session: &crate::vsh_shell::ShellSession) -> serde_json::Value {
    serde_json::json!({
        "id": session.id,
        "name": session.name,
        "cwd": session.cwd.display().to_string(),
        "env": session.env,
        "aliases": session.aliases,
        "createdAt": session.created_at,
        "lastActiveAt": session.last_active_at,
        "commandCount": session.history.len(),
    })
}
