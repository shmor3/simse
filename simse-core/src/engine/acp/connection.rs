use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use agent_client_protocol::{
    self as acp, Agent, Client, ClientSideConnection, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
    PromptRequest, PromptResponse, RequestPermissionRequest, RequestPermissionResponse,
    RequestPermissionOutcome, SelectedPermissionOutcome, SessionNotification,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, SetSessionModeRequest,
    SetSessionModeResponse, CancelNotification, SessionId, Implementation,
    ProtocolVersion, SessionConfigId, SessionConfigValueId,
    SessionModeId, McpServer, ContentBlock, StreamReceiver, PermissionOptionKind,
};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

use crate::engine::acp::error::AcpError;
use crate::engine::acp::permission::PermissionPolicy;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: HashMap<String, String>,
    pub timeout_ms: u64,
    pub init_timeout_ms: u64,
    pub client_name: String,
    pub client_version: String,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            cwd: None,
            env: HashMap::new(),
            timeout_ms: 60_000,
            init_timeout_ms: 30_000,
            client_name: "simse".into(),
            client_version: "1.0.0".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Channel-based request/response protocol
// ---------------------------------------------------------------------------

pub(crate) enum ConnectionRequest {
    Initialize {
        resp: oneshot::Sender<Result<InitializeResponse, AcpError>>,
    },
    NewSession {
        cwd: PathBuf,
        mcp_servers: Vec<McpServer>,
        resp: oneshot::Sender<Result<NewSessionResponse, AcpError>>,
    },
    Prompt {
        session_id: SessionId,
        content: Vec<ContentBlock>,
        resp: oneshot::Sender<Result<PromptResponse, AcpError>>,
    },
    LoadSession {
        session_id: SessionId,
        cwd: PathBuf,
        mcp_servers: Vec<McpServer>,
        resp: oneshot::Sender<Result<LoadSessionResponse, AcpError>>,
    },
    SetSessionMode {
        session_id: SessionId,
        mode_id: SessionModeId,
        resp: oneshot::Sender<Result<SetSessionModeResponse, AcpError>>,
    },
    SetConfigOption {
        session_id: SessionId,
        config_id: SessionConfigId,
        value: SessionConfigValueId,
        resp: oneshot::Sender<Result<SetSessionConfigOptionResponse, AcpError>>,
    },
    Cancel {
        session_id: SessionId,
    },
    SetPermissionPolicy {
        policy: PermissionPolicy,
    },
    Subscribe {
        resp: oneshot::Sender<StreamReceiver>,
    },
    Close {
        resp: oneshot::Sender<()>,
    },
}

// ---------------------------------------------------------------------------
// ConnectionWrapper — Send-safe public handle
// ---------------------------------------------------------------------------

pub struct ConnectionWrapper {
    request_tx: mpsc::UnboundedSender<ConnectionRequest>,
    update_tx: broadcast::Sender<SessionNotification>,
    connected: Arc<AtomicBool>,
    #[allow(dead_code)]
    thread_handle: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl ConnectionWrapper {
    pub async fn new(config: ConnectionConfig) -> Result<Self, AcpError> {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (update_tx, _) = broadcast::channel(256);
        let connected = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = oneshot::channel::<Result<(), AcpError>>();

        let connected_clone = connected.clone();
        let update_tx_clone = update_tx.clone();

        let thread_handle = std::thread::spawn(move || {
            run_connection_thread(
                config,
                request_rx,
                update_tx_clone,
                connected_clone,
                ready_tx,
            );
        });

        ready_rx
            .await
            .map_err(|_| AcpError::ConnectionFailed("connection thread died during setup".into()))??;

        Ok(Self {
            request_tx,
            update_tx,
            connected,
            thread_handle: std::sync::Mutex::new(Some(thread_handle)),
        })
    }

    pub fn is_healthy(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn is_connected(&self) -> bool {
        self.is_healthy()
    }

    pub fn subscribe_updates(&self) -> broadcast::Receiver<SessionNotification> {
        self.update_tx.subscribe()
    }

    pub async fn subscribe_stream(&self) -> Result<StreamReceiver, AcpError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.request_tx
            .send(ConnectionRequest::Subscribe { resp: resp_tx })
            .map_err(|_| AcpError::ConnectionFailed("connection thread closed".into()))?;
        resp_rx
            .await
            .map_err(|_| AcpError::ConnectionFailed("connection thread died".into()))
    }

    pub async fn initialize(&self) -> Result<InitializeResponse, AcpError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.request_tx
            .send(ConnectionRequest::Initialize { resp: resp_tx })
            .map_err(|_| AcpError::ConnectionFailed("connection thread closed".into()))?;
        resp_rx
            .await
            .map_err(|_| AcpError::ConnectionFailed("connection thread died".into()))?
    }

