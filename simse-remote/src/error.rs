use thiserror::Error;

#[derive(Debug, Error)]
pub enum RemoteError {
    #[error("Not initialized")]
    NotInitialized,
    #[error("Not authenticated")]
    NotAuthenticated,
    #[error("Auth failed: {0}")]
    AuthFailed(String),
    #[error("Token expired")]
    TokenExpired,
    #[error("Tunnel not connected")]
    TunnelNotConnected,
    #[error("Tunnel already connected")]
    TunnelAlreadyConnected,
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Relay error: {0}")]
    RelayError(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("WebSocket error: {0}")]
    WebSocket(String),
}

impl RemoteError {
    pub fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "REMOTE_NOT_INITIALIZED",
            Self::NotAuthenticated => "REMOTE_NOT_AUTHENTICATED",
            Self::AuthFailed(_) => "REMOTE_AUTH_FAILED",
            Self::TokenExpired => "REMOTE_TOKEN_EXPIRED",
            Self::TunnelNotConnected => "REMOTE_TUNNEL_NOT_CONNECTED",
            Self::TunnelAlreadyConnected => "REMOTE_TUNNEL_ALREADY_CONNECTED",
            Self::ConnectionFailed(_) => "REMOTE_CONNECTION_FAILED",
            Self::Timeout(_) => "REMOTE_TIMEOUT",
            Self::RelayError(_) => "REMOTE_RELAY_ERROR",
            Self::InvalidParams(_) => "REMOTE_INVALID_PARAMS",
            Self::Io(_) => "REMOTE_IO_ERROR",
            Self::Json(_) => "REMOTE_JSON_ERROR",
            Self::Http(_) => "REMOTE_HTTP_ERROR",
            Self::WebSocket(_) => "REMOTE_WEBSOCKET_ERROR",
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "remoteCode": self.code(),
            "message": self.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_have_remote_prefix() {
        let cases = vec![
            (RemoteError::NotInitialized, "REMOTE_NOT_INITIALIZED"),
            (RemoteError::NotAuthenticated, "REMOTE_NOT_AUTHENTICATED"),
            (RemoteError::AuthFailed("test".into()), "REMOTE_AUTH_FAILED"),
            (RemoteError::TokenExpired, "REMOTE_TOKEN_EXPIRED"),
            (RemoteError::TunnelNotConnected, "REMOTE_TUNNEL_NOT_CONNECTED"),
            (RemoteError::TunnelAlreadyConnected, "REMOTE_TUNNEL_ALREADY_CONNECTED"),
            (RemoteError::ConnectionFailed("test".into()), "REMOTE_CONNECTION_FAILED"),
            (RemoteError::Timeout("test".into()), "REMOTE_TIMEOUT"),
            (RemoteError::RelayError("test".into()), "REMOTE_RELAY_ERROR"),
            (RemoteError::InvalidParams("test".into()), "REMOTE_INVALID_PARAMS"),
            (RemoteError::WebSocket("test".into()), "REMOTE_WEBSOCKET_ERROR"),
        ];
        for (err, expected_code) in cases {
            assert_eq!(err.code(), expected_code, "wrong code for {err}");
        }
    }

    #[test]
    fn to_json_rpc_error_has_remote_code() {
        let err = RemoteError::AuthFailed("bad credentials".into());
        let json = err.to_json_rpc_error();
        assert_eq!(json["remoteCode"], "REMOTE_AUTH_FAILED");
        assert_eq!(json["message"], "Auth failed: bad credentials");
    }
}
