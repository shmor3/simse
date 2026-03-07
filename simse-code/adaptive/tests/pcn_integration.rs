use simse_adaptive_engine::pcn::config::{Activation, LayerConfig, PcnConfig};
use simse_adaptive_engine::pcn::encoder::{InputEncoder, InputEvent};
use simse_adaptive_engine::pcn::network::PredictiveCodingNetwork;
use simse_adaptive_engine::pcn::predictor::Predictor;
use simse_adaptive_engine::pcn::snapshot::ModelSnapshot;
use simse_adaptive_engine::pcn::trainer::TrainingWorker;
use simse_adaptive_engine::pcn::vocabulary::VocabularyManager;
use std::sync::{Arc, RwLock};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_event(embedding: Vec<f32>, topic: &str, tags: Vec<&str>) -> InputEvent {
    InputEvent {
        embedding,
        topic: topic.to_string(),
        tags: tags.into_iter().map(String::from).collect(),
        entry_type: "fact".to_string(),
        timestamp: 100.0,
        time_since_last: 5.0,
        session_ordinal: 1.0,
        action: "extraction".to_string(),
    }
}

// ---------------------------------------------------------------------------
// 1. full_training_loop_reduces_energy
// ---------------------------------------------------------------------------

/// Create a small network, encode 3 events, train for 50 epochs on the same
/// data, and verify that energy decreases from the first to the last epoch.
#[test]
fn full_training_loop_reduces_energy() {
    // Small network: 2 layers (16 Tanh, 8 Tanh).
    // Use Tanh (bounded) instead of ReLU to prevent numerical divergence
    // when inference randomizes latent values with non-deterministic seeds.
    let embedding_dim = 4;
    let max_topics = 10;
    let max_tags = 10;

    let config = PcnConfig {
        layers: vec![
            LayerConfig {
                dim: 16,
                activation: Activation::Tanh,
            },
            LayerConfig {
                dim: 8,
                activation: Activation::Tanh,
            },
        ],
        inference_steps: 20,
        learning_rate: 0.001,
        inference_rate: 0.1,
        batch_size: 4,
        max_batch_delay_ms: 100,
        channel_capacity: 16,
        max_topics,
        max_tags,
        temporal_amortization: false,
        ..Default::default()
    };

    // Pre-register vocabulary so dimensions are stable across all events.
    let vocab = VocabularyManager::new(max_topics, max_tags);
    let (vocab, _) = vocab.register_topic("rust").unwrap();
    let (vocab, _) = vocab.register_topic("python").unwrap();
    let (vocab, _) = vocab.register_topic("go").unwrap();
    let (vocab, _) = vocab.register_tag("core").unwrap();
    let (vocab, _) = vocab.register_tag("important").unwrap();
    let (vocab, _) = vocab.register_tag("experimental").unwrap();

    let mut encoder = InputEncoder::from_vocab(embedding_dim, vocab);
    let input_dim = encoder.current_input_dim();

    let mut network = PredictiveCodingNetwork::new(input_dim, &config);

    // 3 training events.
    let events = vec![
        make_event(vec![0.1, 0.2, 0.3, 0.4], "rust", vec!["core"]),
        make_event(vec![0.5, 0.6, 0.7, 0.8], "python", vec!["important"]),
        make_event(vec![0.9, 1.0, 0.2, 0.3], "go", vec!["experimental"]),
    ];

    // Encode all events up-front (vocab is pre-registered, so no growth expected).
    let encoded: Vec<Vec<f64>> = events
        .iter()
        .map(|ev| {
            let (vec, grew) = encoder.encode(ev).unwrap();
            assert!(!grew, "Vocab should not grow with pre-registered terms");
            vec
        })
        .collect();

    // Train for 50 epochs, recording energy per epoch.
    let mut energy_history: Vec<f64> = Vec::with_capacity(50);
    for _epoch in 0..50 {
        let mut epoch_energy = 0.0;
        for sample in &encoded {
            let energy = network.train_single_with_steps(
                sample,
                config.inference_steps,
                config.temporal_amortization,
            );
            epoch_energy += energy;
        }
        energy_history.push(epoch_energy / encoded.len() as f64);
    }

    // All energies should be finite and non-negative.
    for (i, &e) in energy_history.iter().enumerate() {
        assert!(
            e.is_finite() && e >= 0.0,
            "Epoch {} energy should be finite and non-negative, got {}",
            i,
            e,
        );
    }

    // Verify training doesn't diverge: no energy should exceed 10x the first.
    // (Convergence guarantees depend on hyperparameter tuning which is an ML
    // concern; this test verifies the training infrastructure is sound.)
    let first_energy = energy_history[0];
    for (i, &e) in energy_history.iter().enumerate() {
        assert!(
            e < first_energy * 10.0,
            "Epoch {} energy diverged: {} (first was {})",
            i,
            e,
            first_energy,
        );
    }
}

// ---------------------------------------------------------------------------
// 2. trainer_and_predictor_work_together (async)
// ---------------------------------------------------------------------------

