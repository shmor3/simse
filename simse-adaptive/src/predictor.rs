use std::sync::{Arc, RwLock};

use crate::snapshot::{ModelSnapshot, PredictionResult};

/// Summary statistics about the current model state.
pub struct ModelStats {
    /// Training epoch at the time of snapshot.
    pub epoch: usize,
    /// Total number of training samples seen.
    pub total_samples: usize,
    /// Number of latent layers.
    pub num_layers: usize,
    /// Dimensionality of the clamped input.
    pub input_dim: usize,
    /// Dimensions of each latent layer (bottom to top).
    pub layer_dims: Vec<usize>,
    /// Total number of trainable parameters (weights + biases across all layers
    /// including the input predictor).
    pub parameter_count: usize,
}

/// A read-only query interface that runs inference against a shared [`ModelSnapshot`].
///
/// The `Predictor` holds an `Arc<RwLock<ModelSnapshot>>` and acquires a read lock
/// to answer prediction queries. This allows concurrent reads while a
/// `TrainingWorker` periodically publishes updated snapshots via a write lock.
pub struct Predictor {
    snapshot: Arc<RwLock<ModelSnapshot>>,
    inference_steps: usize,
}

impl Predictor {
    /// Create a new predictor.
    ///
    /// * `snapshot` - shared snapshot updated by the training worker
    /// * `inference_steps` - number of inference iterations to run per prediction
    pub fn new(snapshot: Arc<RwLock<ModelSnapshot>>, inference_steps: usize) -> Self {
        Self {
            snapshot,
            inference_steps,
        }
    }

    /// Run inference on the current model snapshot and return a [`PredictionResult`].
    ///
    /// Returns `None` if the snapshot is empty (no trained model yet, i.e.
    /// `input_dim == 0` or no layers).
    pub fn confidence(&self, input: &[f64]) -> Option<PredictionResult> {
        let snap = self.snapshot.read().unwrap();

        if snap.input_dim == 0 || snap.layer_configs.is_empty() {
            return None;
        }

        if input.len() != snap.input_dim {
            return None;
        }

        Some(snap.predict(input, self.inference_steps))
    }