    pub async fn new_session(
        &self,
        cwd: PathBuf,
        mcp_servers: Vec<McpServer>,
    ) -> Result<NewSessionResponse, AcpError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.request_tx
            .send(ConnectionRequest::NewSession {
                cwd,
                mcp_servers,
                resp: resp_tx,
            })
            .map_err(|_| AcpError::ConnectionFailed("connection thread closed".into()))?;
        resp_rx
            .await
            .map_err(|_| AcpError::ConnectionFailed("connection thread died".into()))?
    }

    pub async fn prompt(
        &self,
        session_id: SessionId,
        content: Vec<ContentBlock>,
    ) -> Result<PromptResponse, AcpError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.request_tx
            .send(ConnectionRequest::Prompt {
                session_id,
                content,
                resp: resp_tx,
            })
            .map_err(|_| AcpError::ConnectionFailed("connection thread closed".into()))?;
        resp_rx
            .await
            .map_err(|_| AcpError::ConnectionFailed("connection thread died".into()))?
    }

    pub async fn load_session(
        &self,
        session_id: SessionId,
        cwd: PathBuf,
        mcp_servers: Vec<McpServer>,
    ) -> Result<LoadSessionResponse, AcpError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.request_tx
            .send(ConnectionRequest::LoadSession {
                session_id,
                cwd,
                mcp_servers,
                resp: resp_tx,
            })
            .map_err(|_| AcpError::ConnectionFailed("connection thread closed".into()))?;
        resp_rx
            .await
            .map_err(|_| AcpError::ConnectionFailed("connection thread died".into()))?
    }

    pub async fn set_session_mode(
        &self,
        session_id: SessionId,
        mode_id: SessionModeId,
    ) -> Result<SetSessionModeResponse, AcpError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.request_tx
            .send(ConnectionRequest::SetSessionMode {
                session_id,
                mode_id,
                resp: resp_tx,
            })
            .map_err(|_| AcpError::ConnectionFailed("connection thread closed".into()))?;
        resp_rx
            .await
            .map_err(|_| AcpError::ConnectionFailed("connection thread died".into()))?
    }

    pub async fn set_config_option(
        &self,
        session_id: SessionId,
        config_id: SessionConfigId,
        value: SessionConfigValueId,
    ) -> Result<SetSessionConfigOptionResponse, AcpError> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self.request_tx
            .send(ConnectionRequest::SetConfigOption {
                session_id,
                config_id,
                value,
                resp: resp_tx,
            })
            .map_err(|_| AcpError::ConnectionFailed("connection thread closed".into()))?;
        resp_rx
            .await
            .map_err(|_| AcpError::ConnectionFailed("connection thread died".into()))?
    }

    pub fn cancel(&self, session_id: SessionId) {
        let _ = self.request_tx.send(ConnectionRequest::Cancel { session_id });
    }

    pub fn set_permission_policy(&self, policy: PermissionPolicy) {
        let _ = self
            .request_tx
            .send(ConnectionRequest::SetPermissionPolicy { policy });
    }

    pub async fn close(&self) {
        let (resp_tx, resp_rx) = oneshot::channel();
        let _ = self.request_tx.send(ConnectionRequest::Close { resp: resp_tx });
        let _ = resp_rx.await;
    }
}

// ---------------------------------------------------------------------------
// SimseClient — implements the SDK Client trait (!Send, runs in LocalSet)
// ---------------------------------------------------------------------------

struct SimseClient {
    permission_policy: Arc<RwLock<PermissionPolicy>>,
    update_tx: broadcast::Sender<SessionNotification>,
}

#[async_trait::async_trait(?Send)]
impl Client for SimseClient {
    async fn request_permission(
        &self,
        args: RequestPermissionRequest,
    ) -> acp::Result<RequestPermissionResponse> {
        let policy = *self.permission_policy.read().await;
        match resolve_permission_sdk(policy, &args) {
            Some(response) => Ok(response),
            None => {
                // Prompt mode: for now, cancel (external resolution not yet wired)
                Ok(RequestPermissionResponse::new(RequestPermissionOutcome::Cancelled))
            }
        }
    }

    async fn session_notification(&self, args: SessionNotification) -> acp::Result<()> {
        let _ = self.update_tx.send(args);
        Ok(())
    }
}

/// Map PermissionPolicy to SDK RequestPermissionResponse.
fn resolve_permission_sdk(
    policy: PermissionPolicy,
    params: &RequestPermissionRequest,
) -> Option<RequestPermissionResponse> {
    match policy {
        PermissionPolicy::AutoApprove => {
            let options = &params.options;
            let selected = options
                .iter()
                .find(|o| o.kind == PermissionOptionKind::AllowAlways)
                .or_else(|| options.iter().find(|o| o.kind == PermissionOptionKind::AllowOnce))
                .or_else(|| options.first());
            Some(RequestPermissionResponse::new(match selected {
                Some(opt) => RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    opt.option_id.clone(),
                )),
                None => RequestPermissionOutcome::Cancelled,
            }))
        }
        PermissionPolicy::Deny => {
            let options = &params.options;
            let selected = options
                .iter()
                .find(|o| o.kind == PermissionOptionKind::RejectOnce)
                .or_else(|| options.iter().find(|o| o.kind == PermissionOptionKind::RejectAlways));
            Some(RequestPermissionResponse::new(match selected {
                Some(opt) => RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                    opt.option_id.clone(),
                )),
                None => RequestPermissionOutcome::Cancelled,
            }))
        }
        PermissionPolicy::Prompt => None,
    }
}

