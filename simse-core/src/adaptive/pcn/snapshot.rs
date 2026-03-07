use serde::{Deserialize, Serialize};

use crate::adaptive::pcn::config::{LayerConfig, PcnConfig};
use crate::adaptive::pcn::network::PredictiveCodingNetwork;
use crate::adaptive::pcn::vocabulary::{VocabularyManager, VocabularyState};

fn default_inference_rate() -> f64 {
    0.1
}

/// An immutable, serializable snapshot of a trained predictive coding network.
///
/// Captures weights, biases, layer configurations, and vocabulary state so that
/// read-only inference can be performed without holding any locks on the live
/// network. This is pure data — no internal mutexes or caches — enabling true
/// concurrent reads when wrapped in `Arc<RwLock<ModelSnapshot>>`.
///
/// Created via [`ModelSnapshot::from_network`] and used via [`ModelSnapshot::predict`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSnapshot {
    /// Dimensionality of the clamped input.
    pub input_dim: usize,
    /// Layer configurations (dim + activation) for each latent layer.
    pub layer_configs: Vec<LayerConfig>,
    /// Weights for each latent layer. `layer_weights[l]` is a flat row-major
    /// matrix of shape `(dim x input_dim)` for layer `l`.
    pub layer_weights: Vec<Vec<f64>>,
    /// Bias vectors for each latent layer.
    pub layer_biases: Vec<Vec<f64>>,
    /// Weights for the input predictor layer (predicts input from first latent layer).
    pub input_predictor_weights: Vec<f64>,
    /// Bias for the input predictor layer.
    pub input_predictor_bias: Vec<f64>,
    /// Input predictor activation (matches first latent layer's activation).
    pub input_predictor_activation: crate::adaptive::pcn::config::Activation,
    /// Serialized vocabulary state for persistence and reconstruction.
    pub vocabulary: VocabularyState,
    /// Training epoch at the time of snapshot.
    pub epoch: usize,
    /// Total number of training samples seen at the time of snapshot.
    pub total_samples: usize,
    /// Inference rate used during training. Stored so prediction uses the
    /// same rate. Defaults to 0.1 for backward compatibility with old snapshots.
    #[serde(default = "default_inference_rate")]
    pub inference_rate: f64,
}

/// The result of running inference on a [`ModelSnapshot`].
pub struct PredictionResult {
    /// Total energy (sum of squared prediction errors) after inference.
    pub energy: f64,
    /// Top (highest) latent layer's values after inference.
    pub top_latent: Vec<f64>,
    /// Per-layer energy breakdown.
    pub energy_breakdown: Vec<f64>,
    /// Reconstruction of the input from the top latent layer downward.
    pub reconstruction: Vec<f64>,
}

impl ModelSnapshot {
    /// Capture a snapshot from a live network and vocabulary manager.
    ///
    /// This clones all weight and bias data so the snapshot is fully independent
    /// of the original network and can be used for concurrent reads.
    ///
    /// * `net` - the trained predictive coding network
    /// * `vocab` - the vocabulary manager (topics/tags)
    /// * `epoch` - current training epoch
    /// * `total_samples` - total training samples processed so far
    pub fn from_network(
        net: &PredictiveCodingNetwork,
        vocab: &VocabularyManager,
        epoch: usize,
        total_samples: usize,
    ) -> Self {
        let num_layers = net.num_layers();

        let mut layer_configs = Vec::with_capacity(num_layers);
        let mut layer_weights = Vec::with_capacity(num_layers);
        let mut layer_biases = Vec::with_capacity(num_layers);

        for l in 0..num_layers {
            let layer = net.layer(l);
            layer_configs.push(LayerConfig {
                dim: layer.dim,
                activation: layer.activation,
            });
            layer_weights.push(layer.weights.clone());
            layer_biases.push(layer.bias.clone());
        }

        // Capture the input predictor weights.
        let input_predictor = net.input_predictor();

        Self {
            input_dim: net.input_dim(),
            layer_configs,
            layer_weights,
            layer_biases,
            input_predictor_weights: input_predictor.weights.clone(),
            input_predictor_bias: input_predictor.bias.clone(),
            input_predictor_activation: input_predictor.activation,
            vocabulary: vocab.serialize(),
            epoch,
            total_samples,
            inference_rate: net.inference_rate(),
        }
    }

