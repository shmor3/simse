use std::path::PathBuf;
use std::sync::Arc;

use crate::sandbox::config::BackendConfig;
use crate::sandbox::error::SandboxError;
use crate::sandbox::ssh::fs::SshFs;
use crate::sandbox::ssh::net::SshNet;
use crate::sandbox::ssh::pool::SshPool;
use crate::sandbox::ssh::shell::SshShell;
use crate::sandbox::vfs_backend::FsImpl;
use crate::sandbox::vfs_disk::DiskFs;
use crate::sandbox::vfs_path::VfsLimits;
use crate::sandbox::vfs_store::VirtualFs;
use crate::sandbox::vnet_backend::NetImpl;
use crate::sandbox::vnet_local::LocalNet;
use crate::sandbox::vnet_network::{SandboxInit as VnetSandboxInit, VirtualNetwork};
use crate::sandbox::vsh_backend::{LocalShell, ShellImpl};
use crate::sandbox::vsh_sandbox::SandboxConfig as VshSandboxConfig;
use crate::sandbox::vsh_shell::VirtualShell;

// ── Init config types ──────────────────────────────────────────────────────

/// Top-level configuration for initializing a [`Sandbox`].
#[derive(Debug, Clone)]
pub struct InitConfig {
    pub backend: BackendConfig,
    pub vfs: Option<VfsInitConfig>,
    pub vsh: Option<VshInitConfig>,
    pub vnet: Option<VnetInitConfig>,
}

/// VFS initialization parameters.
///
/// Controls the in-memory VFS limits and the disk-backed filesystem
/// root and allowed paths.
#[derive(Debug, Clone)]
pub struct VfsInitConfig {
    pub root_directory: String,
    pub allowed_paths: Vec<String>,
    pub max_history: usize,
    pub limits: VfsLimits,
}

impl Default for VfsInitConfig {
    fn default() -> Self {
        Self {
            root_directory: ".".to_string(),
            allowed_paths: Vec::new(),
            max_history: 50,
            limits: VfsLimits::default(),
        }
    }
}

/// VSH (virtual shell) initialization parameters.
#[derive(Debug, Clone)]
pub struct VshInitConfig {
    pub root_directory: String,
    pub allowed_paths: Vec<String>,
    pub blocked_patterns: Vec<String>,
    pub shell: String,
    pub default_timeout_ms: u64,
    pub max_output_bytes: usize,
}

impl Default for VshInitConfig {
    fn default() -> Self {
        Self {
            root_directory: ".".to_string(),
            allowed_paths: Vec::new(),
            blocked_patterns: Vec::new(),
            shell: "sh".to_string(),
            default_timeout_ms: 120_000,
            max_output_bytes: 50_000,
        }
    }
}

/// VNet (virtual network) initialization parameters.
#[derive(Debug, Clone)]
pub struct VnetInitConfig {
    pub allowed_hosts: Vec<String>,
    pub allowed_ports: Vec<(u16, u16)>,
    pub allowed_protocols: Vec<String>,
    pub default_timeout_ms: u64,
    pub max_response_bytes: u64,
    pub max_connections: usize,
}

impl Default for VnetInitConfig {
    fn default() -> Self {
        Self {
            allowed_hosts: vec!["*".to_string()],
            allowed_ports: vec![(1, 65535)],
            allowed_protocols: vec![
                "http".to_string(),
                "https".to_string(),
                "ws".to_string(),
                "wss".to_string(),
                "tcp".to_string(),
                "udp".to_string(),
            ],
            default_timeout_ms: 30_000,
            max_response_bytes: 10 * 1024 * 1024,
            max_connections: 100,
        }
    }
}

// ── Sandbox ────────────────────────────────────────────────────────────────

/// Unified sandbox orchestrator.
///
/// Creates and manages VFS, VSH, and VNet engines behind backend-agnostic
/// enum dispatch. Supports both local execution and remote execution over SSH.
/// Provides methods for health checks, disposal, and live backend switching.
pub struct Sandbox {
    backend_config: Option<BackendConfig>,
    ssh_pool: Option<Arc<SshPool>>,

    // VFS: in-memory VFS + separate disk/SSH backend for file:// paths
    vfs: Option<VirtualFs>,
    fs_backend: Option<FsImpl>,

    // VSH: pure state + separate backend for I/O
    vsh: Option<VirtualShell>,
    shell_backend: Option<ShellImpl>,

    // VNet: owns its backend internally (backend stored inside VirtualNetwork)
    vnet: Option<VirtualNetwork>,

    // State
    initialized: bool,

