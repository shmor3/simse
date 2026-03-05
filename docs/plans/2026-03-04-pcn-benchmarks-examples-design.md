# PCN Benchmarks & Examples Design

## Overview

Add production-quality benchmarks (Criterion) and runnable examples to the `simse-predictive-coding` crate. Benchmarks measure performance characteristics across model sizes. Examples demonstrate the librarian's predictive coding capabilities to both integrators and evaluators.

## Benchmarks

### Framework

Criterion.rs — statistical benchmarking with HTML reports, regression detection, stable Rust.

### File: `benches/pcn_benchmarks.rs`

Single Criterion file with 4 benchmark groups.

#### 1. `bench_inference`

Measures `network.infer()` latency.

| Variant | Layers | Input dim | Steps |
|---------|--------|-----------|-------|
| small/10 | [16, 8] | 24 | 10 |
| small/20 | [16, 8] | 24 | 20 |
| small/50 | [16, 8] | 24 | 50 |
| production/10 | [512, 256, 64] | ~1500 | 10 |
| production/20 | [512, 256, 64] | ~1500 | 20 |
| production/50 | [512, 256, 64] | ~1500 | 50 |

#### 2. `bench_training`

Measures `network.train_single_with_steps()` latency.

| Variant | Layers | Input dim | Amortized |
|---------|--------|-----------|-----------|
| small | [16, 8] | 24 | false |
| small/amortized | [16, 8] | 24 | true |
| production | [512, 256, 64] | ~1500 | false |
| production/amortized | [512, 256, 64] | ~1500 | true |

#### 3. `bench_encoding`

Measures `encoder.encode()` latency.

| Variant | Embedding dim | Vocab fill |
|---------|---------------|------------|
| empty_vocab | 768 | 0 topics, 0 tags |
| half_vocab | 768 | 250 topics, 500 tags |
| full_vocab | 768 | 500 topics, 1000 tags |

#### 4. `bench_snapshot`

Measures snapshot operations.

| Variant | Operation | Scale |
|---------|-----------|-------|
| predict/cold | `snapshot.predict()` first call (cache miss) | small |
| predict/warm | `snapshot.predict()` subsequent call (cache hit) | small |
| predict/cold/production | `snapshot.predict()` first call | production |
| predict/warm/production | `snapshot.predict()` subsequent call | production |
| from_network/small | `ModelSnapshot::from_network()` | small |
| from_network/production | `ModelSnapshot::from_network()` | production |
| save_json/small | `save_snapshot(compress=false)` | small |
| save_gzip/small | `save_snapshot(compress=true)` | small |
| load_json/small | `load_snapshot(compress=false)` | small |
| load_gzip/small | `load_snapshot(compress=true)` | small |
| save_gzip/production | `save_snapshot(compress=true)` | production |
| load_gzip/production | `load_snapshot(compress=true)` | production |

## Examples

### 1. `examples/training_basics.rs`

End-to-end training workflow.

- Small model: [16, 8] layers, 4-dim embeddings
- Encodes 5 library events with different topics/tags
- Trains for 50 epochs, prints energy curve
- Takes snapshot, runs prediction, shows reconstruction error
- **Output:** epoch-vs-energy table, final model stats (layers, params, samples)

### 2. `examples/anomaly_detection.rs`

Detecting unusual patterns via prediction energy.

- Trains on 20 "normal" events (consistent topic/embedding cluster)
- Introduces 5 "anomalous" events (different embeddings, unusual topics)
- Uses `Predictor::anomalies()` to rank all inputs by energy
- **Output:** ranked list showing anomalous inputs have higher energy, clear separation

### 3. `examples/temporal_amortization.rs`

Amortized inference speedup demonstration.

- Production-scale model: [512, 256, 64], 768-dim embeddings
- Generates temporally correlated event sequence
- Compares fresh inference vs amortized inference on same data
- **Output:** side-by-side table (steps, energy_fresh, energy_amortized)

### 4. `examples/model_persistence.rs`

Save/load round-trip verification.

- Trains a model, saves as both JSON and gzip
- Loads both back, runs prediction on each
- Verifies results match original
- **Output:** file sizes, energy comparison, round-trip pass/fail

### 5. `examples/production_scale.rs`

Realistic librarian workload at production scale.

- Config: [512, 256, 64], 768-dim embeddings, 500 topics, 1000 tags
- Simulates 100 events across varied topics with vocabulary growth
- Shows network resizing mid-training when vocabulary grows
- **Output:** architecture summary, parameter count, throughput (samples/sec), energy trend

## Cargo.toml Changes

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
tempfile = "3"

[[bench]]
name = "pcn_benchmarks"
harness = false
```

## File Structure

```
simse-predictive-coding/
  benches/
    pcn_benchmarks.rs        # Criterion benchmarks (4 groups)
  examples/
    training_basics.rs       # End-to-end training
    anomaly_detection.rs     # Anomaly detection via energy
    temporal_amortization.rs # Amortized vs fresh inference
    model_persistence.rs     # Save/load round-trip
    production_scale.rs      # Production-scale workload
```