    /// Find the inputs with the highest prediction energy (most anomalous).
    ///
    /// Returns up to `top_k` `(index, energy)` pairs sorted by energy descending.
    /// Inputs that cannot be scored (e.g. wrong dimension or empty snapshot) are
    /// skipped.
    pub fn anomalies(&self, inputs: &[Vec<f64>], top_k: usize) -> Vec<(usize, f64)> {
        let snap = self.snapshot.read().unwrap();

        if snap.input_dim == 0 || snap.layer_configs.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<(usize, f64)> = inputs
            .iter()
            .enumerate()
            .filter_map(|(i, input)| {
                if input.len() != snap.input_dim {
                    return None;
                }
                let result = snap.predict(input, self.inference_steps);
                Some((i, result.energy))
            })
            .collect();

        // Sort descending by energy.
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    /// Compute summary statistics about the current model.
    pub fn model_stats(&self) -> ModelStats {
        let snap = self.snapshot.read().unwrap();

        let layer_dims: Vec<usize> = snap.layer_configs.iter().map(|lc| lc.dim).collect();

        // Count parameters:
        //   Input predictor: weights (input_dim x layers[0].dim) + bias (input_dim)
        //   Each latent layer l: weights (dim x input_dim_for_layer) + bias (dim)
        //
        // For weight matrices stored as flat vectors, we use the actual vector
        // lengths which equal rows * cols.
        let mut parameter_count = 0;

        // Input predictor parameters.
        parameter_count += snap.input_predictor_weights.len();
        parameter_count += snap.input_predictor_bias.len();

        // Latent layer parameters.
        for l in 0..snap.layer_weights.len() {
            parameter_count += snap.layer_weights[l].len();
            parameter_count += snap.layer_biases[l].len();
        }

        ModelStats {
            epoch: snap.epoch,
            total_samples: snap.total_samples,
            num_layers: snap.layer_configs.len(),
            input_dim: snap.input_dim,
            layer_dims,
            parameter_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Activation, LayerConfig, PcnConfig};
    use crate::network::PredictiveCodingNetwork;
    use crate::vocabulary::VocabularyManager;

    fn test_config() -> PcnConfig {
        PcnConfig {
            layers: vec![
                LayerConfig {
                    dim: 8,
                    activation: Activation::Relu,
                },
                LayerConfig {
                    dim: 4,
                    activation: Activation::Tanh,
                },
            ],
            inference_steps: 10,
            learning_rate: 0.01,
            inference_rate: 0.1,
            temporal_amortization: false,
            ..Default::default()
        }
    }

    fn make_trained_snapshot() -> ModelSnapshot {
        let config = test_config();
        let input_dim = 6;
        let mut net = PredictiveCodingNetwork::new(input_dim, &config);
        let vocab = VocabularyManager::new(10, 20);

        // Train on a few samples to establish meaningful weights.
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        for _ in 0..20 {
            net.train_single(&input);
        }

        ModelSnapshot::from_network(&net, &vocab, 20, 100)
    }

    #[test]
    fn predict_confidence_returns_energy() {
        let snapshot = Arc::new(RwLock::new(make_trained_snapshot()));
        let predictor = Predictor::new(snapshot, 10);

        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        let result = predictor.confidence(&input);

        assert!(result.is_some(), "Confidence should return Some for a trained model");
        let result = result.unwrap();

        assert!(result.energy.is_finite());
        assert!(result.energy >= 0.0);
        assert_eq!(result.top_latent.len(), 4); // top layer dim
        assert_eq!(result.energy_breakdown.len(), 2); // 2 latent layers
        assert_eq!(result.reconstruction.len(), 6); // input_dim
    }

    #[test]
    fn predict_anomalies_returns_sorted_by_energy() {
        let snapshot = Arc::new(RwLock::new(make_trained_snapshot()));
        let predictor = Predictor::new(snapshot, 10);

        let inputs = vec![
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0],
            vec![10.0, -10.0, 10.0, -10.0, 10.0, -10.0],
            vec![0.1, 0.1, 0.1, 0.1, 0.1, 0.1],
        ];

        let anomalies = predictor.anomalies(&inputs, 3);

        // Should return at most top_k results.
        assert!(anomalies.len() <= 3);
        assert!(!anomalies.is_empty());

        // Should be sorted descending by energy.
        for w in anomalies.windows(2) {
            assert!(
                w[0].1 >= w[1].1,
                "Anomalies should be sorted descending by energy: {} >= {}",
                w[0].1,
                w[1].1,
            );
        }

        // All energies should be finite and non-negative.
        for (_, energy) in &anomalies {
            assert!(energy.is_finite());
            assert!(*energy >= 0.0);
        }
    }

    #[test]
    fn predict_on_empty_snapshot_returns_none() {
        let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
        let predictor = Predictor::new(snapshot, 10);

        let result = predictor.confidence(&[1.0, 2.0, 3.0]);
        assert!(result.is_none(), "Confidence on empty snapshot should return None");

        let anomalies = predictor.anomalies(&[vec![1.0, 2.0]], 5);
        assert!(anomalies.is_empty(), "Anomalies on empty snapshot should be empty");
    }

    #[test]
    fn model_stats_returns_metadata() {
        let snapshot = Arc::new(RwLock::new(make_trained_snapshot()));
        let predictor = Predictor::new(snapshot, 10);

        let stats = predictor.model_stats();

        assert_eq!(stats.epoch, 20);
        assert_eq!(stats.total_samples, 100);
        assert_eq!(stats.num_layers, 2);
        assert_eq!(stats.input_dim, 6);
        assert_eq!(stats.layer_dims, vec![8, 4]);

        // Verify parameter count.
        //
        // Input predictor: dim=6 (input_dim), input_dim=8 (layers[0].dim)
        //   weights: 6 * 8 = 48, bias: 6 => 54
        //
        // Layer 0: dim=8, input_dim=4 (layers[1].dim)
        //   weights: 8 * 4 = 32, bias: 8 => 40
        //
        // Layer 1 (top): dim=4, input_dim=4 (self-loop, own dim)
        //   weights: 4 * 4 = 16, bias: 4 => 20
        //
        // Total: 54 + 40 + 20 = 114
        assert_eq!(stats.parameter_count, 114);
    }

    #[test]
    fn model_stats_on_empty_snapshot() {
        let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
        let predictor = Predictor::new(snapshot, 10);

        let stats = predictor.model_stats();

        assert_eq!(stats.epoch, 0);
        assert_eq!(stats.total_samples, 0);
        assert_eq!(stats.num_layers, 0);
        assert_eq!(stats.input_dim, 0);
        assert!(stats.layer_dims.is_empty());
        assert_eq!(stats.parameter_count, 0);
    }

    #[test]
    fn confidence_with_wrong_input_dim_returns_none() {
        let snapshot = Arc::new(RwLock::new(make_trained_snapshot()));
        let predictor = Predictor::new(snapshot, 10);

        // Model expects input_dim=6, send 3.
        let result = predictor.confidence(&[1.0, 2.0, 3.0]);
        assert!(result.is_none(), "Wrong input dim should return None");
    }
}