    // Retained configs for engine re-creation on backend switch
    vfs_init: Option<VfsInitConfig>,
    vsh_init: Option<VshInitConfig>,
    vnet_init: Option<VnetInitConfig>,
}

impl Sandbox {
    /// Create a new uninitialized sandbox.
    pub fn new() -> Self {
        Self {
            backend_config: None,
            ssh_pool: None,
            vfs: None,
            fs_backend: None,
            vsh: None,
            shell_backend: None,
            vnet: None,
            initialized: false,
            vfs_init: None,
            vsh_init: None,
            vnet_init: None,
        }
    }

    /// Initialize the sandbox with the given configuration.
    ///
    /// Creates the appropriate backends (local or SSH) and initializes
    /// all requested engines (VFS, VSH, VNet).
    pub async fn initialize(&mut self, config: InitConfig) -> Result<(), SandboxError> {
        if self.initialized {
            return Err(SandboxError::InvalidParams(
                "sandbox already initialized".into(),
            ));
        }

        // Store init configs for backend switching
        self.vfs_init = config.vfs.clone();
        self.vsh_init = config.vsh.clone();
        self.vnet_init = config.vnet.clone();

        // Connect SSH if needed
        let ssh_pool = match &config.backend {
            BackendConfig::Local => None,
            BackendConfig::Ssh(ssh_config) => {
                let pool = SshPool::connect(ssh_config).await?;
                Some(Arc::new(pool))
            }
        };

        // Create backends and engines
        self.create_backends_and_engines(&config, &ssh_pool)?;

        // Initialize VNet if present
        if let Some(ref vnet_cfg) = config.vnet {
            let net_backend = Self::create_net_backend(&config.backend, &ssh_pool)?;
            self.vnet = Some(Self::build_vnet(vnet_cfg, net_backend));
        }

        self.backend_config = Some(config.backend);
        self.ssh_pool = ssh_pool;
        self.initialized = true;

        tracing::info!("sandbox initialized");
        Ok(())
    }

    /// Switch the sandbox to a different backend.
    ///
    /// Disconnects the old SSH pool (if any), creates new backends, and
    /// re-creates VirtualShell and VirtualNetwork with the new backends.
    /// The in-memory VFS is preserved; only the `fs_backend` is replaced.
    pub async fn switch_backend(
        &mut self,
        new_config: BackendConfig,
    ) -> Result<(), SandboxError> {
        if !self.initialized {
            return Err(SandboxError::NotInitialized);
        }

        // Disconnect old SSH pool
        if let Some(ref pool) = self.ssh_pool {
            if let Err(e) = pool.disconnect().await {
                tracing::warn!("error disconnecting old SSH pool: {e}");
            }
        }

        // Connect new SSH if needed
        let ssh_pool = match &new_config {
            BackendConfig::Local => None,
            BackendConfig::Ssh(ssh_config) => {
                let pool = SshPool::connect(ssh_config)
                    .await
                    .map_err(|e| SandboxError::BackendSwitch(format!("SSH connect: {e}")))?;
                Some(Arc::new(pool))
            }
        };

        // Replace fs_backend using stored VFS config
        if self.fs_backend.is_some() {
            let vfs_cfg = self.vfs_init.clone().unwrap_or_default();
            self.fs_backend =
                Some(Self::create_fs_backend_from_cfg(&vfs_cfg, &new_config, &ssh_pool)?);
        }

        // Recreate VSH with new backend
        if let Some(ref vsh_cfg) = self.vsh_init {
            let shell_backend = Self::create_shell_backend(&new_config, &ssh_pool)?;
            self.vsh = Some(Self::build_vsh(vsh_cfg));
            self.shell_backend = Some(shell_backend);
        }

        // Recreate VNet with new backend
        if let Some(ref vnet_cfg) = self.vnet_init.clone() {
            let net_backend = Self::create_net_backend(&new_config, &ssh_pool)?;
            self.vnet = Some(Self::build_vnet(vnet_cfg, net_backend));
        }

        self.backend_config = Some(new_config);
        self.ssh_pool = ssh_pool;

        tracing::info!("sandbox backend switched");
        Ok(())
    }

    /// Return a health report as JSON.
    pub fn health(&self) -> Result<serde_json::Value, SandboxError> {
        let backend_type = match &self.backend_config {
            Some(BackendConfig::Local) => "local",
            Some(BackendConfig::Ssh(_)) => "ssh",
            None => "none",
        };

        let ssh_healthy = self
            .ssh_pool
            .as_ref()
            .map(|pool| pool.is_healthy());

        Ok(serde_json::json!({
            "initialized": self.initialized,
            "backendType": backend_type,
            "sshHealthy": ssh_healthy,
            "engines": {
                "vfs": self.vfs.is_some(),
                "fsBackend": self.fs_backend.is_some(),
                "vsh": self.vsh.is_some(),
                "vnet": self.vnet.is_some(),
            }
        }))
    }

