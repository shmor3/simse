use std::path::PathBuf;
use std::sync::Arc;

use simse_vfs_engine::backend::FsBackend;
use simse_vfs_engine::disk::DiskFs;
use simse_vfs_engine::local_backend::LocalFsBackend;
use simse_vfs_engine::path::VfsLimits;
use simse_vfs_engine::vfs::VirtualFs;
use simse_vnet_engine::backend::NetBackend;
use simse_vnet_engine::local_backend::LocalNetBackend;
use simse_vnet_engine::network::{SandboxInit as VnetSandboxInit, VirtualNetwork};
use simse_vsh_engine::backend::ShellBackend;
use simse_vsh_engine::local_backend::LocalShellBackend;
use simse_vsh_engine::sandbox::SandboxConfig as VshSandboxConfig;
use simse_vsh_engine::shell::VirtualShell;

use crate::config::BackendConfig;
use crate::error::SandboxError;
use crate::ssh::fs_backend::SshFsBackend;
use crate::ssh::net_backend::SshNetBackend;
use crate::ssh::pool::SshPool;
use crate::ssh::shell_backend::SshShellBackend;

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
/// traits. Supports both local execution and remote execution over SSH.
/// Provides methods for health checks, disposal, and live backend switching.
pub struct Sandbox {
    backend_config: Option<BackendConfig>,
    ssh_pool: Option<Arc<SshPool>>,

    // VFS: in-memory VFS + separate disk/SSH backend for file:// paths
    vfs: Option<VirtualFs>,
    fs_backend: Option<Box<dyn FsBackend>>,

    // VSH: owns its backend internally
    vsh: Option<VirtualShell>,

    // VNet: owns its backend internally
    vnet: Option<VirtualNetwork>,

    // State
    initialized: bool,

    // Retained configs for engine re-creation on backend switch
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
            vnet: None,
            initialized: false,
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
        self.initialize_vnet(&config.vnet, &config.backend, &ssh_pool);

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

        // Rebuild the init config from stored values
        let init_config = InitConfig {
            backend: new_config.clone(),
            vfs: None, // VFS in-memory state is preserved
            vsh: self.vsh_init.clone(),
            vnet: self.vnet_init.clone(),
        };

        // Replace fs_backend
        self.fs_backend = Some(Self::create_fs_backend(&init_config, &ssh_pool)?);

        // Recreate VSH with new backend
        if let Some(ref vsh_cfg) = self.vsh_init {
            let shell_backend = Self::create_shell_backend(&new_config, &ssh_pool);
            self.vsh = Some(Self::build_vsh(vsh_cfg, shell_backend));
        }

        // Recreate VNet with new backend
        self.vnet = None;
        self.initialize_vnet(&self.vnet_init.clone(), &new_config, &ssh_pool);

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
        self.vnet = None;
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

    /// Mutably access the in-memory VFS.
    pub fn vfs_mut(&mut self) -> Result<&mut VirtualFs, SandboxError> {
        self.vfs.as_mut().ok_or(SandboxError::NotInitialized)
    }

    /// Access the filesystem backend (for `file://` paths).
    pub fn fs_backend(&self) -> Result<&dyn FsBackend, SandboxError> {
        self.fs_backend
            .as_deref()
            .ok_or(SandboxError::NotInitialized)
    }

    /// Access the virtual shell.
    pub fn vsh(&self) -> Result<&VirtualShell, SandboxError> {
        self.vsh.as_ref().ok_or(SandboxError::NotInitialized)
    }

    /// Mutably access the virtual shell.
    pub fn vsh_mut(&mut self) -> Result<&mut VirtualShell, SandboxError> {
        self.vsh.as_mut().ok_or(SandboxError::NotInitialized)
    }

    /// Access the virtual network.
    pub fn vnet(&self) -> Result<&VirtualNetwork, SandboxError> {
        self.vnet.as_ref().ok_or(SandboxError::NotInitialized)
    }

