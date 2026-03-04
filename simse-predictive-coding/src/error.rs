use thiserror::Error;

#[derive(Debug, Error)]
pub enum PcnError {
    #[error("Not initialized")]
    NotInitialized,
    #[error("Invalid config: {0}")]
    InvalidConfig(String),
    #[error("Training failed: {0}")]
    TrainingFailed(String),
    #[error("Inference timeout")]
    InferenceTimeout,
    #[error("Model corrupt: {0}")]
    ModelCorrupt(String),
    #[error("Vocabulary overflow: {0}")]
    VocabularyOverflow(String),
    #[error("Invalid params: {0}")]
    InvalidParams(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl PcnError {
    pub fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "PCN_NOT_INITIALIZED",
            Self::InvalidConfig(_) => "PCN_INVALID_CONFIG",
            Self::TrainingFailed(_) => "PCN_TRAINING_FAILED",
            Self::InferenceTimeout => "PCN_INFERENCE_TIMEOUT",
            Self::ModelCorrupt(_) => "PCN_MODEL_CORRUPT",
            Self::VocabularyOverflow(_) => "PCN_VOCABULARY_OVERFLOW",
            Self::InvalidParams(_) => "PCN_INVALID_PARAMS",
            Self::Io(_) => "PCN_IO_ERROR",
            Self::Json(_) => "PCN_JSON_ERROR",
        }
    }

    pub fn to_json_rpc_error(&self) -> serde_json::Value {
        serde_json::json!({
            "pcnCode": self.code(),
            "message": self.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_have_pcn_prefix() {
        let err = PcnError::NotInitialized;
        assert!(err.code().starts_with("PCN_"));
    }

    #[test]
    fn to_json_rpc_error_has_pcn_code() {
        let err = PcnError::InvalidConfig("bad".into());
        let json = err.to_json_rpc_error();
        assert_eq!(json["pcnCode"], "PCN_INVALID_CONFIG");
    }

    #[test]
    fn all_variants_have_unique_codes() {
        let variants: Vec<PcnError> = vec![
            PcnError::NotInitialized,
            PcnError::InvalidConfig("x".into()),
            PcnError::TrainingFailed("x".into()),
            PcnError::InferenceTimeout,
            PcnError::ModelCorrupt("x".into()),
            PcnError::VocabularyOverflow("x".into()),
            PcnError::InvalidParams("x".into()),
        ];
        let codes: Vec<&str> = variants.iter().map(|v| v.code()).collect();
        let unique: std::collections::HashSet<&str> = codes.iter().copied().collect();
        assert_eq!(codes.len(), unique.len(), "All error codes should be unique");
    }

    #[test]
    fn error_display_messages() {
        assert_eq!(PcnError::NotInitialized.to_string(), "Not initialized");
        assert_eq!(
            PcnError::InvalidConfig("bad dims".into()).to_string(),
            "Invalid config: bad dims"
        );
    }
}