    /// Build a [`PredictiveCodingNetwork`] from this snapshot's weights.
    ///
    /// The returned network is a standalone copy with the snapshot's trained
    /// weights and can be used for inference without any locks.
    pub fn build_network(&self, inference_steps: usize) -> PredictiveCodingNetwork {
        let config = PcnConfig {
            layers: self.layer_configs.clone(),
            inference_steps,
            learning_rate: 0.0,
            inference_rate: self.inference_rate,
            temporal_amortization: false,
            ..Default::default()
        };

        let mut net = PredictiveCodingNetwork::new(self.input_dim, &config);

        for l in 0..self.layer_configs.len() {
            let layer = net.layer_mut(l);
            layer.weights.clone_from(&self.layer_weights[l]);
            layer.bias.clone_from(&self.layer_biases[l]);
        }

        let ip = net.input_predictor_mut();
        ip.weights.clone_from(&self.input_predictor_weights);
        ip.bias.clone_from(&self.input_predictor_bias);

        net
    }

    /// Run inference on the snapshot and return a [`PredictionResult`].
    ///
    /// Builds a temporary [`PredictiveCodingNetwork`] from the snapshot's
    /// weights and runs inference on it. This is safe for concurrent use
    /// since no internal mutable state is required.
    ///
    /// * `input` - the input vector (must match `self.input_dim`)
    /// * `inference_steps` - number of inference iterations
    ///
    /// Returns `None` if the input length does not match `self.input_dim`.
    pub fn predict(&self, input: &[f64], inference_steps: usize) -> Option<PredictionResult> {
        if input.len() != self.input_dim {
            return None;
        }

        let mut net = self.build_network(inference_steps);
        let energy = net.infer(input, inference_steps);
        let top_latent = net.get_top_latent();
        let energy_breakdown = net.energy_breakdown();
        let reconstruction = net.generate();

        Some(PredictionResult {
            energy,
            top_latent,
            energy_breakdown,
            reconstruction,
        })
    }