    /// Dispose the sandbox, disconnecting SSH and clearing all engines.
    pub async fn dispose(&mut self) -> Result<(), SandboxError> {
        // Disconnect SSH pool
        if let Some(ref pool) = self.ssh_pool {
            if let Err(e) = pool.disconnect().await {
                tracing::warn!("error disconnecting SSH pool during dispose: {e}");
            }
        }

        self.ssh_pool = None;
        self.backend_config = None;
        self.vfs = None;
        self.fs_backend = None;
        self.vsh = None;
        self.shell_backend = None;
        self.vnet = None;
        self.vfs_init = None;
        self.vsh_init = None;
        self.vnet_init = None;
        self.initialized = false;

        tracing::info!("sandbox disposed");
        Ok(())
    }

    // ── Accessor methods ───────────────────────────────────────────────

    /// Access the in-memory VFS.
    pub fn vfs(&self) -> Result<&VirtualFs, SandboxError> {
        self.vfs.as_ref().ok_or(SandboxError::NotInitialized)
    }

    /// Take ownership of the in-memory VFS for a state transition.
    ///
    /// The caller must put the updated VFS back via [`vfs_set`].
    pub fn vfs_take(&mut self) -> Result<VirtualFs, SandboxError> {
        self.vfs.take().ok_or(SandboxError::NotInitialized)
    }

    /// Put an updated VFS back after a state transition.
    pub fn vfs_set(&mut self, vfs: VirtualFs) {
        self.vfs = Some(vfs);
    }

    /// Access the filesystem backend (for `file://` paths).
    pub fn fs_backend(&self) -> Result<&FsImpl, SandboxError> {
        self.fs_backend
            .as_ref()
            .ok_or(SandboxError::NotInitialized)
    }

    /// Access the virtual shell (read-only).
    pub fn vsh(&self) -> Result<&VirtualShell, SandboxError> {
        self.vsh.as_ref().ok_or(SandboxError::NotInitialized)
    }

    /// Take ownership of the virtual shell for a state transition.
    ///
    /// The caller must put the updated shell back via [].
    pub fn vsh_take(&mut self) -> Result<VirtualShell, SandboxError> {
        self.vsh.take().ok_or(SandboxError::NotInitialized)
    }

    /// Put an updated virtual shell back after a state transition.
    pub fn vsh_set(&mut self, vsh: VirtualShell) {
        self.vsh = Some(vsh);
    }

    /// Access the shell backend (for I/O execution).
    pub fn shell_backend(&self) -> Result<&ShellImpl, SandboxError> {
        self.shell_backend
            .as_ref()
            .ok_or(SandboxError::NotInitialized)
    }

    /// Access the virtual network.
    pub fn vnet(&self) -> Result<&VirtualNetwork, SandboxError> {
        self.vnet.as_ref().ok_or(SandboxError::NotInitialized)
    }

    /// Take ownership of the virtual network for a state transition.
    ///
    /// The caller must put the updated network back via [`vnet_set`].
    pub fn vnet_take(&mut self) -> Result<VirtualNetwork, SandboxError> {
        self.vnet.take().ok_or(SandboxError::NotInitialized)
    }

    /// Put an updated virtual network back after a state transition.
    pub fn vnet_set(&mut self, vnet: VirtualNetwork) {
        self.vnet = Some(vnet);
    }

    /// Mutably access the virtual network for I/O hot-path methods
    /// (e.g. `net_http_request`) that require `&mut self` on VirtualNetwork.
    // PERF: hot-path I/O -- needed for backend-delegated async methods
    pub fn vnet_mut(&mut self) -> Result<&mut VirtualNetwork, SandboxError> {
        self.vnet.as_mut().ok_or(SandboxError::NotInitialized)
    }

    /// Check if the sandbox is initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the current backend configuration.
    pub fn backend_config(&self) -> Option<&BackendConfig> {
        self.backend_config.as_ref()
    }

    // ── Internal helpers ───────────────────────────────────────────────

