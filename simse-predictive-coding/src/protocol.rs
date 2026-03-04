use serde::Deserialize;

use crate::config::PcnConfig;
use crate::encoder::LibraryEvent;

pub const INTERNAL_ERROR: i32 = -32603;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const PCN_ERROR: i32 = -32000;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub embedding_dim: usize,
    pub config: PcnConfig,
    pub storage_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedEventParams {
    pub event: LibraryEvent,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_initialize_params() {
        let json = serde_json::json!({
            "embeddingDim": 128,
            "config": {
                "layers": [
                    { "dim": 256, "activation": "relu" },
                    { "dim": 64, "activation": "tanh" }
                ],
                "inferenceSteps": 20,
                "learningRate": 0.005,
                "inferenceRate": 0.1,
                "batchSize": 16,
                "maxBatchDelayMs": 1000,
                "channelCapacity": 1024,
                "autoSaveEpochs": 100,
                "maxTopics": 500,
                "maxTags": 1000,
                "temporalAmortization": true,
                "storagePath": null
            },
            "storagePath": "/tmp/pcn"
        });

        let params: InitializeParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.embedding_dim, 128);
        assert_eq!(params.config.layers.len(), 2);
        assert_eq!(params.config.inference_steps, 20);
        assert_eq!(params.storage_path, Some("/tmp/pcn".to_string()));
    }

    #[test]
    fn parse_predict_confidence_params() {
        let json = serde_json::json!({
            "input": [0.1, 0.2, 0.3, 0.4, 0.5]
        });

        let params: PredictConfidenceParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.input.len(), 5);
        assert!((params.input[0] - 0.1).abs() < 1e-10);
        assert!((params.input[4] - 0.5).abs() < 1e-10);
    }

    #[test]
    fn parse_feed_event_params() {
        let json = serde_json::json!({
            "event": {
                "embedding": [0.1, 0.2, 0.3],
                "topic": "rust",
                "tags": ["important", "core"],
                "entryType": "fact",
                "timestamp": 1000.0,
                "timeSinceLast": 60.0,
                "sessionOrdinal": 5.0,
                "action": "extraction"
            }
        });

        let params: FeedEventParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.event.topic, "rust");
        assert_eq!(params.event.tags.len(), 2);
        assert_eq!(params.event.embedding.len(), 3);
        assert!((params.event.timestamp - 1000.0).abs() < 1e-10);
        assert_eq!(params.event.action, "extraction");
    }

    #[test]
    fn parse_json_rpc_request() {
        let json = serde_json::json!({
            "id": 42,
            "method": "pcn/initialize",
            "params": { "embeddingDim": 64 }
        });

        let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.id, 42);
        assert_eq!(req.method, "pcn/initialize");
        assert!(req.params.is_object());
    }

    #[test]
    fn parse_json_rpc_request_without_params() {
        let json = serde_json::json!({
            "id": 1,
            "method": "pcn/health"
        });

        let req: JsonRpcRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "pcn/health");
        assert!(req.params.is_null());
    }

    #[test]
    fn parse_predict_anomalies_params() {
        let json = serde_json::json!({
            "inputs": [[0.1, 0.2], [0.3, 0.4], [0.5, 0.6]],
            "topK": 2
        });

        let params: PredictAnomaliesParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.inputs.len(), 3);
        assert_eq!(params.top_k, 2);
        assert_eq!(params.inputs[0].len(), 2);
    }

    #[test]
    fn parse_model_snapshot_params() {
        let json = serde_json::json!({
            "path": "/tmp/snapshot.bin",
            "compress": true
        });

        let params: ModelSnapshotParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.path, "/tmp/snapshot.bin");
        assert!(params.compress);
    }

    #[test]
    fn parse_model_snapshot_params_default_compress() {
        let json = serde_json::json!({
            "path": "/tmp/snapshot.bin"
        });

        let params: ModelSnapshotParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.path, "/tmp/snapshot.bin");
        assert!(!params.compress);
    }

    #[test]
    fn parse_model_restore_params() {
        let json = serde_json::json!({
            "path": "/tmp/snapshot.bin",
            "compressed": true
        });

        let params: ModelRestoreParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.path, "/tmp/snapshot.bin");
        assert!(params.compressed);
    }

    #[test]
    fn parse_model_configure_params() {
        let json = serde_json::json!({
            "config": {
                "layers": [{ "dim": 128, "activation": "sigmoid" }],
                "inferenceSteps": 10,
                "learningRate": 0.01,
                "inferenceRate": 0.05,
                "batchSize": 8,
                "maxBatchDelayMs": 500,
                "channelCapacity": 512,
                "autoSaveEpochs": 50,
                "maxTopics": 100,
                "maxTags": 200,
                "temporalAmortization": false,
                "storagePath": "/data/pcn"
            }
        });

        let params: ModelConfigureParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.config.layers.len(), 1);
        assert_eq!(params.config.inference_steps, 10);
        assert!(!params.config.temporal_amortization);
        assert_eq!(
            params.config.storage_path,
            Some("/data/pcn".to_string())
        );
    }

    #[test]
    fn error_code_constants() {
        assert_eq!(INTERNAL_ERROR, -32603);
        assert_eq!(METHOD_NOT_FOUND, -32601);
        assert_eq!(INVALID_PARAMS, -32602);
        assert_eq!(PCN_ERROR, -32000);
    }
}
