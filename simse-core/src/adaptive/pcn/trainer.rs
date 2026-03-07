use std::sync::{Arc, RwLock};

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::adaptive::pcn::config::PcnConfig;
use crate::adaptive::pcn::encoder::{InputEncoder, InputEvent};
use crate::adaptive::pcn::network::PredictiveCodingNetwork;
use crate::adaptive::persistence::save_snapshot;
use crate::adaptive::pcn::snapshot::ModelSnapshot;

/// Aggregate statistics from the background training loop.
#[derive(Debug, Clone, Default)]
pub struct TrainingStats {
    /// Number of completed training epochs (batches processed).
    pub epochs: usize,
    /// Total number of individual samples trained on.
    pub total_samples: usize,
    /// Number of events that were dropped (e.g. due to encoding errors).
    pub dropped_events: usize,
    /// Energy from the most recently trained sample.
    pub last_energy: f64,
    /// History of average energy per batch (one entry per epoch).
    pub energy_history: Vec<f64>,
}

/// Background training worker that processes library events from an mpsc channel,
/// trains a predictive coding network in batches, and atomically swaps the shared
/// [`ModelSnapshot`] after each batch completes.
pub struct TrainingWorker;

impl TrainingWorker {
    /// Run the batch training loop.
    ///
    /// This is the main entry point for the background training worker. It:
    /// 1. Creates an [`InputEncoder`] and [`PredictiveCodingNetwork`]
    /// 2. Loops: receives events from the mpsc channel
    /// 3. Buffers events into batches of `config.batch_size`
    /// 4. On timeout (`config.max_batch_delay_ms`) or full batch: trains
    /// 5. After training each batch: swaps the [`ModelSnapshot`] via `Arc<RwLock>`
    /// 6. On channel close: trains any remaining buffered events and exits
    ///
    /// Returns the final [`TrainingStats`] when the channel is closed and all
    /// remaining events have been processed.
    pub async fn run_batch(
        mut rx: mpsc::Receiver<InputEvent>,
        snapshot: Arc<RwLock<ModelSnapshot>>,
        config: PcnConfig,
        embedding_dim: usize,
    ) -> TrainingStats {
        let mut encoder = InputEncoder::new(embedding_dim, config.max_topics, config.max_tags);
        let input_dim = encoder.current_input_dim();
        let mut network = PredictiveCodingNetwork::new(input_dim, &config);
        let mut stats = TrainingStats::default();
        let mut batch: Vec<InputEvent> = Vec::with_capacity(config.batch_size);

        let batch_timeout = tokio::time::Duration::from_millis(config.max_batch_delay_ms);

        loop {
            // Try to fill a batch, with a timeout so we don't wait forever.
            let event = if batch.is_empty() {
                // No events buffered yet — wait indefinitely for the first event.
                match rx.recv().await {
                    Some(ev) => Some(ev),
                    None => {
                        // Channel closed with no buffered events.
                        break;
                    }
                }
            } else {
                // We have some buffered events — wait with a timeout.
                match tokio::time::timeout(batch_timeout, rx.recv()).await {
                    Ok(Some(ev)) => Some(ev),
                    Ok(None) => {
                        // Channel closed — train remaining batch.
                        Self::train_batch(
                            &mut batch,
                            &mut encoder,
                            &mut network,
                            &config,
                            &snapshot,
                            &mut stats,
                        );
                        break;
                    }
                    Err(_) => {
                        // Timeout — train what we have.
                        Self::train_batch(
                            &mut batch,
                            &mut encoder,
                            &mut network,
                            &config,
                            &snapshot,
                            &mut stats,
                        );
                        continue;
                    }
                }
            };

            if let Some(ev) = event {
                batch.push(ev);
            }

            // If the batch is full, train immediately.
            if batch.len() >= config.batch_size {
                Self::train_batch(
                    &mut batch,
                    &mut encoder,
                    &mut network,
                    &config,
                    &snapshot,
                    &mut stats,
                );
            }
        }

        info!(
            epochs = stats.epochs,
            total_samples = stats.total_samples,
            dropped_events = stats.dropped_events,
            last_energy = stats.last_energy,
            "Training worker finished"
        );

        stats
    }