    /// Mutably access the virtual network.
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
            let shell_backend = Self::create_shell_backend(&config.backend, ssh_pool);
            self.vsh = Some(Self::build_vsh(vsh_cfg, shell_backend));
        }

        Ok(())
    }

    /// Create the appropriate FsBackend based on the backend config.
    fn create_fs_backend(
        config: &InitConfig,
        ssh_pool: &Option<Arc<SshPool>>,
    ) -> Result<Box<dyn FsBackend>, SandboxError> {
        // Use VFS init config for root/allowed paths, fall back to defaults
        let vfs_cfg = config.vfs.as_ref().cloned().unwrap_or_default();

        match &config.backend {
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
                Ok(Box::new(LocalFsBackend::new(disk)))
            }
            BackendConfig::Ssh(_) => {
                let pool = ssh_pool
                    .as_ref()
                    .ok_or_else(|| {
                        SandboxError::InvalidParams("SSH pool not available".into())
                    })?
                    .clone();
                Ok(Box::new(SshFsBackend::new(pool, vfs_cfg.root_directory)))
            }
        }
    }

    /// Create the appropriate ShellBackend based on the backend config.
    fn create_shell_backend(
        backend: &BackendConfig,
        ssh_pool: &Option<Arc<SshPool>>,
    ) -> Box<dyn ShellBackend> {
        match backend {
            BackendConfig::Local => Box::new(LocalShellBackend),
            BackendConfig::Ssh(_) => {
                let pool = ssh_pool
                    .as_ref()
                    .expect("SSH pool must be available for SSH backend")
                    .clone();
                Box::new(SshShellBackend::new(pool))
            }
        }
    }

    /// Create the appropriate NetBackend based on the backend config.
    fn create_net_backend(
        backend: &BackendConfig,
        ssh_pool: &Option<Arc<SshPool>>,
    ) -> Box<dyn NetBackend> {
        match backend {
            BackendConfig::Local => Box::new(LocalNetBackend::new()),
            BackendConfig::Ssh(_) => {
                let pool = ssh_pool
                    .as_ref()
                    .expect("SSH pool must be available for SSH backend")
                    .clone();
                Box::new(SshNetBackend::new(pool))
            }
        }
    }

    /// Build a VirtualShell from init config and a backend.
    fn build_vsh(cfg: &VshInitConfig, backend: Box<dyn ShellBackend>) -> VirtualShell {
        let sandbox_config = VshSandboxConfig {
            root_directory: PathBuf::from(&cfg.root_directory),
            allowed_paths: cfg.allowed_paths.iter().map(PathBuf::from).collect(),
            blocked_patterns: cfg.blocked_patterns.clone(),
            default_timeout_ms: cfg.default_timeout_ms,
            max_output_bytes: cfg.max_output_bytes,
            ..VshSandboxConfig::default()
        };
        VirtualShell::new(sandbox_config, cfg.shell.clone(), backend)
    }

    /// Initialize VNet engine with the appropriate backend.
    fn initialize_vnet(
        &mut self,
        vnet_cfg: &Option<VnetInitConfig>,
        backend: &BackendConfig,
        ssh_pool: &Option<Arc<SshPool>>,
    ) {
        if let Some(cfg) = vnet_cfg {
            let net_backend = Self::create_net_backend(backend, ssh_pool);
            let sandbox_init = VnetSandboxInit {
                allowed_hosts: cfg.allowed_hosts.clone(),
                allowed_ports: cfg.allowed_ports.clone(),
                allowed_protocols: cfg.allowed_protocols.clone(),
                default_timeout_ms: cfg.default_timeout_ms,
                max_response_bytes: cfg.max_response_bytes,
                max_connections: cfg.max_connections,
            };
            let mut vnet = VirtualNetwork::new();
            vnet.initialize(Some(sandbox_init), Some(net_backend));
            self.vnet = Some(vnet);
        }
    }
}

impl Default for Sandbox {
    fn default() -> Self {
        Self::new()
    }
}
