//! Model persistence: save and load round-trip verification.
//!
//! Trains a model, saves it as both raw JSON and gzip-compressed JSON,
//! loads both back, and verifies that predictions match the original.
//!
//! Run: `cargo run --example model_persistence`

use simse_pcn_engine::config::{Activation, LayerConfig, PcnConfig};
use simse_pcn_engine::network::PredictiveCodingNetwork;
use simse_pcn_engine::persistence::{load_snapshot, save_snapshot};
use simse_pcn_engine::snapshot::ModelSnapshot;
use simse_pcn_engine::vocabulary::VocabularyManager;

fn main() {
    println!("=== Predictive Coding Network: Model Persistence ===\n");

    // ---------------------------------------------------------------
    // 1. Train a small model
    // ---------------------------------------------------------------
    let input_dim = 24; // 4 + 5 + 5 + 10
    let config = PcnConfig {
        layers: vec![
            LayerConfig { dim: 16, activation: Activation::Relu },
            LayerConfig { dim: 8, activation: Activation::Tanh },
        ],
        inference_steps: 20,
        learning_rate: 0.005,
        inference_rate: 0.1,
        temporal_amortization: false,
        max_topics: 5,
        max_tags: 5,
        ..Default::default()
    };

    let mut network = PredictiveCodingNetwork::new(input_dim, &config);
    let mut vocab = VocabularyManager::new(5, 5);
    vocab.register_topic("rust").unwrap();
    vocab.register_topic("python").unwrap();
    vocab.register_tag("core").unwrap();
    vocab.register_tag("important").unwrap();

    // Training data (values kept in [0,1] range to avoid weight explosion).
    let inputs: Vec<Vec<f64>> = vec![
        vec![0.5, 0.3, 0.7, 0.2, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.8, 0.5, 0.1, 1.0, 0.0, 0.0, 0.0],
        vec![0.8, 0.1, 0.4, 0.6, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.9, 0.7, 0.2, 0.0, 1.0, 0.0, 0.0],
    ];

    println!("Training for 50 epochs on {} samples...", inputs.len());
    for _ in 0..50 {
        for input in &inputs {
            network.train_single(input);
        }
    }

    let snapshot = ModelSnapshot::from_network(&network, &vocab, 50, 100);
    println!("Snapshot: epoch={}, samples={}, input_dim={}", snapshot.epoch, snapshot.total_samples, snapshot.input_dim);

    // ---------------------------------------------------------------
    // 2. Save as JSON and gzip
    // ---------------------------------------------------------------
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("model.json");
    let gz_path = dir.path().join("model.json.gz");

    save_snapshot(&snapshot, json_path.to_str().unwrap(), false).unwrap();
    save_snapshot(&snapshot, gz_path.to_str().unwrap(), true).unwrap();

    let json_size = std::fs::metadata(&json_path).unwrap().len();
    let gz_size = std::fs::metadata(&gz_path).unwrap().len();
    let ratio = (gz_size as f64 / json_size as f64) * 100.0;

    println!("\n--- File Sizes ---");
    println!("  JSON:  {:>8} bytes", json_size);
    println!("  Gzip:  {:>8} bytes ({:.1}% of JSON)", gz_size, ratio);

    // ---------------------------------------------------------------
    // 3. Load both back
    // ---------------------------------------------------------------
    let loaded_json = load_snapshot(json_path.to_str().unwrap(), false).unwrap();
    let loaded_gz = load_snapshot(gz_path.to_str().unwrap(), true).unwrap();

    println!("\n--- Round-Trip Verification ---");

    // Run prediction on the original and both loaded snapshots.
    let test_input = &inputs[0];

    let result_original = snapshot.predict(test_input, 20);
    let result_json = loaded_json.predict(test_input, 20);
    let result_gz = loaded_gz.predict(test_input, 20);

    println!("  Original energy:    {:.10}", result_original.energy);
    println!("  JSON-loaded energy: {:.10}", result_json.energy);
    println!("  Gzip-loaded energy: {:.10}", result_gz.energy);

    // Verify reconstruction matches.
    let max_diff_json: f64 = result_original
        .reconstruction
        .iter()
        .zip(result_json.reconstruction.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f64, f64::max);

    let max_diff_gz: f64 = result_original
        .reconstruction
        .iter()
        .zip(result_gz.reconstruction.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f64, f64::max);

    println!();
    println!("  Max reconstruction diff (JSON): {:.2e}", max_diff_json);
    println!("  Max reconstruction diff (Gzip): {:.2e}", max_diff_gz);

    let json_pass = max_diff_json < 1e-10;
    let gz_pass = max_diff_gz < 1e-10;

    println!();
    println!("  JSON round-trip: {}", if json_pass { "PASS" } else { "FAIL" });
    println!("  Gzip round-trip: {}", if gz_pass { "PASS" } else { "FAIL" });

    // Verify metadata.
    assert_eq!(loaded_json.epoch, snapshot.epoch);
    assert_eq!(loaded_json.total_samples, snapshot.total_samples);
    assert_eq!(loaded_json.input_dim, snapshot.input_dim);
    assert_eq!(loaded_gz.epoch, snapshot.epoch);
    assert_eq!(loaded_gz.total_samples, snapshot.total_samples);
    assert_eq!(loaded_gz.input_dim, snapshot.input_dim);

    println!("  Metadata match: PASS");
    println!("\nDone.");
}