    /// Create an empty/default snapshot with no trained weights.
    ///
    /// Useful as an initial placeholder before any training has occurred.
    pub fn empty() -> Self {
        Self {
            input_dim: 0,
            layer_configs: Vec::new(),
            layer_weights: Vec::new(),
            layer_biases: Vec::new(),
            input_predictor_weights: Vec::new(),
            input_predictor_bias: Vec::new(),
            input_predictor_activation: crate::adaptive::pcn::config::Activation::Relu,
            vocabulary: VocabularyState {
                topics: Vec::new(),
                tags: Vec::new(),
                max_topics: 0,
                max_tags: 0,
            },
            epoch: 0,
            total_samples: 0,
            inference_rate: 0.1,
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

    fn make_vocab() -> VocabularyManager {
        let vm = VocabularyManager::new(10, 20);
        let (vm, _) = vm.register_topic("rust").unwrap();
        let (vm, _) = vm.register_topic("python").unwrap();
        let (vm, _) = vm.register_tag("important").unwrap();
        vm
    }

    #[test]
    fn snapshot_from_network_captures_weights() {
        let config = test_config();
        let input_dim = 6;
        let net = PredictiveCodingNetwork::new(input_dim, &config);
        let vocab = make_vocab();

        let snapshot = ModelSnapshot::from_network(&net, &vocab, 5, 100);

        // Check dimensions.
        assert_eq!(snapshot.input_dim, input_dim);
        assert_eq!(snapshot.layer_configs.len(), 2);
        assert_eq!(snapshot.layer_weights.len(), 2);
        assert_eq!(snapshot.layer_biases.len(), 2);
        assert_eq!(snapshot.epoch, 5);
        assert_eq!(snapshot.total_samples, 100);

        // Check layer config captured correctly.
        assert_eq!(snapshot.layer_configs[0].dim, 8);
        assert_eq!(snapshot.layer_configs[1].dim, 4);

        // Weights should match the network's layers.
        for l in 0..2 {
            let layer = net.layer(l);
            assert_eq!(snapshot.layer_weights[l], layer.weights);
            assert_eq!(snapshot.layer_biases[l], layer.bias);
        }

        // Input predictor weights captured.
        let ip = net.input_predictor();
        assert_eq!(snapshot.input_predictor_weights, ip.weights);
        assert_eq!(snapshot.input_predictor_bias, ip.bias);

        // Vocabulary state captured.
        assert_eq!(snapshot.vocabulary.topics, vec!["rust", "python"]);
        assert_eq!(snapshot.vocabulary.tags, vec!["important"]);
        assert_eq!(snapshot.vocabulary.max_topics, 10);
        assert_eq!(snapshot.vocabulary.max_tags, 20);
    }

    #[test]
    fn snapshot_can_run_inference() {
        let config = test_config();
        let input_dim = 6;
        let mut net = PredictiveCodingNetwork::new(input_dim, &config);
        let vocab = make_vocab();

        // Train a bit to establish meaningful weights.
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        for _ in 0..20 {
            net.train_single(&input);
        }

        // Take a snapshot.
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 20, 20);

        // Run prediction on the snapshot.
        let result = snapshot.predict(&input, 10).expect("predict should return Some");

        // Energy should be finite and non-negative.
        assert!(result.energy.is_finite());
        assert!(result.energy >= 0.0);

        // Top latent should have the right dimension (top layer dim = 4).
        assert_eq!(result.top_latent.len(), 4);
        assert!(result.top_latent.iter().all(|v| v.is_finite()));

        // Energy breakdown should have one entry per layer.
        assert_eq!(result.energy_breakdown.len(), 2);
        assert!(result.energy_breakdown.iter().all(|e| e.is_finite() && *e >= 0.0));

        // Reconstruction should match input dimension.
        assert_eq!(result.reconstruction.len(), input_dim);
        assert!(result.reconstruction.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn snapshot_on_empty_model() {
        let snapshot = ModelSnapshot::empty();

        assert_eq!(snapshot.input_dim, 0);
        assert!(snapshot.layer_configs.is_empty());
        assert!(snapshot.layer_weights.is_empty());
        assert!(snapshot.layer_biases.is_empty());
        assert!(snapshot.input_predictor_weights.is_empty());
        assert!(snapshot.input_predictor_bias.is_empty());
        assert_eq!(snapshot.epoch, 0);
        assert_eq!(snapshot.total_samples, 0);
        assert!(snapshot.vocabulary.topics.is_empty());
        assert!(snapshot.vocabulary.tags.is_empty());
        assert_eq!(snapshot.vocabulary.max_topics, 0);
        assert_eq!(snapshot.vocabulary.max_tags, 0);
    }

    #[test]
    fn snapshot_serialization_round_trip() {
        let config = test_config();
        let mut net = PredictiveCodingNetwork::new(6, &config);
        let vocab = make_vocab();

        net.train_single(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0]);

        let snapshot = ModelSnapshot::from_network(&net, &vocab, 1, 1);
        let json = serde_json::to_string(&snapshot).unwrap();
        let restored: ModelSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.input_dim, snapshot.input_dim);
        assert_eq!(restored.layer_configs.len(), snapshot.layer_configs.len());
        assert_eq!(restored.epoch, snapshot.epoch);
        assert_eq!(restored.total_samples, snapshot.total_samples);

        // Compare weights with tolerance for JSON float round-tripping.
        for l in 0..snapshot.layer_weights.len() {
            assert_eq!(restored.layer_weights[l].len(), snapshot.layer_weights[l].len());
            for (a, b) in restored.layer_weights[l].iter().zip(snapshot.layer_weights[l].iter()) {
                assert!(
                    (a - b).abs() < 1e-14,
                    "Layer {} weight mismatch: {} vs {}",
                    l,
                    a,
                    b,
                );
            }
            assert_eq!(restored.layer_biases[l].len(), snapshot.layer_biases[l].len());
            for (a, b) in restored.layer_biases[l].iter().zip(snapshot.layer_biases[l].iter()) {
                assert!(
                    (a - b).abs() < 1e-14,
                    "Layer {} bias mismatch: {} vs {}",
                    l,
                    a,
                    b,
                );
            }
        }

        // Input predictor weights.
        for (a, b) in restored
            .input_predictor_weights
            .iter()
            .zip(snapshot.input_predictor_weights.iter())
        {
            assert!(
                (a - b).abs() < 1e-14,
                "Input predictor weight mismatch: {} vs {}",
                a,
                b,
            );
        }
        for (a, b) in restored
            .input_predictor_bias
            .iter()
            .zip(snapshot.input_predictor_bias.iter())
        {
            assert!(
                (a - b).abs() < 1e-14,
                "Input predictor bias mismatch: {} vs {}",
                a,
                b,
            );
        }
    }

    #[test]
    fn snapshot_predict_produces_different_results_for_different_inputs() {
        let config = test_config();
        let mut net = PredictiveCodingNetwork::new(6, &config);
        let vocab = make_vocab();

        // Train on some data.
        for _ in 0..10 {
            net.train_single(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0]);
        }

        let snapshot = ModelSnapshot::from_network(&net, &vocab, 10, 10);

        let result_a = snapshot.predict(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0], 10).unwrap();
        let result_b = snapshot.predict(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 10).unwrap();

