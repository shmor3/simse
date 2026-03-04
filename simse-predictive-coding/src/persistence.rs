use std::io::{Read, Write};

use crate::error::PcnError;
use crate::snapshot::ModelSnapshot;

/// Save a [`ModelSnapshot`] to a file as JSON, optionally gzip-compressed.
///
/// * `snapshot` - the model snapshot to persist
/// * `path` - destination file path
/// * `compress` - if `true`, the JSON is gzip-compressed before writing
pub fn save_snapshot(snapshot: &ModelSnapshot, path: &str, compress: bool) -> Result<(), PcnError> {
    let json = serde_json::to_vec(snapshot)?;
    if compress {
        let mut encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&json).map_err(PcnError::Io)?;
        let compressed = encoder.finish().map_err(PcnError::Io)?;
        std::fs::write(path, compressed).map_err(PcnError::Io)?;
    } else {
        std::fs::write(path, json).map_err(PcnError::Io)?;
    }
    Ok(())
}

/// Load a [`ModelSnapshot`] from a file, optionally gzip-compressed.
///
/// * `path` - source file path
/// * `compressed` - if `true`, the file is decompressed before JSON parsing
pub fn load_snapshot(path: &str, compressed: bool) -> Result<ModelSnapshot, PcnError> {
    let data = std::fs::read(path).map_err(PcnError::Io)?;
    if compressed {
        let mut decoder = flate2::read::GzDecoder::new(&data[..]);
        let mut json = Vec::new();
        decoder.read_to_end(&mut json).map_err(PcnError::Io)?;
        let snapshot: ModelSnapshot = serde_json::from_slice(&json)?;
        Ok(snapshot)
    } else {
        let snapshot: ModelSnapshot = serde_json::from_slice(&data)?;
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Activation, LayerConfig, PcnConfig};
    use crate::network::PredictiveCodingNetwork;
    use crate::vocabulary::VocabularyManager;

    fn assert_vecs_approx(a: &[f64], b: &[f64], tol: f64) {
        assert_eq!(a.len(), b.len(), "Vector length mismatch: {} vs {}", a.len(), b.len());
        for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
            assert!(
                (x - y).abs() < tol,
                "Mismatch at index {}: {} vs {} (diff {})",
                i, x, y, (x - y).abs()
            );
        }
    }

    fn make_snapshot() -> ModelSnapshot {
        let config = PcnConfig {
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
        };

        let mut net = PredictiveCodingNetwork::new(6, &config);
        let mut vocab = VocabularyManager::new(10, 20);
        vocab.register_topic("rust").unwrap();
        vocab.register_tag("important").unwrap();

        // Train a bit so weights are non-trivial.
        for _ in 0..5 {
            net.train_single(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0]);
        }

        ModelSnapshot::from_network(&net, &vocab, 5, 5)
    }

    #[test]
    fn save_load_round_trip_json() {
        let snapshot = make_snapshot();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.json");
        let path_str = path.to_str().unwrap();

        save_snapshot(&snapshot, path_str, false).unwrap();
        let loaded = load_snapshot(path_str, false).unwrap();

        assert_eq!(loaded.input_dim, snapshot.input_dim);
        assert_eq!(loaded.epoch, snapshot.epoch);
        assert_eq!(loaded.total_samples, snapshot.total_samples);
        assert_eq!(loaded.layer_configs.len(), snapshot.layer_configs.len());

        let tol = 1e-14;
        for (lw, sw) in loaded.layer_weights.iter().zip(snapshot.layer_weights.iter()) {
            assert_vecs_approx(lw, sw, tol);
        }
        for (lb, sb) in loaded.layer_biases.iter().zip(snapshot.layer_biases.iter()) {
            assert_vecs_approx(lb, sb, tol);
        }
        assert_vecs_approx(&loaded.input_predictor_weights, &snapshot.input_predictor_weights, tol);
        assert_vecs_approx(&loaded.input_predictor_bias, &snapshot.input_predictor_bias, tol);
        assert_eq!(loaded.vocabulary.topics, snapshot.vocabulary.topics);
        assert_eq!(loaded.vocabulary.tags, snapshot.vocabulary.tags);
    }

    #[test]
    fn save_load_round_trip_gzip() {
        let snapshot = make_snapshot();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.json.gz");
        let path_str = path.to_str().unwrap();

        save_snapshot(&snapshot, path_str, true).unwrap();

        // The file should exist and be smaller than the raw JSON would be.
        let compressed_size = std::fs::metadata(path_str).unwrap().len();
        let raw_json = serde_json::to_vec(&snapshot).unwrap();
        // Compressed should be non-empty (just a basic sanity check).
        assert!(compressed_size > 0);
        // For non-trivial data, gzip should produce a different byte count than raw JSON.
        assert_ne!(compressed_size as usize, raw_json.len());

        let loaded = load_snapshot(path_str, true).unwrap();

        assert_eq!(loaded.input_dim, snapshot.input_dim);
        assert_eq!(loaded.epoch, snapshot.epoch);
        assert_eq!(loaded.total_samples, snapshot.total_samples);

        let tol = 1e-14;
        for (lw, sw) in loaded.layer_weights.iter().zip(snapshot.layer_weights.iter()) {
            assert_vecs_approx(lw, sw, tol);
        }
        for (lb, sb) in loaded.layer_biases.iter().zip(snapshot.layer_biases.iter()) {
            assert_vecs_approx(lb, sb, tol);
        }
        assert_vecs_approx(&loaded.input_predictor_weights, &snapshot.input_predictor_weights, tol);
        assert_vecs_approx(&loaded.input_predictor_bias, &snapshot.input_predictor_bias, tol);
        assert_eq!(loaded.vocabulary.topics, snapshot.vocabulary.topics);
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = load_snapshot("/tmp/does_not_exist_simse_test_12345.json", false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), "PCN_IO_ERROR");
    }
}