    /// Create all backends and engines from the init config.
    fn create_backends_and_engines(
        &mut self,
        config: &InitConfig,
        ssh_pool: &Option<Arc<SshPool>>,
    ) -> Result<(), SandboxError> {
        // VFS (in-memory) + fs_backend (disk or SSH)
        if let Some(ref vfs_cfg) = config.vfs {
            self.vfs = Some(VirtualFs::new(
                vfs_cfg.limits.clone(),
                vfs_cfg.max_history,
            ));
            self.fs_backend = Some(Self::create_fs_backend(config, ssh_pool)?);
        }

        // VSH
        if let Some(ref vsh_cfg) = config.vsh {
            let shell_backend = Self::create_shell_backend(&config.backend, ssh_pool)?;
            self.vsh = Some(Self::build_vsh(vsh_cfg));
            self.shell_backend = Some(shell_backend);
        }

        Ok(())
    }

    /// Create the appropriate FsImpl from init config.
    fn create_fs_backend(
        config: &InitConfig,
        ssh_pool: &Option<Arc<SshPool>>,
    ) -> Result<FsImpl, SandboxError> {
        let vfs_cfg = config.vfs.as_ref().cloned().unwrap_or_default();
        Self::create_fs_backend_from_cfg(&vfs_cfg, &config.backend, ssh_pool)
    }

    /// Create the appropriate FsImpl from explicit config.
    fn create_fs_backend_from_cfg(
        vfs_cfg: &VfsInitConfig,
        backend: &BackendConfig,
        ssh_pool: &Option<Arc<SshPool>>,
    ) -> Result<FsImpl, SandboxError> {
        match backend {
            BackendConfig::Local => {
                let disk = DiskFs::new(
                    PathBuf::from(&vfs_cfg.root_directory),
                    vfs_cfg
                        .allowed_paths
                        .iter()
                        .map(PathBuf::from)
                        .collect(),
                    vfs_cfg.max_history,
                );
                Ok(FsImpl::Local(disk))
            }
            BackendConfig::Ssh(_) => {
                let pool = Self::require_ssh_pool(ssh_pool)?;
                Ok(FsImpl::Ssh(SshFs::new(pool, vfs_cfg.root_directory.clone())))
            }
        }
    }

    /// Create the appropriate ShellImpl based on the backend config.
    fn create_shell_backend(
        backend: &BackendConfig,
        ssh_pool: &Option<Arc<SshPool>>,
    ) -> Result<ShellImpl, SandboxError> {
        match backend {
            BackendConfig::Local => Ok(ShellImpl::Local(LocalShell)),
            BackendConfig::Ssh(_) => {
                let pool = Self::require_ssh_pool(ssh_pool)?;
                Ok(ShellImpl::Ssh(SshShell::new(pool)))
            }
        }
    }

    /// Create the appropriate NetImpl based on the backend config.
    fn create_net_backend(
        backend: &BackendConfig,
        ssh_pool: &Option<Arc<SshPool>>,
    ) -> Result<NetImpl, SandboxError> {
        match backend {
            BackendConfig::Local => Ok(NetImpl::Local(LocalNet::new())),
            BackendConfig::Ssh(_) => {
                let pool = Self::require_ssh_pool(ssh_pool)?;
                Ok(NetImpl::Ssh(SshNet::new(pool)))
            }
        }
    }

    /// Get the SSH pool, returning an error if not available.
    fn require_ssh_pool(
        ssh_pool: &Option<Arc<SshPool>>,
    ) -> Result<Arc<SshPool>, SandboxError> {
        ssh_pool
            .as_ref()
            .cloned()
            .ok_or_else(|| SandboxError::InvalidParams("SSH pool not available".into()))
    }

    /// Build a VirtualShell from init config (pure state, no backend).
    fn build_vsh(cfg: &VshInitConfig) -> VirtualShell {
        let sandbox_config = VshSandboxConfig {
            root_directory: PathBuf::from(&cfg.root_directory),
            allowed_paths: cfg.allowed_paths.iter().map(PathBuf::from).collect(),
            blocked_patterns: cfg.blocked_patterns.clone(),
            default_timeout_ms: cfg.default_timeout_ms,
            max_output_bytes: cfg.max_output_bytes,
            ..VshSandboxConfig::default()
        };
        VirtualShell::new(sandbox_config, cfg.shell.clone())
    }

    /// Build a VirtualNetwork from init config and a backend.
    fn build_vnet(cfg: &VnetInitConfig, net_backend: NetImpl) -> VirtualNetwork {
        let sandbox_init = VnetSandboxInit {
            allowed_hosts: cfg.allowed_hosts.clone(),
            allowed_ports: cfg.allowed_ports.clone(),
            allowed_protocols: cfg.allowed_protocols.clone(),
            default_timeout_ms: cfg.default_timeout_ms,
            max_response_bytes: cfg.max_response_bytes,
            max_connections: cfg.max_connections,
        };
        VirtualNetwork::new().initialize(Some(sandbox_init), Some(net_backend))
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}
