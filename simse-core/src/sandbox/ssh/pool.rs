use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use russh::client::{self, Handle, Msg};
use russh::keys::PublicKeyBase64;
use russh::{Channel, Disconnect};
use russh_sftp::client::SftpSession;
use sha2::{Digest, Sha256};

use crate::sandbox::config::{HostKeyPolicy, SshAuth, SshConfig};
use crate::sandbox::error::SandboxError;

/// Compute the SHA256 fingerprint of an SSH public key.
///
/// Returns a string in the format `SHA256:<base64>` (matching `ssh-keygen -lf`).
fn ssh_key_fingerprint(key: &russh::keys::PublicKey) -> String {
    let key_bytes = key.public_key_bytes();
    let hash = Sha256::digest(key_bytes);
    let b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(hash);
    format!("SHA256:{b64}")
}

/// SSH client handler that tracks connection health and verifies host keys.
///
/// Host key verification is controlled by [`HostKeyPolicy`]:
/// - `AcceptAll` — accepts any key (for trusted/internal networks)
/// - `Fingerprint(expected)` — accepts only if the SHA256 fingerprint matches
pub struct SshHandler {
    healthy: Arc<AtomicBool>,
    host_key_policy: HostKeyPolicy,
    host: String,
}

impl SshHandler {
    fn new(healthy: Arc<AtomicBool>, host_key_policy: HostKeyPolicy, host: String) -> Self {
        Self {
            healthy,
            host_key_policy,
            host,
        }
    }
}

impl client::Handler for SshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = ssh_key_fingerprint(server_public_key);

        match &self.host_key_policy {
            HostKeyPolicy::AcceptAll => {
                tracing::debug!(
                    host = %self.host,
                    fingerprint = %fingerprint,
                    "Accepted server key (policy: AcceptAll)"
                );
                Ok(true)
            }
            HostKeyPolicy::Fingerprint(expected) => {
                if fingerprint == *expected {
                    tracing::debug!(
                        host = %self.host,
                        fingerprint = %fingerprint,
                        "Server key fingerprint verified"
                    );
                    Ok(true)
                } else {
                    tracing::error!(
                        host = %self.host,
                        expected = %expected,
                        actual = %fingerprint,
                        "Server key fingerprint mismatch — possible MITM attack"
                    );
                    Ok(false)
                }
            }
        }
    }

    async fn disconnected(
        &mut self,
        reason: client::DisconnectReason<Self::Error>,
    ) -> Result<(), Self::Error> {
        self.healthy.store(false, Ordering::SeqCst);
        tracing::warn!("SSH disconnected: {reason:?}");
        match reason {
            client::DisconnectReason::ReceivedDisconnect(_) => Ok(()),
            client::DisconnectReason::Error(e) => Err(e),
        }
    }
}

/// A pool of SSH channels multiplexed over a single TCP connection.
///
/// Uses `russh` to maintain a persistent SSH connection and provides
/// methods to open exec, SFTP, and direct-tcpip channels on demand.
/// The underlying handle is protected by a tokio Mutex so it can be
/// shared across tasks.
pub struct SshPool {
    handle: Arc<tokio::sync::Mutex<Handle<SshHandler>>>,
    config: SshConfig,
    healthy: Arc<AtomicBool>,
}

impl SshPool {
    /// Connect to a remote SSH server and authenticate.
    ///
    /// Creates a `russh` client config, establishes the TCP connection,
    /// and authenticates using the method specified in `SshConfig`.
    pub async fn connect(config: &SshConfig) -> Result<Self, SandboxError> {
        let healthy = Arc::new(AtomicBool::new(true));
        let handler = SshHandler::new(
            Arc::clone(&healthy),
            config.host_key_policy.clone(),
            format!("{}:{}", config.host, config.port),
        );

        let russh_config = Arc::new(client::Config {
            keepalive_interval: if config.keepalive_interval_ms > 0 {
                Some(Duration::from_millis(config.keepalive_interval_ms))
            } else {
                None
            },
            ..Default::default()
        });

        let addr = format!("{}:{}", config.host, config.port);
        let mut handle = client::connect(russh_config, &addr, handler)
            .await
            .map_err(|e| SandboxError::SshConnection(format!("connect to {addr}: {e}")))?;

        // Authenticate
        Self::authenticate(&mut handle, config).await?;

        Ok(Self {
            handle: Arc::new(tokio::sync::Mutex::new(handle)),
            config: config.clone(),
            healthy,
        })
    }

