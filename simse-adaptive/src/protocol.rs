use serde::Deserialize;

use crate::pcn::encoder::InputEvent;
use crate::pcn::config::PcnConfig;

// JSON-RPC 2.0 error codes
pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const ADAPTIVE_ERROR: i32 = -32000;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
	pub id: u64,
	pub method: String,
	#[serde(default)]
	pub params: serde_json::Value,
}

// -- PCN protocol types -------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcnInitializeParams {
	pub embedding_dim: usize,
	pub config: PcnConfig,
	pub storage_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedEventParams {
	pub event: InputEvent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictConfidenceParams {
	pub input: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictAnomaliesParams {
	pub inputs: Vec<Vec<f64>>,
	pub top_k: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSnapshotParams {
	pub path: String,
	#[serde(default)]
	pub compress: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRestoreParams {
	pub path: String,
	#[serde(default)]
	pub compressed: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfigureParams {
	pub config: PcnConfig,
}
