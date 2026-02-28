use thiserror::Error;

#[derive(Debug, Error)]
pub enum VectorError {
	#[error("Store not initialized: call store/initialize first")]
	NotInitialized,
	#[error("Empty text: cannot add empty text")]
	EmptyText,
	#[error("Empty embedding: cannot add volume with empty embedding")]
	EmptyEmbedding,
	#[error("Volume not found: {0}")]
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
}

impl VectorError {
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
		}
	}

	pub fn to_json_rpc_error(&self) -> serde_json::Value {
		serde_json::json!({
			"vectorCode": self.code(),
			"message": self.to_string(),
		})
	}
}
