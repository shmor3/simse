use thiserror::Error;

/// Typed error variants for the simse-engine inference server.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("Transport error: {0}")]
    Transport(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Model error: {0}")]
    Model(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Candle error: {0}")]
    Candle(#[from] candle_core::Error),

    #[error("Tokenizer error: {0}")]
    Tokenizer(String),

    #[error("Hub error: {0}")]
    Hub(String),
}

impl EngineError {
    /// Return a machine-readable error code string for this error variant.
    pub fn code(&self) -> &str {
        match self {
            Self::Transport(_) => "TRANSPORT_ERROR",
            Self::Protocol(_) => "PROTOCOL_ERROR",
            Self::Model(_) => "MODEL_ERROR",
            Self::Inference(_) => "INFERENCE_ERROR",
            Self::SessionNotFound(_) => "SESSION_NOT_FOUND",
            Self::Io(_) => "IO_ERROR",
            Self::Json(_) => "JSON_ERROR",
            Self::Candle(_) => "CANDLE_ERROR",
            Self::Tokenizer(_) => "TOKENIZER_ERROR",
            Self::Hub(_) => "HUB_ERROR",
        }
    }
}