    /// Authenticate the SSH connection using the configured auth method.
    async fn authenticate(
        handle: &mut Handle<SshHandler>,
        config: &SshConfig,
    ) -> Result<(), SandboxError> {
        let result = match &config.auth {
            SshAuth::Password { password } => handle
                .authenticate_password(&config.username, password)
                .await
                .map_err(|e| SandboxError::SshAuth(format!("password auth: {e}")))?,

            SshAuth::Key {
                private_key_path,
                passphrase,
            } => {
                let key = russh::keys::load_secret_key(
                    private_key_path,
                    passphrase.as_deref(),
                )
                .map_err(|e| {
                    SandboxError::SshAuth(format!("load key {private_key_path}: {e}"))
                })?;

                let key_arc = Arc::new(key);

                // Determine best RSA hash algorithm if applicable
                let hash_alg = if key_arc.algorithm().is_rsa() {
                    handle
                        .best_supported_rsa_hash()
                        .await
                        .map_err(|e| {
                            SandboxError::SshAuth(format!("rsa hash negotiation: {e}"))
                        })?
                        .flatten()
                } else {
                    None
                };

                let key_with_hash =
                    russh::keys::PrivateKeyWithHashAlg::new(key_arc, hash_alg);

                handle
                    .authenticate_publickey(&config.username, key_with_hash)
                    .await
                    .map_err(|e| SandboxError::SshAuth(format!("pubkey auth: {e}")))?
            }

            SshAuth::Agent => {
                Self::authenticate_with_agent(handle, &config.username).await?
            }
        };

        match result {
            russh::client::AuthResult::Success => {
                tracing::info!("SSH authenticated as {}", config.username);
                Ok(())
            }
            russh::client::AuthResult::Failure {
                remaining_methods, ..
            } => Err(SandboxError::SshAuth(format!(
                "authentication failed for user '{}' (remaining methods: {remaining_methods:?})",
                config.username
            ))),
        }
    }

    /// Authenticate using the SSH agent.
    ///
    /// Connects to the platform's SSH agent, lists available identities,
    /// and tries each key until one succeeds. Uses `.dynamic()` to erase
    /// the platform-specific stream type.
    async fn authenticate_with_agent(
        handle: &mut Handle<SshHandler>,
        username: &str,
    ) -> Result<russh::client::AuthResult, SandboxError> {
        #[cfg(unix)]
        let agent = russh::keys::agent::client::AgentClient::connect_env()
            .await
            .map_err(|e| SandboxError::SshAuth(format!("agent connect: {e}")))?;

        #[cfg(windows)]
        let agent = russh::keys::agent::client::AgentClient::connect_pageant()
            .await
            .map_err(|e| SandboxError::SshAuth(format!("agent connect: {e}")))?;

        // Erase the platform-specific stream type so the rest of
        // this function is platform-independent.
        let mut agent = agent.dynamic();

        let identities = agent
            .request_identities()
            .await
            .map_err(|e| SandboxError::SshAuth(format!("agent list identities: {e}")))?;

        if identities.is_empty() {
            return Err(SandboxError::SshAuth(
                "SSH agent has no identities".into(),
            ));
        }

        for pubkey in identities {
            let result = handle
                .authenticate_publickey_with(username, pubkey, None, &mut agent)
                .await
                .map_err(|e| SandboxError::SshAuth(format!("agent auth: {e}")))?;

            if matches!(result, russh::client::AuthResult::Success) {
                return Ok(result);
            }
        }

        Ok(russh::client::AuthResult::Failure {
            remaining_methods: russh::MethodSet::empty(),
            partial_success: false,
        })
    }

    /// Open a new session channel for command execution.
    pub async fn get_exec_channel(&self) -> Result<Channel<Msg>, SandboxError> {
        let handle = self.handle.lock().await;
        handle
            .channel_open_session()
            .await
            .map_err(|e| SandboxError::SshChannel(format!("open session channel: {e}")))
    }

    /// Open an SFTP session over a new channel.
    ///
    /// Opens a session channel, requests the "sftp" subsystem, and wraps
    /// the channel stream with `SftpSession`.
    pub async fn get_sftp_session(&self) -> Result<SftpSession, SandboxError> {
        let channel: Channel<Msg> = self.get_exec_channel().await?;

        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| SandboxError::SshChannel(format!("request sftp subsystem: {e}")))?;

        let stream = channel.into_stream();

        SftpSession::new(stream)
            .await
            .map_err(|e| SandboxError::SshChannel(format!("sftp session init: {e}")))
    }

    /// Open a direct-tcpip channel for TCP port forwarding.
    ///
    /// Creates a tunnel from the SSH connection to `host:port` on the
    /// remote side. The returned channel can be used to read/write
    /// tunneled TCP data.
    pub async fn get_direct_tcpip(
        &self,
        host: &str,
        port: u32,
    ) -> Result<Channel<Msg>, SandboxError> {
        let handle = self.handle.lock().await;
        handle
            .channel_open_direct_tcpip(host, port, "127.0.0.1", 0)
            .await
            .map_err(|e| {
                SandboxError::SshChannel(format!(
                    "open direct-tcpip to {host}:{port}: {e}"
                ))
            })
    }

    /// Gracefully disconnect from the SSH server.
    pub async fn disconnect(&self) -> Result<(), SandboxError> {
        self.healthy.store(false, Ordering::SeqCst);
        let handle = self.handle.lock().await;
        handle
            .disconnect(Disconnect::ByApplication, "closing", "en")
            .await
            .map_err(|e| SandboxError::SshConnection(format!("disconnect: {e}")))
    }

    /// Check whether the SSH connection is still healthy.
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::SeqCst)
    }

    /// Get a reference to the SSH config used for this pool.
    pub fn config(&self) -> &SshConfig {
        &self.config
    }
}
