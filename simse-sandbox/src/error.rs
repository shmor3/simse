use simse_vfs_engine::error::VfsError;
use simse_vnet_engine::error::VnetError;
use simse_vsh_engine::error::VshError;
use thiserror::Error;

/// Unified error type for the sandbox engine.
///
/// Contains:
/// - Sandbox-level variants (SSH, backend switching, etc.)
/// - Inlined VFS domain variants (`Vfs*` prefix)
/// - Inlined VSH domain variants (`Vsh*` prefix)
/// - Inlined VNet domain variants (`Vnet*` prefix)
/// - Temporary wrapper variants (`Vfs`, `Vsh`, `Vnet`) kept so existing code
///   that converts from the old per-crate error types still compiles. These
///   will be removed once all domain code is migrated into simse-sandbox.
#[derive(Debug, Error)]
pub enum SandboxError {
    // ── Sandbox-level ────────────────────────────────────────────────
    #[error("Not initialized")]
    NotInitialized,
    #[error("SSH connection error: {0}")]
    SshConnection(String),
    #[error("SSH authentication error: {0}")]
    SshAuth(String),
    #[error("SSH channel error: {0}")]
    SshChannel(String),
    #[error("Backend switch error: {0}")]
    BackendSwitch(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    // ── VFS domain (inlined) ─────────────────────────────────────────
    #[error("VFS invalid path: {0}")]
    VfsInvalidPath(String),
    #[error("VFS not found: {0}")]
    VfsNotFound(String),
    #[error("VFS already exists: {0}")]
    VfsAlreadyExists(String),
    #[error("VFS not a file: {0}")]
    VfsNotAFile(String),
    #[error("VFS not a directory: {0}")]
    VfsNotADirectory(String),
    #[error("VFS not empty: {0}")]
    VfsNotEmpty(String),
    #[error("VFS limit exceeded: {0}")]
    VfsLimitExceeded(String),
    #[error("VFS invalid operation: {0}")]
    VfsInvalidOperation(String),
    #[error("VFS permission denied: {0}")]
    VfsPermissionDenied(String),
    #[error("VFS disk not configured")]
    VfsDiskNotConfigured,
    #[error("VFS IO error: {0}")]
    VfsIo(String),
    #[error("VFS JSON error: {0}")]
    VfsJson(String),

    // ── VSH domain (inlined) ─────────────────────────────────────────
    #[error("VSH not initialized")]
    VshNotInitialized,
    #[error("VSH session not found: {0}")]
    VshSessionNotFound(String),
    #[error("VSH execution failed: {0}")]
    VshExecutionFailed(String),
    #[error("VSH command timeout: {0}")]
    VshTimeout(String),
    #[error("VSH sandbox violation: {0}")]
    VshSandboxViolation(String),
    #[error("VSH invalid params: {0}")]
    VshInvalidParams(String),
    #[error("VSH limit exceeded: {0}")]
    VshLimitExceeded(String),
    #[error("VSH IO error: {0}")]
    VshIo(String),
    #[error("VSH JSON error: {0}")]
    VshJson(String),

    // ── VNet domain (inlined) ────────────────────────────────────────
    #[error("VNet not initialized")]
    VnetNotInitialized,
    #[error("VNet sandbox violation: {0}")]
    VnetSandboxViolation(String),
    #[error("VNet connection failed: {0}")]
    VnetConnectionFailed(String),
    #[error("VNet timeout: {0}")]
    VnetTimeout(String),
    #[error("VNet session not found: {0}")]
    VnetSessionNotFound(String),
    #[error("VNet mock not found: {0}")]
    VnetMockNotFound(String),
    #[error("VNet no mock match: {0}")]
    VnetNoMockMatch(String),
    #[error("VNet limit exceeded: {0}")]
    VnetLimitExceeded(String),
    #[error("VNet invalid params: {0}")]
    VnetInvalidParams(String),
    #[error("VNet response too large: {0}")]
    VnetResponseTooLarge(String),
    #[error("VNet DNS resolution failed: {0}")]
    VnetDnsResolutionFailed(String),
    #[error("VNet IO error: {0}")]
    VnetIo(String),
    #[error("VNet JSON error: {0}")]
    VnetJson(String),

    // ── Temporary wrappers (will be removed after migration) ─────────
    #[error("VFS error: {0}")]
    Vfs(#[from] VfsError),
    #[error("VSH error: {0}")]
    Vsh(#[from] VshError),
    #[error("VNet error: {0}")]
    Vnet(#[from] VnetError),
}

impl SandboxError {
    pub fn code(&self) -> &str {
        match self {
            // Sandbox-level
            Self::NotInitialized => "SANDBOX_NOT_INITIALIZED",
            Self::SshConnection(_) => "SANDBOX_SSH_CONNECTION",
            Self::SshAuth(_) => "SANDBOX_SSH_AUTH",
            Self::SshChannel(_) => "SANDBOX_SSH_CHANNEL",
            Self::BackendSwitch(_) => "SANDBOX_BACKEND_SWITCH",
            Self::Timeout(_) => "SANDBOX_TIMEOUT",
            Self::InvalidParams(_) => "SANDBOX_INVALID_PARAMS",
            Self::Io(_) => "SANDBOX_IO_ERROR",
            Self::Json(_) => "SANDBOX_JSON_ERROR",

            // VFS domain (inlined)
            Self::VfsInvalidPath(_) => "SANDBOX_VFS_INVALID_PATH",
            Self::VfsNotFound(_) => "SANDBOX_VFS_NOT_FOUND",
            Self::VfsAlreadyExists(_) => "SANDBOX_VFS_ALREADY_EXISTS",
            Self::VfsNotAFile(_) => "SANDBOX_VFS_NOT_FILE",
            Self::VfsNotADirectory(_) => "SANDBOX_VFS_NOT_DIRECTORY",
            Self::VfsNotEmpty(_) => "SANDBOX_VFS_NOT_EMPTY",
            Self::VfsLimitExceeded(_) => "SANDBOX_VFS_LIMIT_EXCEEDED",
            Self::VfsInvalidOperation(_) => "SANDBOX_VFS_INVALID_OPERATION",
            Self::VfsPermissionDenied(_) => "SANDBOX_VFS_PERMISSION_DENIED",
            Self::VfsDiskNotConfigured => "SANDBOX_VFS_DISK_NOT_CONFIGURED",
            Self::VfsIo(_) => "SANDBOX_VFS_IO_ERROR",
            Self::VfsJson(_) => "SANDBOX_VFS_JSON_ERROR",

            // VSH domain (inlined)
            Self::VshNotInitialized => "SANDBOX_VSH_NOT_INITIALIZED",
            Self::VshSessionNotFound(_) => "SANDBOX_VSH_SESSION_NOT_FOUND",
            Self::VshExecutionFailed(_) => "SANDBOX_VSH_EXECUTION_FAILED",
            Self::VshTimeout(_) => "SANDBOX_VSH_TIMEOUT",
            Self::VshSandboxViolation(_) => "SANDBOX_VSH_SANDBOX_VIOLATION",
            Self::VshInvalidParams(_) => "SANDBOX_VSH_INVALID_PARAMS",
            Self::VshLimitExceeded(_) => "SANDBOX_VSH_LIMIT_EXCEEDED",
            Self::VshIo(_) => "SANDBOX_VSH_IO_ERROR",
            Self::VshJson(_) => "SANDBOX_VSH_JSON_ERROR",

            // VNet domain (inlined)
            Self::VnetNotInitialized => "SANDBOX_VNET_NOT_INITIALIZED",
            Self::VnetSandboxViolation(_) => "SANDBOX_VNET_SANDBOX_VIOLATION",
            Self::VnetConnectionFailed(_) => "SANDBOX_VNET_CONNECTION_FAILED",
            Self::VnetTimeout(_) => "SANDBOX_VNET_TIMEOUT",
            Self::VnetSessionNotFound(_) => "SANDBOX_VNET_SESSION_NOT_FOUND",
            Self::VnetMockNotFound(_) => "SANDBOX_VNET_MOCK_NOT_FOUND",
            Self::VnetNoMockMatch(_) => "SANDBOX_VNET_NO_MOCK_MATCH",
            Self::VnetLimitExceeded(_) => "SANDBOX_VNET_LIMIT_EXCEEDED",
            Self::VnetInvalidParams(_) => "SANDBOX_VNET_INVALID_PARAMS",
            Self::VnetResponseTooLarge(_) => "SANDBOX_VNET_RESPONSE_TOO_LARGE",
            Self::VnetDnsResolutionFailed(_) => "SANDBOX_VNET_DNS_FAILED",
            Self::VnetIo(_) => "SANDBOX_VNET_IO_ERROR",
            Self::VnetJson(_) => "SANDBOX_VNET_JSON_ERROR",

            // Temporary wrappers — delegate to the old per-crate code()
            Self::Vfs(e) => e.code(),
            Self::Vsh(e) => e.code(),
            Self::Vnet(e) => e.code(),
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "sandboxCode": self.code(),
            "message": self.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_level_codes() {
        assert_eq!(SandboxError::NotInitialized.code(), "SANDBOX_NOT_INITIALIZED");
        assert_eq!(SandboxError::SshConnection("x".into()).code(), "SANDBOX_SSH_CONNECTION");
        assert_eq!(SandboxError::SshAuth("x".into()).code(), "SANDBOX_SSH_AUTH");
        assert_eq!(SandboxError::SshChannel("x".into()).code(), "SANDBOX_SSH_CHANNEL");
        assert_eq!(SandboxError::BackendSwitch("x".into()).code(), "SANDBOX_BACKEND_SWITCH");
        assert_eq!(SandboxError::Timeout("x".into()).code(), "SANDBOX_TIMEOUT");
        assert_eq!(SandboxError::InvalidParams("x".into()).code(), "SANDBOX_INVALID_PARAMS");
    }

    #[test]
    fn vfs_inlined_codes() {
        let cases = vec![
            (SandboxError::VfsInvalidPath("p".into()), "SANDBOX_VFS_INVALID_PATH"),
            (SandboxError::VfsNotFound("p".into()), "SANDBOX_VFS_NOT_FOUND"),
            (SandboxError::VfsAlreadyExists("p".into()), "SANDBOX_VFS_ALREADY_EXISTS"),
            (SandboxError::VfsNotAFile("p".into()), "SANDBOX_VFS_NOT_FILE"),
            (SandboxError::VfsNotADirectory("p".into()), "SANDBOX_VFS_NOT_DIRECTORY"),
            (SandboxError::VfsNotEmpty("p".into()), "SANDBOX_VFS_NOT_EMPTY"),
            (SandboxError::VfsLimitExceeded("p".into()), "SANDBOX_VFS_LIMIT_EXCEEDED"),
            (SandboxError::VfsInvalidOperation("p".into()), "SANDBOX_VFS_INVALID_OPERATION"),
            (SandboxError::VfsPermissionDenied("p".into()), "SANDBOX_VFS_PERMISSION_DENIED"),
            (SandboxError::VfsDiskNotConfigured, "SANDBOX_VFS_DISK_NOT_CONFIGURED"),
            (SandboxError::VfsIo("p".into()), "SANDBOX_VFS_IO_ERROR"),
            (SandboxError::VfsJson("p".into()), "SANDBOX_VFS_JSON_ERROR"),
        ];
        for (err, expected) in cases {
            assert_eq!(err.code(), expected, "wrong code for {err}");
        }
    }

    #[test]
    fn vsh_inlined_codes() {
        let cases = vec![
            (SandboxError::VshNotInitialized, "SANDBOX_VSH_NOT_INITIALIZED"),
            (SandboxError::VshSessionNotFound("s".into()), "SANDBOX_VSH_SESSION_NOT_FOUND"),
            (SandboxError::VshExecutionFailed("s".into()), "SANDBOX_VSH_EXECUTION_FAILED"),
            (SandboxError::VshTimeout("s".into()), "SANDBOX_VSH_TIMEOUT"),
            (SandboxError::VshSandboxViolation("s".into()), "SANDBOX_VSH_SANDBOX_VIOLATION"),
            (SandboxError::VshInvalidParams("s".into()), "SANDBOX_VSH_INVALID_PARAMS"),
            (SandboxError::VshLimitExceeded("s".into()), "SANDBOX_VSH_LIMIT_EXCEEDED"),
            (SandboxError::VshIo("s".into()), "SANDBOX_VSH_IO_ERROR"),
            (SandboxError::VshJson("s".into()), "SANDBOX_VSH_JSON_ERROR"),
        ];
        for (err, expected) in cases {
            assert_eq!(err.code(), expected, "wrong code for {err}");
        }
    }

    #[test]
    fn vnet_inlined_codes() {
        let cases = vec![
            (SandboxError::VnetNotInitialized, "SANDBOX_VNET_NOT_INITIALIZED"),
            (SandboxError::VnetSandboxViolation("v".into()), "SANDBOX_VNET_SANDBOX_VIOLATION"),
            (SandboxError::VnetConnectionFailed("v".into()), "SANDBOX_VNET_CONNECTION_FAILED"),
            (SandboxError::VnetTimeout("v".into()), "SANDBOX_VNET_TIMEOUT"),
            (SandboxError::VnetSessionNotFound("v".into()), "SANDBOX_VNET_SESSION_NOT_FOUND"),
            (SandboxError::VnetMockNotFound("v".into()), "SANDBOX_VNET_MOCK_NOT_FOUND"),
            (SandboxError::VnetNoMockMatch("v".into()), "SANDBOX_VNET_NO_MOCK_MATCH"),
            (SandboxError::VnetLimitExceeded("v".into()), "SANDBOX_VNET_LIMIT_EXCEEDED"),
            (SandboxError::VnetInvalidParams("v".into()), "SANDBOX_VNET_INVALID_PARAMS"),
            (SandboxError::VnetResponseTooLarge("v".into()), "SANDBOX_VNET_RESPONSE_TOO_LARGE"),
            (SandboxError::VnetDnsResolutionFailed("v".into()), "SANDBOX_VNET_DNS_FAILED"),
            (SandboxError::VnetIo("v".into()), "SANDBOX_VNET_IO_ERROR"),
            (SandboxError::VnetJson("v".into()), "SANDBOX_VNET_JSON_ERROR"),
        ];
        for (err, expected) in cases {
            assert_eq!(err.code(), expected, "wrong code for {err}");
        }
    }

    #[test]
    fn temporary_wrapper_delegates_code() {
        let vfs_err = SandboxError::Vfs(VfsError::NotFound("x".into()));
        assert_eq!(vfs_err.code(), "VFS_NOT_FOUND");

        let vsh_err = SandboxError::Vsh(VshError::SessionNotFound("x".into()));
        assert_eq!(vsh_err.code(), "VSH_SESSION_NOT_FOUND");

        let vnet_err = SandboxError::Vnet(VnetError::SandboxViolation("x".into()));
        assert_eq!(vnet_err.code(), "VNET_SANDBOX_VIOLATION");
    }

    #[test]
    fn to_json_rpc_error_format() {
        let err = SandboxError::VfsNotFound("/foo".into());
        let val = err.to_json_rpc_error();
        assert_eq!(val["sandboxCode"], "SANDBOX_VFS_NOT_FOUND");
        assert!(val["message"].as_str().unwrap().contains("/foo"));
    }
}