// ---------------------------------------------------------------------------
// Connection thread — runs LocalSet with ClientSideConnection
// ---------------------------------------------------------------------------

fn run_connection_thread(
    config: ConnectionConfig,
    mut request_rx: mpsc::UnboundedReceiver<ConnectionRequest>,
    update_tx: broadcast::Sender<SessionNotification>,
    connected: Arc<AtomicBool>,
    ready_tx: oneshot::Sender<Result<(), AcpError>>,
) {
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let _ = ready_tx.send(Err(AcpError::ConnectionFailed(e.to_string())));
            return;
        }
    };

    let local = tokio::task::LocalSet::new();

    local.block_on(&rt, async move {
        // Spawn child process
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        cmd.env_remove("CLAUDECODE");

        if let Some(ref cwd) = config.cwd {
            cmd.current_dir(cwd);
        }
        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let _ = ready_tx.send(Err(AcpError::ConnectionFailed(format!(
                    "failed to spawn {}: {e}",
                    config.command
                ))));
                return;
            }
        };

        let stdin = child.stdin.take().unwrap().compat_write();
        let stdout = child.stdout.take().unwrap().compat();

        // Spawn stderr reader
        if let Some(stderr) = child.stderr.take() {
            tokio::task::spawn_local(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::warn!("ACP stderr: {line}");
                }
            });
        }

        let permission_policy = Arc::new(RwLock::new(PermissionPolicy::Prompt));

        let client = SimseClient {
            permission_policy: permission_policy.clone(),
            update_tx: update_tx.clone(),
        };

        let (conn, io_task) = ClientSideConnection::new(client, stdin, stdout, |fut| {
            tokio::task::spawn_local(fut);
        });

        // Spawn the IO task
        tokio::task::spawn_local(async move {
            if let Err(e) = io_task.await {
                tracing::error!("ACP IO task error: {e}");
            }
        });

        connected.store(true, Ordering::SeqCst);
        let _ = ready_tx.send(Ok(()));

        // Process requests until Close or channel drops
        while let Some(req) = request_rx.recv().await {
            match req {
                ConnectionRequest::Initialize { resp } => {
                    let result = conn
                        .initialize(
                            InitializeRequest::new(ProtocolVersion::V1)
                                .client_info(Implementation::new(
                                    config.client_name.clone(),
                                    config.client_version.clone(),
                                )),
                        )
                        .await
                        .map_err(AcpError::from);
                    let _ = resp.send(result);
                }
                ConnectionRequest::NewSession {
                    cwd,
                    mcp_servers,
                    resp,
                } => {
                    let result = conn
                        .new_session(NewSessionRequest::new(cwd).mcp_servers(mcp_servers))
                        .await
                        .map_err(AcpError::from);
                    let _ = resp.send(result);
                }
                ConnectionRequest::Prompt {
                    session_id,
                    content,
                    resp,
                } => {
                    let result = conn
                        .prompt(PromptRequest::new(session_id, content))
                        .await
                        .map_err(AcpError::from);
                    let _ = resp.send(result);
                }
                ConnectionRequest::LoadSession {
                    session_id,
                    cwd,
                    mcp_servers,
                    resp,
                } => {
                    let result = conn
                        .load_session(
                            LoadSessionRequest::new(session_id, cwd)
                                .mcp_servers(mcp_servers),
                        )
                        .await
                        .map_err(AcpError::from);
                    let _ = resp.send(result);
                }
                ConnectionRequest::SetSessionMode {
                    session_id,
                    mode_id,
                    resp,
                } => {
                    let result = conn
                        .set_session_mode(SetSessionModeRequest::new(session_id, mode_id))
                        .await
                        .map_err(AcpError::from);
                    let _ = resp.send(result);
                }
                ConnectionRequest::SetConfigOption {
                    session_id,
                    config_id,
                    value,
                    resp,
                } => {
                    let result = conn
                        .set_session_config_option(
                            SetSessionConfigOptionRequest::new(session_id, config_id, value),
                        )
                        .await
                        .map_err(AcpError::from);
                    let _ = resp.send(result);
                }
                ConnectionRequest::Cancel { session_id } => {
                    let _ = conn
                        .cancel(CancelNotification::new(session_id))
                        .await;
                }
                ConnectionRequest::SetPermissionPolicy { policy } => {
                    *permission_policy.write().await = policy;
                }
                ConnectionRequest::Subscribe { resp } => {
                    let _ = resp.send(conn.subscribe());
                }
                ConnectionRequest::Close { resp } => {
                    let _ = resp.send(());
                    break;
                }
            }
        }

        connected.store(false, Ordering::SeqCst);
        drop(child);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_config_defaults() {
        let cfg = ConnectionConfig::default();
        assert_eq!(cfg.timeout_ms, 60_000);
        assert_eq!(cfg.init_timeout_ms, 30_000);
        assert_eq!(cfg.client_name, "simse");
        assert_eq!(cfg.client_version, "1.0.0");
    }
}
