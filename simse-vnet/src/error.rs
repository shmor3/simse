use thiserror::Error;

#[derive(Debug, Error)]
pub enum VnetError {
    #[error("Not initialized")]
    NotInitialized,
    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("Mock not found: {0}")]
    MockNotFound(String),
    #[error("No mock match: {0}")]
    NoMockMatch(String),
    #[error("Limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("Response too large: {0}")]
    ResponseTooLarge(String),
    #[error("DNS resolution failed: {0}")]
    DnsResolutionFailed(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl VnetError {
    pub fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "VNET_NOT_INITIALIZED",
            Self::SandboxViolation(_) => "VNET_SANDBOX_VIOLATION",
            Self::ConnectionFailed(_) => "VNET_CONNECTION_FAILED",
            Self::Timeout(_) => "VNET_TIMEOUT",
            Self::SessionNotFound(_) => "VNET_SESSION_NOT_FOUND",
            Self::MockNotFound(_) => "VNET_MOCK_NOT_FOUND",
            Self::NoMockMatch(_) => "VNET_NO_MOCK_MATCH",
            Self::LimitExceeded(_) => "VNET_LIMIT_EXCEEDED",
            Self::InvalidParams(_) => "VNET_INVALID_PARAMS",
            Self::ResponseTooLarge(_) => "VNET_RESPONSE_TOO_LARGE",
            Self::DnsResolutionFailed(_) => "VNET_DNS_FAILED",
            Self::Io(_) => "VNET_IO_ERROR",
            Self::Json(_) => "VNET_JSON_ERROR",
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "vnetCode": self.code(),
            "message": self.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_have_vnet_prefix() {
        let cases = vec![
            (VnetError::NotInitialized, "VNET_NOT_INITIALIZED"),
            (VnetError::SandboxViolation("test".into()), "VNET_SANDBOX_VIOLATION"),
            (VnetError::ConnectionFailed("test".into()), "VNET_CONNECTION_FAILED"),
            (VnetError::Timeout("test".into()), "VNET_TIMEOUT"),
            (VnetError::SessionNotFound("test".into()), "VNET_SESSION_NOT_FOUND"),
            (VnetError::MockNotFound("test".into()), "VNET_MOCK_NOT_FOUND"),
            (VnetError::NoMockMatch("test".into()), "VNET_NO_MOCK_MATCH"),
            (VnetError::LimitExceeded("test".into()), "VNET_LIMIT_EXCEEDED"),
            (VnetError::InvalidParams("test".into()), "VNET_INVALID_PARAMS"),
            (VnetError::ResponseTooLarge("test".into()), "VNET_RESPONSE_TOO_LARGE"),
            (VnetError::DnsResolutionFailed("test".into()), "VNET_DNS_FAILED"),
        ];
        for (err, expected_code) in cases {
            assert_eq!(err.code(), expected_code, "wrong code for {err}");
        }
    }

    #[test]
    fn to_json_rpc_error_includes_vnet_code() {
        let err = VnetError::SandboxViolation("blocked".into());
        let val = err.to_json_rpc_error();
        assert_eq!(val["vnetCode"], "VNET_SANDBOX_VIOLATION");
        assert!(val["message"].as_str().unwrap().contains("blocked"));
    }
}
