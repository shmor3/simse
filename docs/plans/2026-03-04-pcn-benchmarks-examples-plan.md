# PCN Benchmarks & Examples Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Add Criterion benchmarks and 5 runnable examples to `simse-predictive-coding` demonstrating the librarian's predictive coding capabilities.

**Architecture:** Benchmarks live in `benches/pcn_benchmarks.rs` (single file, 4 Criterion groups). Examples live in `examples/` as standalone binaries. Both use the public API from `simse_pcn_engine`.

**Tech Stack:** Rust, Criterion 0.5, simse_pcn_engine (the crate's lib), rand 0.8, tempfile 3

---

### Task 0: Update Cargo.toml with Criterion and bench harness

**Files:**
- Modify: `simse-predictive-coding/Cargo.toml`

**Step 1: Add criterion to dev-dependencies and bench config**

Add `criterion` to `[dev-dependencies]` and a `[[bench]]` section:

```toml
[dev-dependencies]
tempfile = "3"
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "pcn_benchmarks"
harness = false
```

**Step 2: Create empty benches/ and examples/ directories**

```bash
mkdir -p simse-predictive-coding/benches
mkdir -p simse-predictive-coding/examples
```

**Step 3: Commit**

```bash
git add simse-predictive-coding/Cargo.toml
git commit -m "chore(simse-predictive-coding): add criterion dev-dep and bench harness"
```

---

### Task 1: Create Criterion benchmarks

**Files:**
- Create: `simse-predictive-coding/benches/pcn_benchmarks.rs`

**Step 1: Write the benchmark file**

```rust
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

use simse_pcn_engine::config::{Activation, LayerConfig, PcnConfig};
use simse_pcn_engine::encoder::{InputEncoder, LibraryEvent};
use simse_pcn_engine::network::PredictiveCodingNetwork;
use simse_pcn_engine::persistence::{load_snapshot, save_snapshot};
use simse_pcn_engine::snapshot::ModelSnapshot;
use simse_pcn_engine::vocabulary::VocabularyManager;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Small model: [16, 8] layers, input_dim=24 (embedding=4, topics=5, tags=5).
fn small_config() -> PcnConfig {
    PcnConfig {
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
    }
}

/// Production model: [512, 256, 64] layers.
/// Input dim = 768 + 500 + 1000 + 10 = 2278.
fn production_config() -> PcnConfig {
    PcnConfig {
        layers: vec![
            LayerConfig { dim: 512, activation: Activation::Relu },
            LayerConfig { dim: 256, activation: Activation::Relu },
            LayerConfig { dim: 64, activation: Activation::Tanh },
        ],
        inference_steps: 20,
        learning_rate: 0.005,
        inference_rate: 0.1,
        temporal_amortization: false,
        max_topics: 500,
        max_tags: 1000,
        ..Default::default()
    }
}

fn small_input_dim() -> usize {
    // embedding(4) + topics(5) + tags(5) + fixed(10) = 24
    24
}

fn production_input_dim() -> usize {
    // embedding(768) + topics(500) + tags(1000) + fixed(10) = 2278
    2278
}

fn random_input(dim: usize) -> Vec<f64> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

fn make_event(embedding_dim: usize) -> LibraryEvent {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    LibraryEvent {
        embedding: (0..embedding_dim).map(|_| rng.gen_range(-1.0f32..1.0)).collect(),
        topic: "rust".to_string(),
        tags: vec!["core".to_string()],
        entry_type: "fact".to_string(),
        timestamp: 100.0,
        time_since_last: 5.0,
        session_ordinal: 1.0,
        action: "extraction".to_string(),
    }
}

fn trained_network(input_dim: usize, config: &PcnConfig) -> PredictiveCodingNetwork {
    let mut net = PredictiveCodingNetwork::new(input_dim, config);
    let input = random_input(input_dim);
    for _ in 0..10 {
        net.train_single(&input);
    }
    net
}

fn make_snapshot(input_dim: usize, config: &PcnConfig) -> ModelSnapshot {
    let net = trained_network(input_dim, config);
    let vocab = VocabularyManager::new(config.max_topics, config.max_tags);
    ModelSnapshot::from_network(&net, &vocab, 10, 10)
}

// ---------------------------------------------------------------------------
// Benchmark group 1: Inference
// ---------------------------------------------------------------------------

fn bench_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("inference");

    // Small model
    for steps in [10, 20, 50] {
        let dim = small_input_dim();
        let config = small_config();
        let mut net = PredictiveCodingNetwork::new(dim, &config);
        let input = random_input(dim);

        group.bench_function(format!("small/{}", steps), |b| {
            b.iter(|| net.infer(&input, steps))
        });
    }

    // Production model
    for steps in [10, 20, 50] {
        let dim = production_input_dim();
        let config = production_config();
        let mut net = PredictiveCodingNetwork::new(dim, &config);
        let input = random_input(dim);

        group.bench_function(format!("production/{}", steps), |b| {
            b.iter(|| net.infer(&input, steps))
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark group 2: Training
// ---------------------------------------------------------------------------

fn bench_training(c: &mut Criterion) {
    let mut group = c.benchmark_group("training");

    // Small model
    {
        let dim = small_input_dim();
        let config = small_config();
        let mut net = PredictiveCodingNetwork::new(dim, &config);
        let input = random_input(dim);

        group.bench_function("small", |b| {
            b.iter(|| net.train_single_with_steps(&input, 20, false))
        });

        group.bench_function("small/amortized", |b| {
            b.iter(|| net.train_single_with_steps(&input, 20, true))
        });
    }

    // Production model
    {
        let dim = production_input_dim();
        let config = production_config();
        let mut net = PredictiveCodingNetwork::new(dim, &config);
        let input = random_input(dim);

        group.bench_function("production", |b| {
            b.iter(|| net.train_single_with_steps(&input, 20, false))
        });

        group.bench_function("production/amortized", |b| {
            b.iter(|| net.train_single_with_steps(&input, 20, true))
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark group 3: Encoding
// ---------------------------------------------------------------------------

fn bench_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("encoding");
    let embedding_dim = 768;

    // Empty vocab
    {
        let mut encoder = InputEncoder::new(embedding_dim, 500, 1000);
        let event = make_event(embedding_dim);

        group.bench_function("empty_vocab", |b| {
            b.iter_batched(
                || {
                    // Fresh encoder each iteration so vocab stays empty.
                    (InputEncoder::new(embedding_dim, 500, 1000), event.clone())
                },
                |(mut enc, ev)| enc.encode(&ev),
                BatchSize::SmallInput,
            )
        });
    }

    // Half vocab (250 topics, 500 tags pre-registered)
    {
        let event = make_event(embedding_dim);

        group.bench_function("half_vocab", |b| {
            b.iter_batched(
                || {
                    let mut enc = InputEncoder::new(embedding_dim, 500, 1000);
                    for i in 0..250 {
                        enc.vocab_mut()
                            .register_topic(&format!("topic_{}", i))
                            .unwrap();
                    }
                    for i in 0..500 {
                        enc.vocab_mut()
                            .register_tag(&format!("tag_{}", i))
                            .unwrap();
                    }
                    (enc, event.clone())
                },
                |(mut enc, ev)| enc.encode(&ev),
                BatchSize::SmallInput,
            )
        });
    }

    // Full vocab (500 topics, 1000 tags pre-registered)
    {
        let event = make_event(embedding_dim);

        group.bench_function("full_vocab", |b| {
            b.iter_batched(
                || {
                    let mut enc = InputEncoder::new(embedding_dim, 500, 1000);
                    for i in 0..500 {
                        enc.vocab_mut()
                            .register_topic(&format!("topic_{}", i))
                            .unwrap();
                    }
                    for i in 0..1000 {
                        enc.vocab_mut()
                            .register_tag(&format!("tag_{}", i))
                            .unwrap();
                    }
                    (enc, event.clone())
                },
                |(mut enc, ev)| enc.encode(&ev),
                BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark group 4: Snapshot operations
// ---------------------------------------------------------------------------

fn bench_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("snapshot");

    // predict/cold (small) — clone snapshot each iteration so cache is empty
    {
        let snapshot = make_snapshot(small_input_dim(), &small_config());
        let input = random_input(small_input_dim());

        group.bench_function("predict/cold/small", |b| {
            b.iter_batched(
                || (snapshot.clone(), input.clone()),
                |(s, inp)| s.predict(&inp, 10),
                BatchSize::SmallInput,
            )
        });
    }

    // predict/warm (small) — warm the cache, then measure
    {
        let snapshot = make_snapshot(small_input_dim(), &small_config());
        let input = random_input(small_input_dim());
        // Warm the cache.
        snapshot.predict(&input, 10);

        group.bench_function("predict/warm/small", |b| {
            b.iter(|| snapshot.predict(&input, 10))
        });
    }

    // predict/cold (production)
    {
        let snapshot = make_snapshot(production_input_dim(), &production_config());
        let input = random_input(production_input_dim());

        group.bench_function("predict/cold/production", |b| {
            b.iter_batched(
                || (snapshot.clone(), input.clone()),
                |(s, inp)| s.predict(&inp, 10),
                BatchSize::SmallInput,
            )
        });
    }

    // predict/warm (production)
    {
        let snapshot = make_snapshot(production_input_dim(), &production_config());
        let input = random_input(production_input_dim());
        snapshot.predict(&input, 10);

        group.bench_function("predict/warm/production", |b| {
            b.iter(|| snapshot.predict(&input, 10))
        });
    }

    // from_network (small)
    {
        let config = small_config();
        let net = trained_network(small_input_dim(), &config);
        let vocab = VocabularyManager::new(config.max_topics, config.max_tags);

        group.bench_function("from_network/small", |b| {
            b.iter(|| ModelSnapshot::from_network(&net, &vocab, 10, 10))
        });
    }

    // from_network (production)
    {
        let config = production_config();
        let net = trained_network(production_input_dim(), &config);
        let vocab = VocabularyManager::new(config.max_topics, config.max_tags);

        group.bench_function("from_network/production", |b| {
            b.iter(|| ModelSnapshot::from_network(&net, &vocab, 10, 10))
        });
    }

    // save/load JSON (small)
    {
        let snapshot = make_snapshot(small_input_dim(), &small_config());
        let dir = tempfile::tempdir().unwrap();

        let json_path = dir.path().join("bench.json");
        let json_str = json_path.to_str().unwrap().to_string();

        // Save once for load benchmark.
        save_snapshot(&snapshot, &json_str, false).unwrap();

        group.bench_function("save_json/small", |b| {
            b.iter(|| save_snapshot(&snapshot, &json_str, false))
        });

        group.bench_function("load_json/small", |b| {
            b.iter(|| load_snapshot(&json_str, false))
        });
    }

    // save/load gzip (small)
    {
        let snapshot = make_snapshot(small_input_dim(), &small_config());
        let dir = tempfile::tempdir().unwrap();

        let gz_path = dir.path().join("bench.json.gz");
        let gz_str = gz_path.to_str().unwrap().to_string();

        save_snapshot(&snapshot, &gz_str, true).unwrap();

        group.bench_function("save_gzip/small", |b| {
            b.iter(|| save_snapshot(&snapshot, &gz_str, true))
        });

        group.bench_function("load_gzip/small", |b| {
            b.iter(|| load_snapshot(&gz_str, true))
        });
    }

    // save/load gzip (production)
    {
        let snapshot = make_snapshot(production_input_dim(), &production_config());
        let dir = tempfile::tempdir().unwrap();

        let gz_path = dir.path().join("bench.json.gz");
        let gz_str = gz_path.to_str().unwrap().to_string();

        save_snapshot(&snapshot, &gz_str, true).unwrap();

        group.bench_function("save_gzip/production", |b| {
            b.iter(|| save_snapshot(&snapshot, &gz_str, true))
        });

        group.bench_function("load_gzip/production", |b| {
            b.iter(|| load_snapshot(&gz_str, true))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_inference,
    bench_training,
    bench_encoding,
    bench_snapshot
);
criterion_main!(benches);
```

**Step 2: Verify it compiles**

```bash
cd simse-predictive-coding && cargo bench --no-run
```

Expected: Compiles without errors.

**Step 3: Run benchmarks (quick sanity check)**

```bash
cd simse-predictive-coding && cargo bench -- --quick
```

Expected: All 4 groups produce output without panics.

**Step 4: Commit**

```bash
git add simse-predictive-coding/benches/pcn_benchmarks.rs
git commit -m "bench(simse-predictive-coding): add Criterion benchmarks (4 groups)"
```

---

### Task 2: Create training_basics example

**Files:**
- Create: `simse-predictive-coding/examples/training_basics.rs`

**Step 1: Write the example**

```rust
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
```

**Step 2: Verify it compiles and runs**

```bash
cd simse-predictive-coding && cargo run --example training_basics
```

Expected: Compiles without errors. Output shows epoch/energy table with decreasing energy, model stats, and prediction results.

**Step 3: Commit**

```bash
git add simse-predictive-coding/examples/training_basics.rs
git commit -m "example(simse-predictive-coding): add training_basics example"
```

---

### Task 3: Create anomaly_detection example

**Files:**
- Create: `simse-predictive-coding/examples/anomaly_detection.rs`

**Step 1: Write the example**

```rust
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
```

**Step 2: Verify it compiles and runs**

```bash
cd simse-predictive-coding && cargo run --example anomaly_detection
```

Expected: Compiles. Output shows anomalous events ranked higher (more energy) than normal events with a clear separation ratio.

**Step 3: Commit**

```bash
git add simse-predictive-coding/examples/anomaly_detection.rs
git commit -m "example(simse-predictive-coding): add anomaly_detection example"
```

---

### Task 4: Create temporal_amortization example

**Files:**
- Create: `simse-predictive-coding/examples/temporal_amortization.rs`

**Step 1: Write the example**

```rust
//! Temporal amortization: warm-start inference vs fresh randomization.
//!
//! Demonstrates that reusing latent states from the previous inference
//! (temporal amortization) reaches lower energy in fewer steps than
//! re-randomizing latents each time. This is especially effective for
//! temporally correlated inputs (sequential conversation turns).
//!
//! Run: `cargo run --example temporal_amortization`

use std::time::Instant;

use simse_pcn_engine::config::{Activation, LayerConfig, PcnConfig};
use simse_pcn_engine::network::PredictiveCodingNetwork;

fn main() {
    println!("=== Predictive Coding Network: Temporal Amortization ===\n");

    // ---------------------------------------------------------------
    // 1. Production-scale model
    // ---------------------------------------------------------------
    let input_dim = 2278; // 768 embedding + 500 topics + 1000 tags + 10 fixed
    let config = PcnConfig {
        layers: vec![
            LayerConfig { dim: 512, activation: Activation::Relu },
            LayerConfig { dim: 256, activation: Activation::Relu },
            LayerConfig { dim: 64, activation: Activation::Tanh },
        ],
        inference_steps: 20,
        learning_rate: 0.001,
        inference_rate: 0.1,
        temporal_amortization: false,
        ..Default::default()
    };

    println!("Model: [512, 256, 64] layers, {} input dims", input_dim);
    println!("Training on 20 correlated samples...\n");

    // ---------------------------------------------------------------
    // 2. Generate temporally correlated inputs
    //    Each input drifts slightly from the previous one (simulating
    //    a conversation that evolves topic over time).
    // ---------------------------------------------------------------
    let num_samples = 20;
    let mut inputs: Vec<Vec<f64>> = Vec::with_capacity(num_samples);
    let mut base = vec![0.0; input_dim];

    // Initialize with a pattern.
    for i in 0..input_dim {
        base[i] = ((i as f64) * 0.01).sin() * 0.5;
    }

    for s in 0..num_samples {
        let mut input = base.clone();
        // Small drift: each sample shifts slightly.
        for i in 0..input_dim {
            input[i] += ((s as f64) * 0.05 + (i as f64) * 0.001).cos() * 0.1;
        }
        inputs.push(input);
    }

    // ---------------------------------------------------------------
    // 3. Train the network (same for both comparisons)
    // ---------------------------------------------------------------
    let mut net_fresh = PredictiveCodingNetwork::new(input_dim, &config);
    for sample in &inputs {
        net_fresh.train_single_with_steps(sample, 20, false);
    }

    // Clone weights for amortized network.
    let mut net_amortized = net_fresh.clone();

    // ---------------------------------------------------------------
    // 4. Compare: fresh vs amortized at different step counts
    // ---------------------------------------------------------------
    println!("{:<8} {:<18} {:<18} {:<12}", "Steps", "Energy (Fresh)", "Energy (Amortized)", "Improvement");
    println!("{:-<8} {:-<18} {:-<18} {:-<12}", "", "", "", "");

    for steps in [5, 10, 15, 20, 30, 50] {
        // Fresh inference: randomize latents each time.
        let start = Instant::now();
        let mut total_fresh = 0.0;
        for sample in &inputs {
            total_fresh += net_fresh.infer(sample, steps);
        }
        let avg_fresh = total_fresh / num_samples as f64;
        let fresh_time = start.elapsed();

        // Amortized inference: preserve latents between samples.
        let start = Instant::now();
        let mut total_amortized = 0.0;
        for (i, sample) in inputs.iter().enumerate() {
            if i == 0 {
                // First sample: must randomize (no prior state).
                total_amortized += net_amortized.infer(sample, steps);
            } else {
                total_amortized += net_amortized.infer_amortized(sample, steps);
            }
        }
        let avg_amortized = total_amortized / num_samples as f64;
        let amortized_time = start.elapsed();

        let improvement = if avg_fresh > 0.0 {
            ((avg_fresh - avg_amortized) / avg_fresh) * 100.0
        } else {
            0.0
        };

        println!(
            "{:<8} {:<18.4} {:<18.4} {:<12}",
            steps,
            avg_fresh,
            avg_amortized,
            format!("{:+.1}%", improvement),
        );

        // Also show timing for the last row.
        if steps == 50 {
            println!();
            println!("Timing at {} steps ({} samples):", steps, num_samples);
            println!("  Fresh:     {:?}", fresh_time);
            println!("  Amortized: {:?}", amortized_time);
        }
    }

    println!("\nKey insight: Amortized inference reaches lower energy because");
    println!("latent states from the previous (similar) input provide a");
    println!("better starting point than random initialization.\n");
    println!("Done.");
}
```

**Step 2: Verify it compiles and runs**

```bash
cd simse-predictive-coding && cargo run --example temporal_amortization --release
```

Expected: Compiles. Output shows side-by-side energy comparison. Amortized inference should show lower energy at all step counts. Use `--release` because this is production-scale.

**Step 3: Commit**

```bash
git add simse-predictive-coding/examples/temporal_amortization.rs
git commit -m "example(simse-predictive-coding): add temporal_amortization example"
```

---

### Task 5: Create model_persistence example

**Files:**
- Create: `simse-predictive-coding/examples/model_persistence.rs`

**Step 1: Write the example**

```rust
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

    // Training data.
    let inputs: Vec<Vec<f64>> = vec![
        vec![0.5, 0.3, 0.7, 0.2, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 100.0, 5.0, 1.0, 1.0, 0.0, 0.0, 0.0],
        vec![0.8, 0.1, 0.4, 0.6, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 110.0, 10.0, 2.0, 0.0, 1.0, 0.0, 0.0],
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
```

**Step 2: Verify it compiles and runs**

```bash
cd simse-predictive-coding && cargo run --example model_persistence
```

Expected: Compiles. Output shows file sizes, energy comparisons, and PASS for all round-trip checks.

**Step 3: Commit**

```bash
git add simse-predictive-coding/examples/model_persistence.rs
git commit -m "example(simse-predictive-coding): add model_persistence example"
```

---

### Task 6: Create production_scale example

**Files:**
- Create: `simse-predictive-coding/examples/production_scale.rs`

**Step 1: Write the example**

```rust
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
```

**Step 2: Verify it compiles and runs**

```bash
cd simse-predictive-coding && cargo run --example production_scale --release
```

Expected: Compiles. Output shows architecture, training progress with vocabulary growth, throughput stats, and prediction latency. Use `--release` for representative performance numbers.

**Step 3: Commit**

```bash
git add simse-predictive-coding/examples/production_scale.rs
git commit -m "example(simse-predictive-coding): add production_scale example"
```

---

### Task 7: Final verification

**Step 1: Run all existing tests to ensure nothing is broken**

```bash
cd simse-predictive-coding && cargo test
```

Expected: All existing tests pass.

**Step 2: Run all benchmarks (full run)**

```bash
cd simse-predictive-coding && cargo bench
```

Expected: All 4 benchmark groups produce results without panics.

**Step 3: Run all examples**

```bash
cd simse-predictive-coding && cargo run --example training_basics
cd simse-predictive-coding && cargo run --example anomaly_detection
cd simse-predictive-coding && cargo run --example temporal_amortization --release
cd simse-predictive-coding && cargo run --example model_persistence
cd simse-predictive-coding && cargo run --example production_scale --release
```

Expected: All 5 examples produce clean output without panics.
