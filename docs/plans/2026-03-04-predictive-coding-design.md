# simse-predictive-coding Design

## Overview

A standalone Rust crate implementing Predictive Coding Networks (PCNs) for learning behavioral memory models from validated library data. The crate runs as a JSON-RPC 2.0 / NDJSON stdio binary, subscribes to CirculationDesk events for training data, and serves prediction queries without blocking library operations.

Based on:
- [Introduction to PCNs for ML (Stenlund 2025)](https://arxiv.org/abs/2506.06332)
- [Efficient Online Learning with PCN-TA (IROS 2025)](https://arxiv.org/abs/2510.25993)

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Architecture | Standalone JSON-RPC crate | Matches simse pattern (one crate = one concern) |
| Data source | CirculationDesk event subscription | Non-blocking by design; PCN is a subscriber, not in the critical path |
| Input representation | Dual: embeddings + structured metadata | Richer signal from both semantic and categorical features |
| Query interface | JSON-RPC prediction methods | Consistent with all other simse crates |
| Model scale | Configurable at runtime | Layer count and dimensions specified in config |

## PCN Core Algorithm

Hierarchical predictive coding network with local Hebbian-like weight updates. No global gradient tape or backpropagation required.

### Layer Structure

`L` configurable layers of latent variables `x(l) in R^d_l` plus an input layer `x(0)`. Each layer holds:
- Value nodes `x(l)` — current latent state
- Error nodes `e(l)` — prediction error at this layer
- Weight matrix `W(l)` — generative weights predicting layer below
- Bias `b(l)` — per-layer bias
- Activation function — configurable per layer (ReLU default, Tanh, Sigmoid)

### Inference Step (Prediction Error Minimization)

For each layer `l`:
```
prediction:  x_hat(l) = f(W(l) * x(l+1) + b(l))
error:       e(l) = x(l) - x_hat(l)
update:      x(l) <- x(l) - lr_infer * (e(l) - W(l-1)^T * (f'(a(l-1)) . e(l-1)))
```

### Weight Update (Local Hebbian Rule)

```
W(l) <- W(l) + lr_learn * (f'(a(l)) . e(l)) * x(l+1)^T
```

Each weight update depends only on the activity and prediction error of adjacent neurons — no global coordination needed.

### Temporal Amortization (PCN-TA)

Instead of randomly initializing latents for each new input, carry forward the latent states from the previous sample. This halves inference iterations for temporally correlated data (sequential conversation turns).

### Energy Function

`E = (1/2) * sum ||e(l)||^2` — total prediction error across all layers, minimized during inference.

## Input Encoding

Combined input vector from library events:

| Segment | Source | Dimensions |
|---------|--------|------------|
| Semantic embedding | Volume f32 embedding | `d_embed` (e.g., 768 or 1536) |
| Topic encoding | One-hot over known topic vocabulary | `d_topics` (grows dynamically, capped) |
| Tag bitmap | Binary presence vector over known tags | `d_tags` (grows dynamically, capped) |
| Entry type | One-hot: fact/decision/observation | 3 |
| Temporal features | Normalized timestamp, time-since-last, session ordinal | 3 |
| Action context | One-hot: extraction/compendium/reorganization/optimization | 4 |

Total input dimension: `d_embed + d_topics + d_tags + 10`

### Dynamic Vocabulary

A `VocabularyManager` maintains string-to-index mappings for topics and tags. When new topics/tags appear, the input layer weights are resized (new columns initialized to small random values).

### Event Flow

```
CirculationDesk events -> mpsc channel -> TrainingWorker
                                           |-- encode input vector
                                           |-- run inference (T steps)
                                           |-- update weights (local Hebbian)
                                           +-- swap ModelSnapshot (Arc<RwLock>)
```

Batch accumulation: Events are buffered into mini-batches (configurable size, default 16). If the buffer sits for longer than `max_batch_delay_ms` (default 1000ms), it trains on whatever is available.

## Prediction & Query API

### Pattern Recognition

- `predict/patterns` — Given a partial context (embedding, topic, recent actions), predict likely next patterns. Returns ranked predictions with confidence. Use case: "what does the user typically do after asking about X?"
- `predict/associations` — Given an entry, predict associated topics/tags/entry types. The PCN's generative model runs top-down from the entry's latent representation.

### Behavioral Prediction

- `predict/behavior` — Given user action history (last N actions encoded), predict likely next behaviors. Temporal amortization makes this fast since the latent state is already primed.
- `predict/mistakes` — Feed in an action context; the model returns high-energy (high prediction error) patterns. Anomalies = common mistakes or unusual behavior worth flagging.

### Introspection

- `predict/anomalies` — Return the top-K highest-energy samples from recent inference. Patterns the model was worst at predicting represent novel/unexpected behavior.
- `predict/confidence` — Given an input, return the per-layer energy breakdown. Low total energy = confident understanding. High energy = novel territory.

### Model Management

- `model/stats` — Training epochs, total samples, energy history, layer dimensions, parameter count
- `model/snapshot` — Export current model state for persistence
- `model/restore` — Restore from previously exported state
- `model/reset` — Clear all learned state
- `model/configure` — Update layer config (triggers architecture rebuild with weight transfer where dimensions match)

### Lifecycle

- `pcn/initialize` — Initialize the PCN engine with config
- `pcn/dispose` — Shut down cleanly
- `pcn/health` — Health check

## Concurrency Model

Training must never block librarians or inference queries.

```
+-----------------------------------------------------+
|                  JSON-RPC Server                     |
|                  (main tokio runtime)                |
|                                                      |
|   +--------------+        +---------------------+   |
|   | RPC Handlers |--read--| Arc<RwLock<Snapshot>>|   |
|   | (predict/*)  |        |   (read-biased)      |   |
|   +--------------+        +----------+----------+   |
|                                      | write (rare)  |
|   +--------------+        +----------+----------+   |
|   | Event Sub    |--push--| TrainingWorker       |   |
|   | (mpsc rx)    |        | (dedicated thread)   |   |
|   +--------------+        |  - batch accumulate  |   |
|                           |  - inference loop    |   |
|                           |  - weight update     |   |
|                           |  - snapshot swap     |   |
|                           +---------------------+   |
+-----------------------------------------------------+
```

### Guarantees

1. **RwLock snapshot pattern**: Model stored as immutable `ModelSnapshot`. Prediction queries take a read lock (many concurrent readers). Training worker takes a write lock only to swap the pointer after completing a full batch — O(1) pointer swap, write lock held for nanoseconds.

2. **Dedicated training thread**: `tokio::task::spawn_blocking` runs training on a separate OS thread. Inference loop is pure CPU math — no async, no locks during training.

3. **Backpressure**: Bounded mpsc channel (default 1024 events). If full, new events are dropped with a metric counter. Degraded training is acceptable; blocking the library is not.

4. **No lock held across inference iterations**: Worker operates on its own private copy of the weights during the T-step error minimization loop. Readers use the previous snapshot unimpeded.

## Persistence

Same pattern as simse-vector:
- Model weights serialized as base64-encoded Float32 LE arrays
- Full state: weights, biases, vocabulary mappings, layer config, training metadata
- Optional gzip compression
- Auto-save after every N training epochs (configurable, default 100)
- `model/snapshot` and `model/restore` JSON-RPC methods for explicit save/load

## Error Handling

`PcnError` enum with domain variants:
- `NotInitialized` — PCN engine not yet initialized
- `InvalidConfig` — Invalid layer config or parameters
- `TrainingFailed` — Training batch failed (logged, model retains previous state)
- `InferenceTimeout` — Inference iterations exceeded max steps
- `ModelCorrupt` — Deserialized model state is invalid
- `VocabularyOverflow` — Topic/tag vocabulary exceeded cap

JSON-RPC error format: `{ code: -32000, message: "...", data: { pcnCode: "NOT_INITIALIZED" } }`

Training errors are logged to stderr (tracing) but never crash the server.

## Testing Strategy

- **Unit tests**: PCN layer math (forward pass, error computation, weight update), input encoder, vocabulary manager, snapshot serialization round-trip
- **Integration tests**: Full training loop on synthetic data, verify energy decreases monotonically, verify prediction queries return sensible results after training
- **Concurrency tests**: Spawn training + concurrent readers, verify no deadlocks, verify readers never see partial state
- **Benchmark tests**: Training throughput (samples/sec) for different layer configs

## Crate Structure

```
simse-predictive-coding/
  Cargo.toml
  src/
    lib.rs              # Module declarations + re-exports
    main.rs             # Binary entry point (JSON-RPC server)
    error.rs            # PcnError enum
    config.rs           # PcnConfig, LayerConfig
    network.rs          # PredictiveCodingNetwork: layers, inference, weight update
    layer.rs            # PcnLayer: single layer with value/error nodes
    encoder.rs          # InputEncoder: builds combined input vectors
    vocabulary.rs       # VocabularyManager: dynamic topic/tag vocabularies
    snapshot.rs         # ModelSnapshot: immutable weight snapshot for lock-free reads
    trainer.rs          # TrainingWorker: batch accumulation, training loop, snapshot swap
    predictor.rs        # Predictor: query handlers reading from snapshot
    persistence.rs      # Serialization/deserialization (base64 Float32 LE + gzip)
    protocol.rs         # JSON-RPC request/response types
    transport.rs        # NdjsonTransport
    server.rs           # PcnServer: JSON-RPC dispatcher
  tests/
    pcn_integration.rs  # Integration tests
```

## Dependencies

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
uuid = { version = "1", features = ["v4"] }
base64 = "0.22"
flate2 = "1"
rand = "0.8"
```
