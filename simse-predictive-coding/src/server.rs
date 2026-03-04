use std::io::{self, BufRead};
use std::sync::{Arc, RwLock};

use tokio::sync::mpsc;
use tracing::info;

use crate::config::PcnConfig;
use crate::encoder::LibraryEvent;
use crate::error::PcnError;
use crate::persistence::{load_snapshot, save_snapshot};
use crate::predictor::Predictor;
use crate::protocol::*;
use crate::snapshot::ModelSnapshot;
use crate::trainer::TrainingWorker;
use crate::transport::NdjsonTransport;

/// PCN JSON-RPC server -- dispatches incoming requests to predictive coding operations.
pub struct PcnServer {
	transport: NdjsonTransport,
	snapshot: Arc<RwLock<ModelSnapshot>>,
	predictor: Option<Predictor>,
	event_tx: Option<mpsc::Sender<LibraryEvent>>,
	initialized: bool,
	config: Option<PcnConfig>,
	embedding_dim: usize,
}

impl PcnServer {
	pub fn new(transport: NdjsonTransport) -> Self {
		Self {
			transport,
			snapshot: Arc::new(RwLock::new(ModelSnapshot::empty())),
			predictor: None,
			event_tx: None,
			initialized: false,
			config: None,
			embedding_dim: 0,
		}
	}

	/// Main loop: read JSON-RPC messages from stdin, dispatch to handlers.
	pub async fn run(&mut self) -> Result<(), PcnError> {
		let stdin = io::stdin();
		let reader = stdin.lock();

		for line_result in reader.lines() {
			let line = line_result?;
			if line.trim().is_empty() {
				continue;
			}

			let request: JsonRpcRequest = match serde_json::from_str(&line) {
				Ok(r) => r,
				Err(e) => {
					tracing::error!("Failed to parse request: {}", e);
					continue;
				}
			};

			self.dispatch(request).await;
		}

		Ok(())
	}

	// -- Dispatch -------------------------------------------------------------

	async fn dispatch(&mut self, req: JsonRpcRequest) {
		let result = match req.method.as_str() {
			"pcn/initialize" => self.handle_initialize(req.params),
			"pcn/dispose" => self.handle_dispose(),
			"pcn/health" => self.handle_health(),
			"feed/event" => self.handle_feed_event(req.params),
			"predict/confidence" => self.handle_predict_confidence(req.params),
			"predict/anomalies" => self.handle_predict_anomalies(req.params),
			"model/stats" => self.handle_model_stats(),
			"model/snapshot" => self.handle_model_snapshot(req.params),
			"model/restore" => self.handle_model_restore(req.params),
			"model/reset" => self.handle_model_reset(),
			_ => {
				self.transport.write_error(
					req.id,
					METHOD_NOT_FOUND,
					format!("Unknown method: {}", req.method),
					None,
				);
				return;
			}
		};

		match result {
			Ok(value) => self.transport.write_response(req.id, value),
			Err(e) => self.transport.write_error(
				req.id,
				PCN_ERROR,
				e.to_string(),
				Some(e.to_json_rpc_error()),
			),
		}
	}

	// -- Initialize -----------------------------------------------------------

	fn handle_initialize(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, PcnError> {
		let p: InitializeParams = parse_params(params)?;

		let config = p.config;
		let embedding_dim = p.embedding_dim;

		// Create the shared snapshot.
		let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));

		// Create the mpsc channel for feeding events to the training worker.
		let (tx, rx) = mpsc::channel::<LibraryEvent>(config.channel_capacity);

		// Spawn the background training worker.
		let worker_snapshot = snapshot.clone();
		let worker_config = config.clone();
		tokio::spawn(async move {
			let stats =
				TrainingWorker::run_batch(rx, worker_snapshot, worker_config, embedding_dim).await;
			info!(
				epochs = stats.epochs,
				total_samples = stats.total_samples,
				"Training worker exited"
			);
		});