    /// Train a single batch of events, update stats, and swap the snapshot.
    fn train_batch(
        batch: &mut Vec<InputEvent>,
        encoder: &mut InputEncoder,
        network: &mut PredictiveCodingNetwork,
        config: &PcnConfig,
        snapshot: &Arc<RwLock<ModelSnapshot>>,
        stats: &mut TrainingStats,
    ) {
        if batch.is_empty() {
            return;
        }

        let mut batch_energy_sum = 0.0;
        let mut batch_trained = 0usize;

        for event in batch.drain(..) {
            // Encode the event. If vocabulary grew, resize the network.
            let encoded = match encoder.encode(&event) {
                Ok((vec, grew)) => {
                    if grew {
                        let new_dim = encoder.current_input_dim();
                        if new_dim != network.input_dim() {
                            debug!(
                                old_dim = network.input_dim(),
                                new_dim, "Vocabulary grew, resizing network"
                            );
                            network.resize_input(new_dim);
                        }
                    }
                    vec
                }
                Err(e) => {
                    warn!(error = %e, "Failed to encode event, dropping");
                    stats.dropped_events += 1;
                    continue;
                }
            };

            // Train on this sample.
            let energy = network.train_single_with_steps(
                &encoded,
                config.inference_steps,
                config.temporal_amortization,
            );

            batch_energy_sum += energy;
            batch_trained += 1;
            stats.last_energy = energy;
        }

        if batch_trained > 0 {
            stats.epochs += 1;
            stats.total_samples += batch_trained;

            let avg_energy = batch_energy_sum / batch_trained as f64;
            stats.energy_history.push(avg_energy);

            debug!(
                epoch = stats.epochs,
                batch_size = batch_trained,
                avg_energy,
                "Batch training complete"
            );

            // Build new snapshot from current network state.
            let new_snapshot = ModelSnapshot::from_network(
                network,
                encoder.vocab(),
                stats.epochs,
                stats.total_samples,
            );

            // Auto-save before the RwLock swap to avoid re-locking.
            if config.auto_save_epochs > 0
                && stats.epochs.is_multiple_of(config.auto_save_epochs)
            {
                if let Some(ref storage_path) = config.storage_path {
                    let path = format!("{}/pcn-auto-{}.json.gz", storage_path, stats.epochs);
                    if let Err(e) = save_snapshot(&new_snapshot, &path, true) {
                        warn!(error = %e, path, "Auto-save failed");
                    } else {
                        debug!(epoch = stats.epochs, path, "Auto-saved snapshot");
                    }
                }
            }

            // Swap the snapshot into the shared RwLock.
            match snapshot.write() {
                Ok(mut guard) => {
                    *guard = new_snapshot;
                }
                Err(poisoned) => {
                    warn!("Snapshot RwLock was poisoned, recovering");
                    let mut guard = poisoned.into_inner();
                    *guard = new_snapshot;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adaptive::pcn::config::{Activation, LayerConfig, PcnConfig};

    fn test_config() -> PcnConfig {
        PcnConfig {
            layers: vec![
                LayerConfig {
                    dim: 32,
                    activation: Activation::Tanh,
                },
                LayerConfig {
                    dim: 16,
                    activation: Activation::Tanh,
                },
            ],
            inference_steps: 10,
            learning_rate: 0.001,
            inference_rate: 0.05,
            batch_size: 2,
            max_batch_delay_ms: 100,
            channel_capacity: 16,
            max_topics: 10,
            max_tags: 20,
            temporal_amortization: false,
            ..Default::default()
        }
    }

    fn make_event(embedding: Vec<f32>, topic: &str) -> InputEvent {
        InputEvent {
            embedding,
            topic: topic.to_string(),
            tags: vec!["test".to_string()],
            entry_type: "fact".to_string(),
            timestamp: 0.1,
            time_since_last: 0.05,
            session_ordinal: 0.01,
            action: "extraction".to_string(),
        }
    }

    #[tokio::test]
    async fn trainer_processes_batch_and_updates_snapshot() {
        let config = test_config();
        let embedding_dim = 4;
        let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
        let (tx, rx) = mpsc::channel::<InputEvent>(config.channel_capacity);

        // Send 3 events (batch_size=2, so we get 1 full batch + 1 remainder).
        tx.send(make_event(vec![0.1, 0.2, 0.3, 0.4], "rust"))
            .await
            .unwrap();
        tx.send(make_event(vec![0.5, 0.6, 0.7, 0.8], "python"))
            .await
            .unwrap();
        tx.send(make_event(vec![0.9, 1.0, 1.1, 1.2], "rust"))
            .await
            .unwrap();

        // Drop the sender to close the channel so the worker exits.
        drop(tx);

        let stats = TrainingWorker::run_batch(rx, snapshot.clone(), config, embedding_dim).await;

        // Should have processed all 3 samples across 2 epochs (batch of 2 + batch of 1).
        assert_eq!(stats.total_samples, 3);
        assert_eq!(stats.epochs, 2);
        assert_eq!(stats.dropped_events, 0);
        assert!(stats.last_energy.is_finite());
        assert!(stats.last_energy >= 0.0);
        assert_eq!(stats.energy_history.len(), 2);

        // Snapshot should be updated with the trained model.
        let snap = snapshot.read().unwrap();
        assert!(snap.input_dim > 0, "Snapshot input_dim should be > 0 after training");
        assert_eq!(snap.epoch, 2);
        assert_eq!(snap.total_samples, 3);
        assert!(!snap.layer_weights.is_empty());
        assert!(!snap.vocabulary.topics.is_empty());
    }

    #[test]
    fn training_stats_default() {
        let stats = TrainingStats::default();
        assert_eq!(stats.epochs, 0);
        assert_eq!(stats.total_samples, 0);
        assert_eq!(stats.dropped_events, 0);
        assert_eq!(stats.last_energy, 0.0);
        assert!(stats.energy_history.is_empty());
    }

    #[tokio::test]
    async fn auto_save_creates_files_at_interval() {
        let dir = tempfile::tempdir().unwrap();
        let storage_path = dir.path().to_str().unwrap().to_string();

        let config = PcnConfig {
            auto_save_epochs: 2,
            storage_path: Some(storage_path.clone()),
            ..test_config()
        };
        let embedding_dim = 4;
        let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
        let (tx, rx) = mpsc::channel::<InputEvent>(config.channel_capacity);

        // Send 6 events with batch_size=2 → 3 epochs. Auto-save at epoch 2.
        for i in 0..6 {
            tx.send(make_event(vec![0.1 * i as f32; 4], "rust"))
                .await
                .unwrap();
        }
        drop(tx);

        let stats = TrainingWorker::run_batch(rx, snapshot, config, embedding_dim).await;
        assert_eq!(stats.epochs, 3);

        // Should have auto-saved at epoch 2.
        let auto_save_path = format!("{}/pcn-auto-2.json.gz", storage_path);
        assert!(
            std::path::Path::new(&auto_save_path).exists(),
            "Auto-save file should exist at epoch 2: {}",
            auto_save_path
        );

        // Should NOT have auto-saved at epoch 1 or 3.
        assert!(!std::path::Path::new(&format!("{}/pcn-auto-1.json.gz", storage_path)).exists());
        assert!(!std::path::Path::new(&format!("{}/pcn-auto-3.json.gz", storage_path)).exists());
    }

    #[tokio::test]
    async fn auto_save_disabled_when_zero() {
        let dir = tempfile::tempdir().unwrap();
        let storage_path = dir.path().to_str().unwrap().to_string();

        let config = PcnConfig {
            auto_save_epochs: 0,
            storage_path: Some(storage_path.clone()),
            ..test_config()
        };
        let embedding_dim = 4;
        let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
        let (tx, rx) = mpsc::channel::<InputEvent>(config.channel_capacity);

        for i in 0..4 {
            tx.send(make_event(vec![0.1 * i as f32; 4], "rust"))
                .await
                .unwrap();
        }
        drop(tx);

        let stats = TrainingWorker::run_batch(rx, snapshot, config, embedding_dim).await;
        assert_eq!(stats.epochs, 2);

        // No auto-save files should exist.
        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(entries.is_empty(), "No auto-save files when auto_save_epochs=0");
    }

    #[tokio::test]
    async fn auto_save_skipped_when_no_storage_path() {
        let config = PcnConfig {
            auto_save_epochs: 1,
            storage_path: None,
            ..test_config()
        };
        let embedding_dim = 4;
        let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
        let (tx, rx) = mpsc::channel::<InputEvent>(config.channel_capacity);

        tx.send(make_event(vec![0.1; 4], "rust")).await.unwrap();
        tx.send(make_event(vec![0.2; 4], "rust")).await.unwrap();
        drop(tx);

        // Should complete without errors (no storage_path → no auto-save attempt).
        let stats = TrainingWorker::run_batch(rx, snapshot, config, embedding_dim).await;
        assert_eq!(stats.epochs, 1);
    }
}
