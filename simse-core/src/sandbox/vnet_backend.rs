use std::collections::HashMap;

use crate::sandbox::error::SandboxError;
use crate::sandbox::ssh::net::SshNet;
use crate::sandbox::vnet_local::LocalNet;
use crate::sandbox::vnet_types::HttpResponseResult;

/// Unified network backend — dispatches to local reqwest/tokio or SSH.
pub enum NetImpl {
    Local(LocalNet),
    Ssh(SshNet),
}

impl NetImpl {
    pub async fn http_request(
        &self,
        url: &str,
        method: &str,
        headers: &HashMap<String, String>,
        body: Option<&str>,
        timeout_ms: u64,
        max_response_bytes: u64,
    ) -> Result<HttpResponseResult, SandboxError> {
        match self {
            Self::Local(net) => {
                net.http_request(url, method, headers, body, timeout_ms, max_response_bytes)
                    .await
            }
            Self::Ssh(net) => {
                net.http_request(url, method, headers, body, timeout_ms, max_response_bytes)
                    .await
            }
        }
    }

    pub async fn ws_connect(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
    ) -> Result<String, SandboxError> {
        match self {
            Self::Local(net) => net.ws_connect(url, headers).await,
            Self::Ssh(net) => net.ws_connect(url, headers).await,
        }
    }

    pub async fn ws_send(&self, session_id: &str, data: &str) -> Result<(), SandboxError> {
        match self {
            Self::Local(net) => net.ws_send(session_id, data).await,
            Self::Ssh(net) => net.ws_send(session_id, data).await,
        }
    }

    pub async fn ws_close(&self, session_id: &str) -> Result<(), SandboxError> {
        match self {
            Self::Local(net) => net.ws_close(session_id).await,
            Self::Ssh(net) => net.ws_close(session_id).await,
        }
    }

    pub async fn tcp_connect(
        &self,
        host: &str,
        port: u16,
    ) -> Result<String, SandboxError> {
        match self {
            Self::Local(net) => net.tcp_connect(host, port).await,
            Self::Ssh(net) => net.tcp_connect(host, port).await,
        }
    }

    pub async fn tcp_send(&self, session_id: &str, data: &str) -> Result<(), SandboxError> {
        match self {
            Self::Local(net) => net.tcp_send(session_id, data).await,
            Self::Ssh(net) => net.tcp_send(session_id, data).await,
        }
    }

    pub async fn tcp_close(&self, session_id: &str) -> Result<(), SandboxError> {
        match self {
            Self::Local(net) => net.tcp_close(session_id).await,
            Self::Ssh(net) => net.tcp_close(session_id).await,
        }
    }

    pub async fn udp_send(
        &self,
        host: &str,
        port: u16,
        data: &str,
        timeout_ms: u64,
    ) -> Result<Option<String>, SandboxError> {
        match self {
            Self::Local(net) => net.udp_send(host, port, data, timeout_ms).await,
            Self::Ssh(net) => net.udp_send(host, port, data, timeout_ms).await,
        }
    }

    pub async fn resolve(&self, hostname: &str) -> Result<Vec<String>, SandboxError> {
        match self {
            Self::Local(net) => net.resolve(hostname).await,
            Self::Ssh(net) => net.resolve(hostname).await,
        }
    }
}