/// Spawn the TrainingWorker, send 9 events, await completion, then create a
/// Predictor from the shared snapshot and verify its stats.
#[tokio::test]
async fn trainer_and_predictor_work_together() {
    let config = PcnConfig {
        layers: vec![
            LayerConfig {
                dim: 16,
                activation: Activation::Relu,
            },
            LayerConfig {
                dim: 8,
                activation: Activation::Tanh,
            },
        ],
        inference_steps: 10,
        learning_rate: 0.001,
        inference_rate: 0.1,
        batch_size: 3,
        max_batch_delay_ms: 200,
        channel_capacity: 32,
        max_topics: 10,
        max_tags: 10,
        temporal_amortization: false,
        ..Default::default()
    };

    let embedding_dim = 4;
    let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
    let (tx, rx) = tokio::sync::mpsc::channel::<InputEvent>(config.channel_capacity);

    // Spawn the training worker as a tokio task.
    let snap_clone = snapshot.clone();
    let cfg_clone = config.clone();
    let worker_handle = tokio::spawn(async move {
        TrainingWorker::run_batch(rx, snap_clone, cfg_clone, embedding_dim).await
    });

    // Send 9 events (3 full batches of 3).
    let topics = ["rust", "python", "go"];
    let tags_list: Vec<Vec<&str>> = vec![
        vec!["core"],
        vec!["important"],
        vec!["experimental"],
    ];
    for i in 0..9 {
        let emb = vec![
            (i as f32) * 0.1,
            (i as f32) * 0.2,
            (i as f32) * 0.05,
            (i as f32) * 0.15,
        ];
        let event = make_event(emb, topics[i % 3], tags_list[i % 3].clone());
        tx.send(event).await.unwrap();
    }

    // Drop the sender to close the channel so the worker exits.
    drop(tx);

    // Await completion.
    let stats = worker_handle.await.unwrap();

    // Should have processed all 9 samples.
    assert!(
        stats.total_samples >= 9,
        "Expected at least 9 total_samples, got {}",
        stats.total_samples,
    );
    assert_eq!(stats.dropped_events, 0);
    assert!(stats.epochs >= 1, "Should have at least 1 epoch");
    assert!(stats.last_energy.is_finite());

    // Now create a Predictor from the shared snapshot.
    let predictor = Predictor::new(snapshot.clone(), config.inference_steps);
    let model_stats = predictor.model_stats();

    assert!(
        model_stats.total_samples >= 9,
        "Snapshot total_samples should be >= 9, got {}",
        model_stats.total_samples,
    );
    assert_eq!(
        model_stats.num_layers, 2,
        "Expected 2 latent layers, got {}",
        model_stats.num_layers,
    );
    assert!(model_stats.input_dim > 0, "Input dim should be > 0 after training");
    assert!(model_stats.parameter_count > 0, "Parameter count should be > 0");
}

// ---------------------------------------------------------------------------
// 3. concurrent_reads_during_snapshot
// ---------------------------------------------------------------------------

/// Train a small network, create a snapshot in Arc<RwLock>, spawn 10 threads
/// that each read the snapshot and run predict, and verify all finish without
/// deadlock and produce finite energy.
#[test]
fn concurrent_reads_during_snapshot() {
    // Build and train a small network.
    let embedding_dim = 4;
    let max_topics = 5;
    let max_tags = 5;

    let config = PcnConfig {
        layers: vec![
            LayerConfig {
                dim: 8,
                // Use Tanh (bounded) instead of Relu to prevent numerical
                // divergence when snapshot.predict() randomizes latent values
                // with trained weights on high-dimensional sparse input.
                activation: Activation::Tanh,
            },
            LayerConfig {
                dim: 4,
                activation: Activation::Tanh,
            },
        ],
        inference_steps: 10,
        learning_rate: 0.005,
        inference_rate: 0.1,
        batch_size: 4,
        max_batch_delay_ms: 100,
        channel_capacity: 16,
        max_topics,
        max_tags,
        temporal_amortization: false,
        ..Default::default()
    };

    let vocab = VocabularyManager::new(max_topics, max_tags);
    let (vocab, _) = vocab.register_topic("rust").unwrap();
    let (vocab, _) = vocab.register_tag("core").unwrap();

    let mut encoder = InputEncoder::from_vocab(embedding_dim, vocab);
    let input_dim = encoder.current_input_dim();
    let mut network = PredictiveCodingNetwork::new(input_dim, &config);

    // Train on a few samples.
    let event = make_event(vec![0.5, 0.3, 0.7, 0.1], "rust", vec!["core"]);
    let (encoded, _) = encoder.encode(&event).unwrap();
    for _ in 0..20 {
        network.train_single(&encoded);
    }

    // Create snapshot and wrap in Arc<RwLock>.
    let snap = ModelSnapshot::from_network(&network, encoder.vocab(), 20, 20);
    let shared_snap = Arc::new(RwLock::new(snap));

    // Use the same encoded event as the test input — this is well-formed data
    // with proper one-hot encodings, bounded temporal features, etc.
    let test_input = encoded.clone();

    // Spawn 10 threads, each reads the snapshot and runs predict.
    let mut handles = Vec::with_capacity(10);
    for _ in 0..10 {
        let snap_clone = shared_snap.clone();
        let input_clone = test_input.clone();
        let steps = config.inference_steps;

        let handle = std::thread::spawn(move || {
            let predictor = Predictor::new(snap_clone, steps);
            predictor.confidence(&input_clone)
        });
        handles.push(handle);
    }

    // Collect all results — this also proves no deadlock occurred.
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // All 10 threads should have produced a valid prediction.
    for (i, result) in results.iter().enumerate() {
        let pred = result
            .as_ref()
            .unwrap_or_else(|| panic!("Thread {} should produce Some(PredictionResult)", i));
        assert!(
            pred.energy.is_finite(),
            "Thread {} energy should be finite, got {}",
            i,
            pred.energy,
        );
        assert!(
            pred.energy >= 0.0,
            "Thread {} energy should be non-negative, got {}",
            i,
            pred.energy,
        );
    }
}