		// Create the predictor.
		let predictor = Predictor::new(snapshot.clone(), config.inference_steps);

		self.snapshot = snapshot;
		self.predictor = Some(predictor);
		self.event_tx = Some(tx);
		self.config = Some(config);
		self.embedding_dim = embedding_dim;
		self.initialized = true;

		Ok(serde_json::json!({ "ok": true }))
	}

	// -- Dispose --------------------------------------------------------------

	fn handle_dispose(&mut self) -> Result<serde_json::Value, PcnError> {
		// Drop the sender to close the channel, which will cause the training
		// worker to finish processing remaining events and exit.
		self.event_tx = None;
		self.predictor = None;
		self.initialized = false;
		self.config = None;

		Ok(serde_json::json!({ "ok": true }))
	}

	// -- Health ---------------------------------------------------------------

	fn handle_health(&self) -> Result<serde_json::Value, PcnError> {
		Ok(serde_json::json!({
			"initialized": self.initialized,
			"embeddingDim": self.embedding_dim,
			"hasPredictor": self.predictor.is_some(),
			"hasEventChannel": self.event_tx.is_some(),
		}))
	}

	// -- Feed event -----------------------------------------------------------

	fn handle_feed_event(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, PcnError> {
		self.require_initialized()?;

		let p: FeedEventParams = parse_params(params)?;
		let tx = self.event_tx.as_ref().unwrap();

		match tx.try_send(p.event) {
			Ok(()) => Ok(serde_json::json!({ "queued": true })),
			Err(mpsc::error::TrySendError::Full(_)) => {
				Ok(serde_json::json!({ "queued": false, "reason": "channel_full" }))
			}
			Err(mpsc::error::TrySendError::Closed(_)) => {
				Ok(serde_json::json!({ "queued": false, "reason": "channel_closed" }))
			}
		}
	}

	// -- Predict confidence ---------------------------------------------------

	fn handle_predict_confidence(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, PcnError> {
		self.require_initialized()?;

		let p: PredictConfidenceParams = parse_params(params)?;
		let predictor = self.predictor.as_ref().unwrap();

		match predictor.confidence(&p.input) {
			Some(result) => Ok(serde_json::json!({
				"energy": result.energy,
				"topLatent": result.top_latent,
				"energyBreakdown": result.energy_breakdown,
				"reconstruction": result.reconstruction,
			})),
			None => Ok(serde_json::json!({
				"energy": null,
				"reason": "no_trained_model",
			})),
		}
	}

	// -- Predict anomalies ----------------------------------------------------

	fn handle_predict_anomalies(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, PcnError> {
		self.require_initialized()?;

		let p: PredictAnomaliesParams = parse_params(params)?;
		let predictor = self.predictor.as_ref().unwrap();

		let anomalies = predictor.anomalies(&p.inputs, p.top_k);
		let results: Vec<serde_json::Value> = anomalies
			.into_iter()
			.map(|(index, energy)| {
				serde_json::json!({
					"index": index,
					"energy": energy,
				})
			})
			.collect();

		Ok(serde_json::json!({ "anomalies": results }))
	}

	// -- Model stats ----------------------------------------------------------

	fn handle_model_stats(&self) -> Result<serde_json::Value, PcnError> {
		self.require_initialized()?;

		let predictor = self.predictor.as_ref().unwrap();
		let stats = predictor.model_stats();

		Ok(serde_json::json!({
			"epoch": stats.epoch,
			"totalSamples": stats.total_samples,
			"numLayers": stats.num_layers,
			"inputDim": stats.input_dim,
			"layerDims": stats.layer_dims,
			"parameterCount": stats.parameter_count,
		}))
	}

	// -- Model snapshot (save) ------------------------------------------------

	fn handle_model_snapshot(
		&self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, PcnError> {
		self.require_initialized()?;

		let p: ModelSnapshotParams = parse_params(params)?;
		let snap = self.snapshot.read().unwrap();

		save_snapshot(&snap, &p.path, p.compress)?;

		Ok(serde_json::json!({
			"ok": true,
			"path": p.path,
			"compressed": p.compress,
		}))
	}

	// -- Model restore (load) -------------------------------------------------

	fn handle_model_restore(
		&mut self,
		params: serde_json::Value,
	) -> Result<serde_json::Value, PcnError> {
		self.require_initialized()?;

		let p: ModelRestoreParams = parse_params(params)?;
		let loaded = load_snapshot(&p.path, p.compressed)?;

		let mut snap = self.snapshot.write().unwrap();
		*snap = loaded;

		Ok(serde_json::json!({
			"ok": true,
			"path": p.path,
			"epoch": snap.epoch,
			"totalSamples": snap.total_samples,
		}))
	}

	// -- Model reset ----------------------------------------------------------

	fn handle_model_reset(&self) -> Result<serde_json::Value, PcnError> {
		self.require_initialized()?;

		let mut snap = self.snapshot.write().unwrap();
		*snap = ModelSnapshot::empty();

		Ok(serde_json::json!({ "ok": true }))
	}

	// -- Helpers --------------------------------------------------------------

	fn require_initialized(&self) -> Result<(), PcnError> {
		if !self.initialized {
			return Err(PcnError::NotInitialized);
		}
		Ok(())
	}
}

fn parse_params<T: serde::de::DeserializeOwned>(
	params: serde_json::Value,
) -> Result<T, PcnError> {
	serde_json::from_value(params).map_err(|e| PcnError::InvalidParams(e.to_string()))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn server_starts_uninitialized() {
		let transport = NdjsonTransport::new();
		let server = PcnServer::new(transport);

		assert!(!server.initialized);
		assert!(server.predictor.is_none());
		assert!(server.event_tx.is_none());
		assert!(server.config.is_none());
		assert_eq!(server.embedding_dim, 0);
	}

	#[test]
	fn require_initialized_returns_error_when_not_initialized() {
		let transport = NdjsonTransport::new();
		let server = PcnServer::new(transport);

		let result = server.require_initialized();
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert_eq!(err.code(), "PCN_NOT_INITIALIZED");
	}

	#[test]
	fn health_when_not_initialized() {
		let transport = NdjsonTransport::new();
		let server = PcnServer::new(transport);

		let result = server.handle_health().unwrap();
		assert_eq!(result["initialized"], false);
		assert_eq!(result["embeddingDim"], 0);
		assert_eq!(result["hasPredictor"], false);
		assert_eq!(result["hasEventChannel"], false);
	}

	#[tokio::test]
	async fn initialize_and_dispose() {
		let transport = NdjsonTransport::new();
		let mut server = PcnServer::new(transport);

		let params = serde_json::json!({
			"embeddingDim": 4,
			"config": {
				"layers": [
					{ "dim": 8, "activation": "relu" },
					{ "dim": 4, "activation": "tanh" }
				],
				"inferenceSteps": 5,
				"learningRate": 0.01,
				"inferenceRate": 0.1,
				"batchSize": 2,
				"maxBatchDelayMs": 100,
				"channelCapacity": 16,
				"autoSaveEpochs": 10,
				"maxTopics": 10,
				"maxTags": 20,
				"temporalAmortization": false,
				"storagePath": null
			}
		});

		let result = server.handle_initialize(params).unwrap();
		assert_eq!(result["ok"], true);
		assert!(server.initialized);
		assert!(server.predictor.is_some());
		assert!(server.event_tx.is_some());
		assert_eq!(server.embedding_dim, 4);

		// Health should reflect initialized state.
		let health = server.handle_health().unwrap();
		assert_eq!(health["initialized"], true);
		assert_eq!(health["embeddingDim"], 4);
		assert_eq!(health["hasPredictor"], true);
		assert_eq!(health["hasEventChannel"], true);

		// Dispose.
		let result = server.handle_dispose().unwrap();
		assert_eq!(result["ok"], true);
		assert!(!server.initialized);
		assert!(server.predictor.is_none());
		assert!(server.event_tx.is_none());
	}

	#[tokio::test]
	async fn feed_event_requires_initialization() {
		let transport = NdjsonTransport::new();
		let server = PcnServer::new(transport);

		let params = serde_json::json!({
			"event": {
				"embedding": [0.1, 0.2, 0.3],
				"topic": "rust",
				"tags": ["test"],
				"entryType": "fact",
				"timestamp": 1.0,
				"timeSinceLast": 0.5,
				"sessionOrdinal": 1.0,
				"action": "extraction"
			}
		});

		let result = server.handle_feed_event(params);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().code(), "PCN_NOT_INITIALIZED");
	}

	#[tokio::test]
	async fn feed_event_queues_successfully() {
		let transport = NdjsonTransport::new();
		let mut server = PcnServer::new(transport);

		let init_params = serde_json::json!({
			"embeddingDim": 4,
			"config": {
				"layers": [{ "dim": 8, "activation": "relu" }],
				"inferenceSteps": 5,
				"learningRate": 0.01,
				"inferenceRate": 0.1,
				"batchSize": 2,
				"maxBatchDelayMs": 100,
				"channelCapacity": 2,
				"autoSaveEpochs": 10,
				"maxTopics": 10,
				"maxTags": 20,
				"temporalAmortization": false,
				"storagePath": null
			}
		});
		server.handle_initialize(init_params).unwrap();

		let event_params = serde_json::json!({
			"event": {
				"embedding": [0.1, 0.2, 0.3, 0.4],
				"topic": "rust",
				"tags": ["test"],
				"entryType": "fact",
				"timestamp": 1.0,
				"timeSinceLast": 0.5,
				"sessionOrdinal": 1.0,
				"action": "extraction"
			}
		});

		let result = server.handle_feed_event(event_params).unwrap();
		assert_eq!(result["queued"], true);

		// Clean up.
		server.handle_dispose().unwrap();
	}

	#[tokio::test]
	async fn predict_confidence_requires_initialization() {
		let transport = NdjsonTransport::new();
		let server = PcnServer::new(transport);

		let params = serde_json::json!({ "input": [0.1, 0.2] });
		let result = server.handle_predict_confidence(params);
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().code(), "PCN_NOT_INITIALIZED");
	}

	#[tokio::test]
	async fn model_stats_requires_initialization() {
		let transport = NdjsonTransport::new();
		let server = PcnServer::new(transport);

		let result = server.handle_model_stats();
		assert!(result.is_err());
		assert_eq!(result.unwrap_err().code(), "PCN_NOT_INITIALIZED");
	}

	#[tokio::test]
	async fn model_reset_clears_snapshot() {
		let transport = NdjsonTransport::new();
		let mut server = PcnServer::new(transport);

		let init_params = serde_json::json!({
			"embeddingDim": 4,
			"config": {
				"layers": [{ "dim": 8, "activation": "relu" }],
				"inferenceSteps": 5,
				"learningRate": 0.01,
				"inferenceRate": 0.1,
				"batchSize": 2,
				"maxBatchDelayMs": 100,
				"channelCapacity": 16,
				"autoSaveEpochs": 10,
				"maxTopics": 10,
				"maxTags": 20,
				"temporalAmortization": false,
				"storagePath": null
			}
		});
		server.handle_initialize(init_params).unwrap();

		let result = server.handle_model_reset().unwrap();
		assert_eq!(result["ok"], true);

		// Snapshot should be empty now.
		let snap = server.snapshot.read().unwrap();
		assert_eq!(snap.input_dim, 0);
		assert_eq!(snap.epoch, 0);

		// Clean up.
		drop(snap);
		server.handle_dispose().unwrap();
	}
}
