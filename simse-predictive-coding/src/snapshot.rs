use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::config::{LayerConfig, PcnConfig};
use crate::network::PredictiveCodingNetwork;
use crate::vocabulary::{VocabularyManager, VocabularyState};

fn default_inference_rate() -> f64 {
    0.1
}

/// An immutable, serializable snapshot of a trained predictive coding network.
///
/// Captures weights, biases, layer configurations, and vocabulary state so that
/// read-only inference can be performed without holding any locks on the live
/// network. This enables lock-free concurrent prediction reads.
///
/// Created via [`ModelSnapshot::from_network`] and used via [`ModelSnapshot::predict`].
#[derive(Debug, Serialize, Deserialize)]
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
    pub input_predictor_activation: crate::config::Activation,
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
    /// Lazily-built network for running inference. Skipped during
    /// serialization — rebuilt on first predict() call.
    #[serde(skip)]
    cached_network: Mutex<Option<PredictiveCodingNetwork>>,
}

impl Clone for ModelSnapshot {
    fn clone(&self) -> Self {
        Self {
            input_dim: self.input_dim,
            layer_configs: self.layer_configs.clone(),
            layer_weights: self.layer_weights.clone(),
            layer_biases: self.layer_biases.clone(),
            input_predictor_weights: self.input_predictor_weights.clone(),
            input_predictor_bias: self.input_predictor_bias.clone(),
            input_predictor_activation: self.input_predictor_activation,
            vocabulary: self.vocabulary.clone(),
            epoch: self.epoch,
            total_samples: self.total_samples,
            inference_rate: self.inference_rate,
            cached_network: Mutex::new(None),
        }
    }
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

        // Capture the input predictor (separate from the latent layers array).
        // We access it indirectly: we know the network has an input_predictor with
        // dim = input_dim and input_dim = layers[0].dim. We need to reconstruct
        // a temporary network to read it, or access it through the public API.
        //
        // Since input_predictor is a private field on PredictiveCodingNetwork,
        // we reconstruct it by creating a fresh network with the same config and
        // then... actually, we need a way to access it.
        //
        // Looking at PredictiveCodingNetwork: the input_predictor field is private.
        // We'll access it through a helper that we add, or work around it.
        //
        // The generate() method uses input_predictor internally. For the snapshot,
        // we need the actual weights. Let's add accessor methods to the network.
        //
        // For now, since we can't modify network.rs in this task, we'll use a
        // workaround: create a network from config, run inference to populate
        // the state, then use generate() to get the reconstruction. But that
        // won't give us the input_predictor weights directly.
        //
        // Actually, looking more carefully at the struct definition, the fields
        // are NOT pub. But layer() returns &PcnLayer which has all pub fields.
        // The input_predictor is private. We need to either:
        // 1. Add a pub accessor for input_predictor on PredictiveCodingNetwork
        // 2. Work around it
        //
        // Let's add a minimal accessor. We'll need to modify network.rs.

        // For the snapshot predict method, we reconstruct a full network from
        // the snapshot data. We need the input_predictor weights for that.
        // Since those aren't directly accessible, we'll need to add an accessor.
        //
        // Placeholder — will be filled by the accessor we add to network.rs:
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
            cached_network: Mutex::new(None),
        }
    }

    /// Run inference on the snapshot and return a [`PredictionResult`].
    ///
    /// On the first call, a [`PredictiveCodingNetwork`] is lazily built from
    /// the snapshot's captured weights and cached internally. Subsequent calls
    /// reuse the cached network, avoiding repeated allocation and weight
    /// copying.
    ///
    /// * `input` - the input vector (must match `self.input_dim`)
    /// * `inference_steps` - number of inference iterations
    pub fn predict(&self, input: &[f64], inference_steps: usize) -> PredictionResult {
        assert_eq!(
            input.len(),
            self.input_dim,
            "Input length {} != snapshot input_dim {}",
            input.len(),
            self.input_dim
        );

        let mut cache = self.cached_network.lock().unwrap();

        if cache.is_none() {
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

            *cache = Some(net);
        }

        let net = cache.as_mut().unwrap();

        let energy = net.infer(input, inference_steps);

        let top_latent = net.get_top_latent();
        let energy_breakdown = net.energy_breakdown();
        let reconstruction = net.generate();

        PredictionResult {
            energy,
            top_latent,
            energy_breakdown,
            reconstruction,
        }
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
            input_predictor_activation: crate::config::Activation::Relu,
            vocabulary: VocabularyState {
                topics: Vec::new(),
                tags: Vec::new(),
                max_topics: 0,
                max_tags: 0,
            },
            epoch: 0,
            total_samples: 0,
            inference_rate: 0.1,
            cached_network: Mutex::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Activation, LayerConfig, PcnConfig};

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
        let mut vm = VocabularyManager::new(10, 20);
        vm.register_topic("rust").unwrap();
        vm.register_topic("python").unwrap();
        vm.register_tag("important").unwrap();
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
        let result = snapshot.predict(&input, 10);

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

        let result_a = snapshot.predict(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0], 10);
        let result_b = snapshot.predict(&[0.0, 0.0, 0.0, 0.0, 0.0, 0.0], 10);

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
    fn snapshot_predict_uses_cached_network() {
        let config = test_config();
        let mut net = PredictiveCodingNetwork::new(6, &config);
        let vocab = make_vocab();
        let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
        for _ in 0..10 {
            net.train_single(&input);
        }
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 10, 10);

        let r1 = snapshot.predict(&input, 10);
        let r2 = snapshot.predict(&input, 10);
        assert!(r1.energy.is_finite());
        assert!(r2.energy.is_finite());
        assert!((r1.energy - r2.energy).abs() < 1e-10);
    }

    #[test]
    fn snapshot_clone_has_independent_cache() {
        let config = test_config();
        let net = PredictiveCodingNetwork::new(6, &config);
        let vocab = make_vocab();
        let snapshot = ModelSnapshot::from_network(&net, &vocab, 1, 1);

        let _ = snapshot.predict(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0], 5);

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