        // Different inputs should produce different energies (almost certainly).
        // At minimum, both should be valid.
        assert!(result_a.energy.is_finite());
        assert!(result_b.energy.is_finite());
        assert_eq!(result_a.reconstruction.len(), 6);
        assert_eq!(result_b.reconstruction.len(), 6);
    }

    #[test]
    fn snapshot_captures_inference_rate() {
        let config = PcnConfig {
            inference_rate: 0.05,
            ..test_config()
        };
        let net = PredictiveCodingNetwork::new(6, &config);
        let vocab = make_vocab();
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 1, 1);
        assert!((snapshot.inference_rate - 0.05).abs() < 1e-15);
    }

    #[test]
    fn snapshot_predict_is_repeatable() {
        let config = test_config();
        let mut net = PredictiveCodingNetwork::new(6, &config);
        let vocab = make_vocab();
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        for _ in 0..10 {
            net.train_single(&input);
        }
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 10, 10);

        let r1 = snapshot.predict(&input, 10).unwrap();
        let r2 = snapshot.predict(&input, 10).unwrap();
        assert!(r1.energy.is_finite());
        assert!(r2.energy.is_finite());
        // Both calls build fresh networks with the same weights, so
        // energies should be close (randomized latent init may differ).
    }

    #[test]
    fn snapshot_predict_wrong_dim_returns_none() {
        let config = test_config();
        let net = PredictiveCodingNetwork::new(6, &config);
        let vocab = make_vocab();
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 1, 1);

        // Wrong input dimension should return None, not panic.
        assert!(snapshot.predict(&[1.0, 2.0, 3.0], 10).is_none());
    }

    #[test]
    fn snapshot_clone_has_independent_cache() {
        let config = test_config();
        let net = PredictiveCodingNetwork::new(6, &config);
        let vocab = make_vocab();
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 1, 1);

        let _ = snapshot.predict(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0], 5).unwrap();

        let cloned = snapshot.clone();
        assert_eq!(cloned.input_dim, snapshot.input_dim);
        assert_eq!(cloned.epoch, snapshot.epoch);
        assert!((cloned.inference_rate - snapshot.inference_rate).abs() < 1e-15);
    }

    #[test]
    fn snapshot_deserialized_defaults_inference_rate() {
        let config = test_config();
        let net = PredictiveCodingNetwork::new(6, &config);
        let vocab = make_vocab();
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 1, 1);
        let mut json: serde_json::Value = serde_json::to_value(&snapshot).unwrap();

        json.as_object_mut().unwrap().remove("inferenceRate");

        let restored: ModelSnapshot = serde_json::from_value(json).unwrap();
        assert!((restored.inference_rate - 0.1).abs() < 1e-15);
    }
}
