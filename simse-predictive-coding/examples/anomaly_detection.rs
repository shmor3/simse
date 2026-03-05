//! Anomaly detection using predictive coding energy.
//!
//! Trains on 20 "normal" events, then scores both normal and anomalous
//! inputs. Anomalous inputs have higher prediction energy because the
//! model has never learned to predict them.
//!
//! Run: `cargo run --example anomaly_detection`

use simse_pcn_engine::config::{Activation, LayerConfig, PcnConfig};
use simse_pcn_engine::encoder::{InputEncoder, LibraryEvent};
use simse_pcn_engine::network::PredictiveCodingNetwork;
use simse_pcn_engine::predictor::Predictor;
use simse_pcn_engine::snapshot::ModelSnapshot;
use std::sync::{Arc, RwLock};

fn normal_event(i: usize) -> LibraryEvent {
    // Normal events cluster around a consistent embedding pattern
    // with small variations (simulating typical library usage).
    let base = [0.5, 0.3, 0.7, 0.2];
    let jitter = (i as f32) * 0.02;
    LibraryEvent {
        embedding: vec![
            base[0] + jitter,
            base[1] - jitter * 0.5,
            base[2] + jitter * 0.3,
            base[3] - jitter * 0.1,
        ],
        topic: ["rust", "python"][i % 2].into(),
        tags: vec!["core".into()],
        entry_type: "fact".into(),
        timestamp: (i as f64) * 10.0,
        time_since_last: 10.0,
        session_ordinal: (i as f64) + 1.0,
        action: "extraction".into(),
    }
}

fn anomalous_event(i: usize) -> LibraryEvent {
    // Anomalous events have very different embeddings, unusual topics,
    // and different action patterns.
    LibraryEvent {
        embedding: vec![
            -0.9 + (i as f32) * 0.1,
            0.95,
            -0.8,
            0.99,
        ],
        topic: "quantum_computing".into(),
        tags: vec!["anomaly".into(), "unusual".into()],
        entry_type: "observation".into(),
        timestamp: 500.0 + (i as f64) * 100.0,
        time_since_last: 200.0,
        session_ordinal: 50.0,
        action: "optimization".into(),
    }
}

fn main() {
    println!("=== Predictive Coding Network: Anomaly Detection ===\n");

    // ---------------------------------------------------------------
    // 1. Setup
    // ---------------------------------------------------------------
    let embedding_dim = 4;
    let max_topics = 10;
    let max_tags = 10;

    let config = PcnConfig {
        layers: vec![
            LayerConfig { dim: 32, activation: Activation::Tanh },
            LayerConfig { dim: 16, activation: Activation::Tanh },
        ],
        inference_steps: 30,
        learning_rate: 0.001,
        inference_rate: 0.1,
        temporal_amortization: false,
        max_topics,
        max_tags,
        ..Default::default()
    };

    let mut encoder = InputEncoder::new(embedding_dim, max_topics, max_tags);

    // Pre-register vocabulary for both normal and anomalous events.
    for t in ["rust", "python", "quantum_computing"] {
        encoder.vocab_mut().register_topic(t).unwrap();
    }
    for t in ["core", "anomaly", "unusual"] {
        encoder.vocab_mut().register_tag(t).unwrap();
    }

    let input_dim = encoder.current_input_dim();
    let mut network = PredictiveCodingNetwork::new(input_dim, &config);

    // ---------------------------------------------------------------
    // 2. Generate and encode training data (normal events only)
    // ---------------------------------------------------------------
    let normal_events: Vec<LibraryEvent> = (0..20).map(normal_event).collect();
    let normal_encoded: Vec<Vec<f64>> = normal_events
        .iter()
        .map(|ev| encoder.encode(ev).unwrap().0)
        .collect();

    println!("Training on {} normal events...", normal_encoded.len());

    // Train for 80 epochs on normal data only.
    for epoch in 0..80 {
        for sample in &normal_encoded {
            network.train_single_with_steps(sample, config.inference_steps, false);
        }
        if epoch % 20 == 0 {
            // Spot-check energy.
            let e = network.infer(&normal_encoded[0], config.inference_steps);
            println!("  Epoch {}: sample energy = {:.4}", epoch, e);
        }
    }
    println!("Training complete.\n");

    // ---------------------------------------------------------------
    // 3. Score all inputs (normal + anomalous)
    // ---------------------------------------------------------------
    let anomalous_events: Vec<LibraryEvent> = (0..5).map(anomalous_event).collect();
    let anomalous_encoded: Vec<Vec<f64>> = anomalous_events
        .iter()
        .map(|ev| encoder.encode(ev).unwrap().0)
        .collect();

    // Build snapshot and predictor.
    let snapshot = ModelSnapshot::from_network(&network, encoder.vocab(), 80, 80 * 20);
    let shared = Arc::new(RwLock::new(snapshot));
    let predictor = Predictor::new(shared, config.inference_steps);

    // Combine all inputs for anomaly ranking.
    let mut all_inputs: Vec<(Vec<f64>, &str)> = Vec::new();
    for enc in &normal_encoded {
        all_inputs.push((enc.clone(), "normal"));
    }
    for enc in &anomalous_encoded {
        all_inputs.push((enc.clone(), "ANOMALY"));
    }

    let input_vecs: Vec<Vec<f64>> = all_inputs.iter().map(|(v, _)| v.clone()).collect();
    let anomalies = predictor.anomalies(&input_vecs, 25);

    // ---------------------------------------------------------------
    // 4. Display results
    // ---------------------------------------------------------------
    println!("--- Anomaly Ranking (by prediction energy, descending) ---\n");
    println!("{:<6} {:<10} {:<15}", "Rank", "Type", "Energy");
    println!("{:-<6} {:-<10} {:-<15}", "", "", "");

    let mut normal_energies = Vec::new();
    let mut anomaly_energies = Vec::new();

    for (rank, (idx, energy)) in anomalies.iter().enumerate() {
        let label = all_inputs[*idx].1;
        println!("{:<6} {:<10} {:<15.4}", rank + 1, label, energy);

        match label {
            "normal" => normal_energies.push(*energy),
            _ => anomaly_energies.push(*energy),
        }
    }

    println!();

    let avg_normal = if normal_energies.is_empty() {
        0.0
    } else {
        normal_energies.iter().sum::<f64>() / normal_energies.len() as f64
    };
    let avg_anomaly = if anomaly_energies.is_empty() {
        0.0
    } else {
        anomaly_energies.iter().sum::<f64>() / anomaly_energies.len() as f64
    };

    println!("--- Summary ---");
    println!("  Avg normal energy:  {:.4}", avg_normal);
    println!("  Avg anomaly energy: {:.4}", avg_anomaly);
    println!(
        "  Separation ratio:   {:.2}x",
        if avg_normal > 0.0 { avg_anomaly / avg_normal } else { 0.0 }
    );

    println!("\nDone.");
}
