use thiserror::Error;

#[derive(Debug, Error)]
pub enum AdaptiveError {
	// -- Vector store variants -----------------------------------------------
	#[error("Store not initialized: call store/initialize first")]
	NotInitialized,
	#[error("Empty text: cannot add empty text")]
	EmptyText,
	#[error("Empty embedding: cannot add volume with empty embedding")]
	EmptyEmbedding,
	#[error("Entry not found: {0}")]
	NotFound(String),
	#[error("Duplicate detected: similarity {0:.4}")]
	Duplicate(f64),
	#[error("Invalid regex pattern: {0}")]
	InvalidRegex(String),
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Serialization error: {0}")]
	Serialization(String),
	#[error("Storage corruption: {0}")]
	Corruption(String),

	// -- PCN variants --------------------------------------------------------
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
	#[error("JSON error: {0}")]
	Json(#[from] serde_json::Error),
}

impl AdaptiveError {
	pub fn code(&self) -> &str {
		match self {
			Self::NotInitialized => "STACKS_NOT_LOADED",
			Self::EmptyText => "STACKS_EMPTY_TEXT",
			Self::EmptyEmbedding => "STACKS_EMPTY_EMBEDDING",
			Self::NotFound(_) => "MEMORY_ENTRY_NOT_FOUND",
			Self::Duplicate(_) => "STACKS_DUPLICATE",
			Self::InvalidRegex(_) => "STACKS_INVALID_REGEX",
			Self::Io(_) => "STACKS_IO",
			Self::Serialization(_) => "STACKS_SERIALIZATION",
			Self::Corruption(_) => "STACKS_CORRUPT",
			Self::InvalidConfig(_) => "PCN_INVALID_CONFIG",
			Self::TrainingFailed(_) => "PCN_TRAINING_FAILED",
			Self::InferenceTimeout => "PCN_INFERENCE_TIMEOUT",
			Self::ModelCorrupt(_) => "PCN_MODEL_CORRUPT",
			Self::VocabularyOverflow(_) => "PCN_VOCABULARY_OVERFLOW",
			Self::InvalidParams(_) => "PCN_INVALID_PARAMS",
			Self::Json(_) => "PCN_JSON_ERROR",
		}
	}

	pub fn to_json_rpc_error(&self) -> serde_json::Value {
		serde_json::json!({
			"adaptiveCode": self.code(),
			"message": self.to_string(),
		})
	}
}
