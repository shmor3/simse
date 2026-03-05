//! End-to-end training workflow for a predictive coding network.
//!
//! Demonstrates: network creation, event encoding, training loop with
//! energy convergence, snapshot creation, and prediction.
//!
//! Run: `cargo run --example training_basics`

use simse_pcn_engine::config::{Activation, LayerConfig, PcnConfig};
use simse_pcn_engine::encoder::{InputEncoder, LibraryEvent};
use simse_pcn_engine::network::PredictiveCodingNetwork;
use simse_pcn_engine::predictor::Predictor;
use simse_pcn_engine::snapshot::ModelSnapshot;
use std::sync::{Arc, RwLock};

fn main() {
    println!("=== Predictive Coding Network: Training Basics ===\n");

    // ---------------------------------------------------------------
    // 1. Configure a small network
    // ---------------------------------------------------------------
    let embedding_dim = 4;
    let max_topics = 10;
    let max_tags = 10;

    let config = PcnConfig {
        layers: vec![
            LayerConfig { dim: 16, activation: Activation::Relu },
            LayerConfig { dim: 8, activation: Activation::Tanh },
        ],
        inference_steps: 20,
        learning_rate: 0.001,
        inference_rate: 0.1,
        temporal_amortization: false,
        max_topics,
        max_tags,
        ..Default::default()
    };

    // ---------------------------------------------------------------
    // 2. Create encoder and network
    // ---------------------------------------------------------------
    let mut encoder = InputEncoder::new(embedding_dim, max_topics, max_tags);

    // Pre-register vocabulary so input dim is stable.
    let topics = ["rust", "python", "go", "typescript", "sql"];
    let tags = ["core", "important", "experimental", "archived", "pinned"];
    for t in &topics {
        encoder.vocab_mut().register_topic(t).unwrap();
    }
    for t in &tags {
        encoder.vocab_mut().register_tag(t).unwrap();
    }

    let input_dim = encoder.current_input_dim();
    let mut network = PredictiveCodingNetwork::new(input_dim, &config);

    println!("Model architecture:");
    println!("  Input dimension: {}", input_dim);
    println!("  Layers: {:?}", config.layers.iter().map(|l| l.dim).collect::<Vec<_>>());
    println!("  Inference steps: {}", config.inference_steps);
    println!("  Learning rate: {}", config.learning_rate);
    println!();

    // ---------------------------------------------------------------
    // 3. Create training events
    // ---------------------------------------------------------------
    let events = vec![
        LibraryEvent {
            embedding: vec![0.8, 0.2, 0.1, 0.9],
            topic: "rust".into(),
            tags: vec!["core".into(), "important".into()],
            entry_type: "fact".into(),
            timestamp: 100.0,
            time_since_last: 0.0,
            session_ordinal: 1.0,
            action: "extraction".into(),
        },
        LibraryEvent {
            embedding: vec![0.7, 0.3, 0.2, 0.8],
            topic: "rust".into(),
            tags: vec!["core".into()],
            entry_type: "decision".into(),
            timestamp: 105.0,
            time_since_last: 5.0,
            session_ordinal: 2.0,
            action: "extraction".into(),
        },
        LibraryEvent {
            embedding: vec![0.1, 0.9, 0.8, 0.2],
            topic: "python".into(),
            tags: vec!["experimental".into()],
            entry_type: "observation".into(),
            timestamp: 110.0,
            time_since_last: 5.0,
            session_ordinal: 3.0,
            action: "compendium".into(),
        },
        LibraryEvent {
            embedding: vec![0.3, 0.6, 0.5, 0.4],
            topic: "go".into(),
            tags: vec!["important".into(), "pinned".into()],
            entry_type: "fact".into(),
            timestamp: 115.0,
            time_since_last: 5.0,
            session_ordinal: 4.0,
            action: "reorganization".into(),
        },
        LibraryEvent {
            embedding: vec![0.5, 0.5, 0.3, 0.7],
            topic: "typescript".into(),
            tags: vec!["core".into(), "archived".into()],
            entry_type: "decision".into(),
            timestamp: 120.0,
            time_since_last: 5.0,
            session_ordinal: 5.0,
            action: "optimization".into(),
        },
    ];

    // Encode all events.
    let encoded: Vec<Vec<f64>> = events
        .iter()
        .map(|ev| encoder.encode(ev).unwrap().0)
        .collect();

    println!("Training data: {} events, {} dimensions each\n", encoded.len(), input_dim);

    // ---------------------------------------------------------------
    // 4. Train for 50 epochs
    // ---------------------------------------------------------------
    println!("{:<8} {:<15}", "Epoch", "Avg Energy");
    println!("{:-<8} {:-<15}", "", "");

    let num_epochs = 50;
    let mut energy_history = Vec::with_capacity(num_epochs);

    for epoch in 0..num_epochs {
        let mut epoch_energy = 0.0;
        for sample in &encoded {
            let energy = network.train_single_with_steps(
                sample,
                config.inference_steps,
                config.temporal_amortization,
            );
            epoch_energy += energy;
        }
        let avg = epoch_energy / encoded.len() as f64;
        energy_history.push(avg);

        if epoch % 10 == 0 || epoch == num_epochs - 1 {
            println!("{:<8} {:<15.6}", epoch, avg);
        }
    }

    let first = energy_history[0];
    let last = *energy_history.last().unwrap();
    let reduction = ((first - last) / first) * 100.0;

    println!();
    println!("Energy reduction: {:.1}% (from {:.4} to {:.4})", reduction, first, last);

    // ---------------------------------------------------------------
    // 5. Take a snapshot and run prediction
    // ---------------------------------------------------------------
    let snapshot = ModelSnapshot::from_network(&network, encoder.vocab(), num_epochs, num_epochs * encoded.len());
    let shared = Arc::new(RwLock::new(snapshot));
    let predictor = Predictor::new(shared, config.inference_steps);

    println!("\n--- Model Stats ---");
    let stats = predictor.model_stats();
    println!("  Epochs trained: {}", stats.epoch);
    println!("  Total samples: {}", stats.total_samples);
    println!("  Layers: {:?}", stats.layer_dims);
    println!("  Parameters: {}", stats.parameter_count);

    // Run prediction on each training sample.
    println!("\n--- Prediction on Training Data ---");
    println!("{:<12} {:<15} {:<15}", "Event", "Energy", "Recon Error");
    println!("{:-<12} {:-<15} {:-<15}", "", "", "");

    for (i, sample) in encoded.iter().enumerate() {
        if let Some(result) = predictor.confidence(sample) {
            let recon_error: f64 = result
                .reconstruction
                .iter()
                .zip(sample.iter())
                .map(|(r, s)| (r - s).powi(2))
                .sum::<f64>()
                .sqrt();
            println!("{:<12} {:<15.6} {:<15.6}", i, result.energy, recon_error);
        }
    }

    println!("\nDone.");
}
