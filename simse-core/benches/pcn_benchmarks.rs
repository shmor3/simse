use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

use simse_core::adaptive::pcn_config::{Activation, LayerConfig, PcnConfig};
use simse_core::adaptive::encoder::{InputEncoder, LibraryEvent};
use simse_core::adaptive::network::PredictiveCodingNetwork;
use simse_core::adaptive::persistence::{load_snapshot, save_snapshot};
use simse_core::adaptive::snapshot::ModelSnapshot;
use simse_core::adaptive::vocabulary::VocabularyManager;

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
