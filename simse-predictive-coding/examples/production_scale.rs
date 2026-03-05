//! Production-scale librarian workload.
//!
//! Simulates a realistic stream of 100 library events with varied topics
//! and vocabulary growth. Shows training throughput, energy trends, and
//! the network resizing itself as new topics/tags appear.
//!
//! Run: `cargo run --example production_scale --release`

use std::time::Instant;

use simse_pcn_engine::config::{Activation, LayerConfig, PcnConfig};
use simse_pcn_engine::encoder::{InputEncoder, LibraryEvent};
use simse_pcn_engine::network::PredictiveCodingNetwork;
use simse_pcn_engine::predictor::Predictor;
use simse_pcn_engine::snapshot::ModelSnapshot;
use std::sync::{Arc, RwLock};

fn main() {
    println!("=== Predictive Coding Network: Production Scale ===\n");

    // ---------------------------------------------------------------
    // 1. Production configuration
    // ---------------------------------------------------------------
    let embedding_dim = 768;
    let max_topics = 500;
    let max_tags = 1000;

    let config = PcnConfig {
        layers: vec![
            LayerConfig { dim: 512, activation: Activation::Relu },
            LayerConfig { dim: 256, activation: Activation::Relu },
            LayerConfig { dim: 64, activation: Activation::Tanh },
        ],
        inference_steps: 20,
        learning_rate: 0.001,
        inference_rate: 0.1,
        temporal_amortization: true,
        max_topics,
        max_tags,
        ..Default::default()
    };

    let mut encoder = InputEncoder::new(embedding_dim, max_topics, max_tags);
    let initial_input_dim = encoder.current_input_dim();

    println!("--- Architecture ---");
    println!("  Embedding dim: {}", embedding_dim);
    println!("  Max topics: {}", max_topics);
    println!("  Max tags: {}", max_tags);
    println!("  Initial input dim: {}", initial_input_dim);
    println!("  Layers: [512, 256, 64]");
    println!("  Temporal amortization: enabled");

    let mut network = PredictiveCodingNetwork::new(initial_input_dim, &config);

    // Count parameters.
    // Input predictor: initial_input_dim * 512 + initial_input_dim
    // Layer 0: 512 * 256 + 512
    // Layer 1: 256 * 64 + 256
    // Layer 2: 64 * 64 + 64 (top: self-loop)
    let param_count = (initial_input_dim * 512 + initial_input_dim)
        + (512 * 256 + 512)
        + (256 * 64 + 256)
        + (64 * 64 + 64);
    println!("  Parameter count: {} ({:.2}M)", param_count, param_count as f64 / 1_000_000.0);
    println!();

    // ---------------------------------------------------------------
    // 2. Generate 100 diverse events (topics/tags appear gradually)
    // ---------------------------------------------------------------
    let num_events = 100;
    let topic_pool: Vec<String> = (0..30).map(|i| format!("topic_{}", i)).collect();
    let tag_pool: Vec<String> = (0..50).map(|i| format!("tag_{}", i)).collect();
    let entry_types = ["fact", "decision", "observation"];
    let actions = ["extraction", "compendium", "reorganization", "optimization"];

    let mut events: Vec<LibraryEvent> = Vec::with_capacity(num_events);
    for i in 0..num_events {
        // Gradually introduce topics (simulate organic growth).
        let topic_idx = i % topic_pool.len().min(5 + i / 10);
        let tag_idx_1 = i % tag_pool.len().min(3 + i / 5);
        let tag_idx_2 = (i + 7) % tag_pool.len().min(3 + i / 5);

        // Embedding: semi-structured pattern based on topic.
        let mut embedding = vec![0.0f32; embedding_dim];
        for j in 0..embedding_dim {
            embedding[j] = ((topic_idx as f32 + j as f32 * 0.01) * 0.1).sin() * 0.5
                + ((i as f32) * 0.001).cos() * 0.1;
        }

        events.push(LibraryEvent {
            embedding,
            topic: topic_pool[topic_idx].clone(),
            tags: vec![tag_pool[tag_idx_1].clone(), tag_pool[tag_idx_2].clone()],
            entry_type: entry_types[i % 3].into(),
            timestamp: (i as f64) * 15.0,
            time_since_last: 15.0,
            session_ordinal: (i as f64) + 1.0,
            action: actions[i % 4].into(),
        });
    }

    // ---------------------------------------------------------------
    // 3. Train with progress tracking
    // ---------------------------------------------------------------
    println!("--- Training ({} events) ---\n", num_events);
    println!("{:<8} {:<15} {:<12} {:<15}", "Event", "Energy", "Input Dim", "Vocab (t/g)");
    println!("{:-<8} {:-<15} {:-<12} {:-<15}", "", "", "", "");

    let train_start = Instant::now();
    let mut resize_count = 0;

    for (i, event) in events.iter().enumerate() {
        // Encode (may grow vocabulary).
        let (encoded, grew) = match encoder.encode(event) {
            Ok(result) => result,
            Err(e) => {
                eprintln!("  Encoding error at event {}: {}", i, e);
                continue;
            }
        };

        // Resize network if vocabulary grew.
        if grew {
            let new_dim = encoder.current_input_dim();
            if new_dim != network.input_dim() {
                network.resize_input(new_dim);
                resize_count += 1;
            }
        }

        // Train.
        let energy = network.train_single_with_steps(
            &encoded,
            config.inference_steps,
            config.temporal_amortization,
        );

        // Print progress every 10 events.
        if i % 10 == 0 || i == num_events - 1 {
            println!(
                "{:<8} {:<15.4} {:<12} {}/{}",
                i,
                energy,
                network.input_dim(),
                encoder.vocab().topic_count(),
                encoder.vocab().tag_count(),
            );
        }
    }

    let train_elapsed = train_start.elapsed();
    let throughput = num_events as f64 / train_elapsed.as_secs_f64();

    // ---------------------------------------------------------------
    // 4. Summary
    // ---------------------------------------------------------------
    println!("\n--- Performance ---");
    println!("  Training time: {:?}", train_elapsed);
    println!("  Throughput: {:.1} samples/sec", throughput);
    println!("  Network resizes: {}", resize_count);
    println!("  Final input dim: {}", network.input_dim());
    println!("  Final vocab: {} topics, {} tags", encoder.vocab().topic_count(), encoder.vocab().tag_count());

    // Take snapshot and run predictions.
    let snapshot = ModelSnapshot::from_network(&network, encoder.vocab(), 1, num_events);
    let shared = Arc::new(RwLock::new(snapshot));
    let predictor = Predictor::new(shared, config.inference_steps);

    let stats = predictor.model_stats();
    println!("\n--- Model Stats ---");
    println!("  Layers: {:?}", stats.layer_dims);
    println!("  Input dim: {}", stats.input_dim);
    println!("  Parameters: {} ({:.2}M)", stats.parameter_count, stats.parameter_count as f64 / 1_000_000.0);
    println!("  Samples trained: {}", stats.total_samples);

    // Prediction latency.
    let test_event = &events[0];
    let (test_encoded, _) = encoder.encode(test_event).unwrap();
    let pred_start = Instant::now();
    let result = predictor.confidence(&test_encoded);
    let pred_elapsed = pred_start.elapsed();

    if let Some(r) = result {
        println!("\n--- Prediction ---");
        println!("  Latency: {:?}", pred_elapsed);
        println!("  Energy: {:.4}", r.energy);
        println!("  Energy breakdown: {:?}",
            r.energy_breakdown.iter().map(|e| format!("{:.2}", e)).collect::<Vec<_>>()
        );
    }

    println!("\nDone.");
}
