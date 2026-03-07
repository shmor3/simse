use serde::Deserialize;

use crate::sandbox::error::SandboxError;

// -- Protocol types (inlined from deleted protocol.rs) ------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendParams {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub ssh: Option<SshParams>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshParams {
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    pub auth: SshAuthParams,
    pub max_channels: Option<usize>,
    pub keepalive_interval_ms: Option<u64>,
    /// Expected server host key fingerprint (`SHA256:<base64>`).
    /// When set, connections are rejected if the server key doesn't match.
    pub host_key_fingerprint: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshAuthParams {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
    pub password: Option<String>,
}

// -- Parsed config types ------------------------------------------------------

#[derive(Debug, Clone)]
pub enum BackendConfig {
    Local,
    Ssh(SshConfig),
}

/// Policy for verifying SSH server host keys.
#[derive(Debug, Clone, Default)]
pub enum HostKeyPolicy {
    /// Accept all host keys without verification (INSECURE).
    /// Suitable only for trusted/internal networks.
    #[default]
    AcceptAll,
    /// Accept only a server whose public key matches this SHA256 fingerprint.
    /// Format: `"SHA256:<base64>"` (same as `ssh-keygen -lf`).
    Fingerprint(String),
}

#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth: SshAuth,
    pub max_channels: usize,
    pub keepalive_interval_ms: u64,
    pub host_key_policy: HostKeyPolicy,
}

#[derive(Debug, Clone)]
pub enum SshAuth {
    Key {
        private_key_path: String,
        passphrase: Option<String>,
    },
    Password {
        password: String,
    },
    Agent,
}

// -- Conversion from protocol types -------------------------------------------

impl BackendConfig {
    pub fn from_params(params: &BackendParams) -> Result<Self, SandboxError> {
        match params.backend_type.as_str() {
            "local" => Ok(BackendConfig::Local),
            "ssh" => {
                let ssh_params = params
                    .ssh
                    .as_ref()
                    .ok_or_else(|| SandboxError::InvalidParams("ssh backend requires ssh params".into()))?;
                let ssh_config = SshConfig::from_params(ssh_params)?;
                Ok(BackendConfig::Ssh(ssh_config))
            }
            other => Err(SandboxError::InvalidParams(format!(
                "unknown backend type: {other}"
            ))),
        }
    }
}

impl SshConfig {
    pub fn from_params(params: &SshParams) -> Result<Self, SandboxError> {
        if params.host.is_empty() {
            return Err(SandboxError::InvalidParams("ssh host cannot be empty".into()));
        }
        if params.username.is_empty() {
            return Err(SandboxError::InvalidParams(
                "ssh username cannot be empty".into(),
            ));
        }

        let auth = match params.auth.auth_type.as_str() {
            "key" => {
                let private_key_path = params
                    .auth
                    .private_key_path
                    .as_ref()
                    .ok_or_else(|| {
                        SandboxError::InvalidParams("key auth requires privateKeyPath".into())
                    })?
                    .clone();
                SshAuth::Key {
                    private_key_path,
                    passphrase: params.auth.passphrase.clone(),
                }
            }
            "password" => {
                let password = params
                    .auth
                    .password
                    .as_ref()
                    .ok_or_else(|| {
                        SandboxError::InvalidParams("password auth requires password".into())
                    })?
                    .clone();
                SshAuth::Password { password }
            }
            "agent" => SshAuth::Agent,
            other => {
                return Err(SandboxError::InvalidParams(format!(
                    "unknown ssh auth type: {other}"
                )));
            }
        };

        let host_key_policy = match &params.host_key_fingerprint {
            Some(fp) => HostKeyPolicy::Fingerprint(fp.clone()),
            None => HostKeyPolicy::AcceptAll,
        };

        Ok(SshConfig {
            host: params.host.clone(),
            port: params.port.unwrap_or(22),
            username: params.username.clone(),
            auth,
            max_channels: params.max_channels.unwrap_or(10),
            keepalive_interval_ms: params.keepalive_interval_ms.unwrap_or(15_000),
            host_key_policy,
        })
    }
}
