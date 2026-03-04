# PCN Refinements Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Fix three review findings: cached network in snapshot.predict(), auto-save after N epochs, and inference_rate stored in ModelSnapshot.

**Architecture:** Three independent changes to the existing `simse-predictive-coding` crate. Task 0 adds the `inference_rate` accessor to `network.rs` (prerequisite for Tasks 1-2). Task 1 rewrites `ModelSnapshot` with cached network + inference_rate. Task 2 adds auto-save to `TrainingWorker`. Task 3 cleans up stale comments.

**Tech Stack:** Rust, serde, std::cell::RefCell, flate2, tempfile (dev)

---

### Task 0: Add inference_rate accessor to PredictiveCodingNetwork

**Files:**
- Modify: `simse-predictive-coding/src/network.rs:120` (after `input_predictor_mut`)

**Step 1: Write the failing test**

Add at the end of `network.rs` tests (before the closing `}`):

```rust
#[test]
fn inference_rate_accessor() {
    let config = PcnConfig {
        inference_rate: 0.05,
        ..test_config()
    };
    let net = PredictiveCodingNetwork::new(6, &config);
    assert!((net.inference_rate() - 0.05).abs() < 1e-15);
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-predictive-coding && cargo test network::tests::inference_rate_accessor -- --exact`
Expected: FAIL with "no method named `inference_rate`"

**Step 3: Write minimal implementation**

Add after line 120 in `network.rs` (after `input_predictor_mut`):

```rust
/// Step size for latent value updates during inference.
pub fn inference_rate(&self) -> f64 {
    self.inference_rate
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-predictive-coding && cargo test network::tests::inference_rate_accessor -- --exact`
Expected: PASS

**Step 5: Run full test suite**

Run: `cd simse-predictive-coding && cargo test`
Expected: All 123 tests pass

**Step 6: Commit**

```bash
git add simse-predictive-coding/src/network.rs
git commit -m "feat(pcn): add inference_rate() accessor to PredictiveCodingNetwork"
```

---

### Task 1: Rewrite ModelSnapshot with cached network and inference_rate

This is the largest task. It modifies `ModelSnapshot` to:
1. Add `inference_rate: f64` field (captured from the network)
2. Add `#[serde(skip)]` `cached_network: RefCell<Option<PredictiveCodingNetwork>>` field
3. Implement `Clone` manually (cache resets to `None` on clone)
4. Rewrite `predict()` to use the cached network
5. Update `from_network()` to capture `inference_rate`
6. Update `empty()` to include the new fields
7. Add serde default for backward compatibility

**Files:**
- Modify: `simse-predictive-coding/src/snapshot.rs` (entire impl rewrite)

**Step 1: Write the failing tests**

Add these tests to `snapshot.rs` `mod tests` (before the closing `}`):

```rust
#[test]
fn snapshot_captures_inference_rate() {
    let config = PcnConfig {
        inference_rate: 0.05,
        ..test_config()
    };
    let net = PredictiveCodingNetwork::new(6, &config);
    let vocab = make_vocab();
    let snapshot = ModelSnapshot::from_network(&net, &vocab, 1, 1);
    assert!((snapshot.inference_rate - 0.05).abs() < 1e-15);
}

#[test]
fn snapshot_predict_uses_cached_network() {
    let config = test_config();
    let mut net = PredictiveCodingNetwork::new(6, &config);
    let vocab = make_vocab();
    let input = vec![1.0, 0.5, -0.3, 0.8, 0.0, -1.0];
    for _ in 0..10 {
        net.train_single(&input);
    }
    let snapshot = ModelSnapshot::from_network(&net, &vocab, 10, 10);

    // First call builds cache. Second call reuses it.
    // Both should produce finite energy (deterministic seeds mean same result).
    let r1 = snapshot.predict(&input, 10);
    let r2 = snapshot.predict(&input, 10);
    assert!(r1.energy.is_finite());
    assert!(r2.energy.is_finite());
    // Same deterministic seed → same energy.
    assert!((r1.energy - r2.energy).abs() < 1e-10);
}

#[test]
fn snapshot_clone_has_independent_cache() {
    let config = test_config();
    let net = PredictiveCodingNetwork::new(6, &config);
    let vocab = make_vocab();
    let snapshot = ModelSnapshot::from_network(&net, &vocab, 1, 1);

    // Trigger cache build on original.
    let _ = snapshot.predict(&[1.0, 0.5, -0.3, 0.8, 0.0, -1.0], 5);

    // Clone should start with empty cache (no shared state).
    let cloned = snapshot.clone();
    assert_eq!(cloned.input_dim, snapshot.input_dim);
    assert_eq!(cloned.epoch, snapshot.epoch);
    assert!((cloned.inference_rate - snapshot.inference_rate).abs() < 1e-15);
}

#[test]
fn snapshot_deserialized_defaults_inference_rate() {
    // Simulate an old snapshot JSON without the inferenceRate field.
    let config = test_config();
    let net = PredictiveCodingNetwork::new(6, &config);
    let vocab = make_vocab();
    let snapshot = ModelSnapshot::from_network(&net, &vocab, 1, 1);
    let mut json: serde_json::Value = serde_json::to_value(&snapshot).unwrap();

    // Remove the inferenceRate field to simulate old format.
    json.as_object_mut().unwrap().remove("inferenceRate");

    let restored: ModelSnapshot = serde_json::from_value(json).unwrap();
    // Should default to 0.1.
    assert!((restored.inference_rate - 0.1).abs() < 1e-15);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-predictive-coding && cargo test snapshot::tests::snapshot_captures_inference_rate -- --exact`
Expected: FAIL (no field `inference_rate`)

**Step 3: Implement the changes to ModelSnapshot**

Replace the entire `ModelSnapshot` struct definition (lines 1-38 of snapshot.rs) with:

```rust
use std::cell::RefCell;

use serde::{Deserialize, Serialize};

use crate::config::{LayerConfig, PcnConfig};
use crate::network::PredictiveCodingNetwork;
use crate::vocabulary::{VocabularyManager, VocabularyState};

fn default_inference_rate() -> f64 {
    0.1
}

/// An immutable, serializable snapshot of a trained predictive coding network.
///
/// Captures weights, biases, layer configurations, and vocabulary state so that
/// read-only inference can be performed without holding any locks on the live
/// network. This enables lock-free concurrent prediction reads.
///
/// A lazily-built `PredictiveCodingNetwork` is cached internally (via `RefCell`)
/// to avoid reconstructing the network on every `predict()` call. The cache is
/// skipped during serialization and rebuilt on first use after deserialization.
///
/// Created via [`ModelSnapshot::from_network`] and used via [`ModelSnapshot::predict`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSnapshot {
    /// Dimensionality of the clamped input.
    pub input_dim: usize,
    /// Layer configurations (dim + activation) for each latent layer.
    pub layer_configs: Vec<LayerConfig>,
    /// Weights for each latent layer. `layer_weights[l]` is a flat row-major
    /// matrix of shape `(dim x input_dim)` for layer `l`.
    pub layer_weights: Vec<Vec<f64>>,
    /// Bias vectors for each latent layer.
    pub layer_biases: Vec<Vec<f64>>,
    /// Weights for the input predictor layer (predicts input from first latent layer).
    pub input_predictor_weights: Vec<f64>,
    /// Bias for the input predictor layer.
    pub input_predictor_bias: Vec<f64>,
    /// Input predictor activation (matches first latent layer's activation).
    pub input_predictor_activation: crate::config::Activation,
    /// Serialized vocabulary state for persistence and reconstruction.
    pub vocabulary: VocabularyState,
    /// Training epoch at the time of snapshot.
    pub epoch: usize,
    /// Total number of training samples seen at the time of snapshot.
    pub total_samples: usize,
    /// Inference rate used during training. Stored so prediction uses the
    /// same rate. Defaults to 0.1 for backward compatibility with old snapshots.
    #[serde(default = "default_inference_rate")]
    pub inference_rate: f64,

    /// Lazily-built network for running inference. Skipped during
    /// serialization — rebuilt on first predict() call.
    #[serde(skip)]
    cached_network: RefCell<Option<PredictiveCodingNetwork>>,
}
```

Implement `Clone` manually (add right after the struct, before the `impl ModelSnapshot` block):

```rust
impl Clone for ModelSnapshot {
    fn clone(&self) -> Self {
        Self {
            input_dim: self.input_dim,
            layer_configs: self.layer_configs.clone(),
            layer_weights: self.layer_weights.clone(),
            layer_biases: self.layer_biases.clone(),
            input_predictor_weights: self.input_predictor_weights.clone(),
            input_predictor_bias: self.input_predictor_bias.clone(),
            input_predictor_activation: self.input_predictor_activation,
            vocabulary: self.vocabulary.clone(),
            epoch: self.epoch,
            total_samples: self.total_samples,
            inference_rate: self.inference_rate,
            // Cache is NOT cloned — each clone rebuilds lazily.
            cached_network: RefCell::new(None),
        }
    }
}
```

Update `from_network()` — add `inference_rate` capture (after `total_samples` in the Self block):

```rust
inference_rate: net.inference_rate(),
cached_network: RefCell::new(None),
```

Update `empty()` — add at the end of the Self block:

```rust
inference_rate: 0.1,
cached_network: RefCell::new(None),
```

Rewrite `predict()` to use the cache:

```rust
pub fn predict(&self, input: &[f64], inference_steps: usize) -> PredictionResult {
    assert_eq!(
        input.len(),
        self.input_dim,
        "Input length {} != snapshot input_dim {}",
        input.len(),
        self.input_dim
    );

    let mut cache = self.cached_network.borrow_mut();

    if cache.is_none() {
        // Build the network from snapshot data and cache it.
        let config = PcnConfig {
            layers: self.layer_configs.clone(),
            inference_steps,
            learning_rate: 0.0,
            inference_rate: self.inference_rate,
            temporal_amortization: false,
            ..Default::default()
        };

        let mut net = PredictiveCodingNetwork::new(self.input_dim, &config);

        for l in 0..self.layer_configs.len() {
            let layer = net.layer_mut(l);
            layer.weights.clone_from(&self.layer_weights[l]);
            layer.bias.clone_from(&self.layer_biases[l]);
        }

        let ip = net.input_predictor_mut();
        ip.weights.clone_from(&self.input_predictor_weights);
        ip.bias.clone_from(&self.input_predictor_bias);

        *cache = Some(net);
    }

    let net = cache.as_mut().unwrap();

    // Run inference (infer() randomizes latent values each call).
    let energy = net.infer(input, inference_steps);

    let top_latent = net.get_top_latent();
    let energy_breakdown = net.energy_breakdown();
    let reconstruction = net.generate();

    PredictionResult {
        energy,
        top_latent,
        energy_breakdown,
        reconstruction,
    }
}
```

**Step 4: Run all tests to verify they pass**

Run: `cd simse-predictive-coding && cargo test`
Expected: All tests pass (existing + 4 new = 127+ tests)

**Step 5: Run clippy**

Run: `cd simse-predictive-coding && cargo clippy -- -D warnings`
Expected: Clean

**Step 6: Commit**

```bash
git add simse-predictive-coding/src/snapshot.rs
git commit -m "feat(pcn): cache network in ModelSnapshot and store inference_rate"
```

---

### Task 2: Implement auto-save in TrainingWorker

**Files:**
- Modify: `simse-predictive-coding/src/trainer.rs:128-217` (train_batch method)

**Step 1: Write the failing tests**

Add these tests to `trainer.rs` `mod tests` (before the closing `}`):

```rust
#[tokio::test]
async fn auto_save_creates_files_at_interval() {
    let dir = tempfile::tempdir().unwrap();
    let storage_path = dir.path().to_str().unwrap().to_string();

    let config = PcnConfig {
        auto_save_epochs: 2,
        storage_path: Some(storage_path.clone()),
        ..test_config()
    };
    let embedding_dim = 4;
    let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
    let (tx, rx) = mpsc::channel::<LibraryEvent>(config.channel_capacity);

    // Send 6 events with batch_size=2 → 3 epochs. Auto-save at epoch 2.
    for i in 0..6 {
        tx.send(make_event(vec![0.1 * i as f32; 4], "rust"))
            .await
            .unwrap();
    }
    drop(tx);

    let stats = TrainingWorker::run_batch(rx, snapshot, config, embedding_dim).await;
    assert_eq!(stats.epochs, 3);

    // Should have auto-saved at epoch 2.
    let auto_save_path = format!("{}/pcn-auto-2.json.gz", storage_path);
    assert!(
        std::path::Path::new(&auto_save_path).exists(),
        "Auto-save file should exist at epoch 2: {}",
        auto_save_path
    );

    // Should NOT have auto-saved at epoch 1 or 3.
    assert!(!std::path::Path::new(&format!("{}/pcn-auto-1.json.gz", storage_path)).exists());
    assert!(!std::path::Path::new(&format!("{}/pcn-auto-3.json.gz", storage_path)).exists());
}

#[tokio::test]
async fn auto_save_disabled_when_zero() {
    let dir = tempfile::tempdir().unwrap();
    let storage_path = dir.path().to_str().unwrap().to_string();

    let config = PcnConfig {
        auto_save_epochs: 0,
        storage_path: Some(storage_path.clone()),
        ..test_config()
    };
    let embedding_dim = 4;
    let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
    let (tx, rx) = mpsc::channel::<LibraryEvent>(config.channel_capacity);

    for i in 0..4 {
        tx.send(make_event(vec![0.1 * i as f32; 4], "rust"))
            .await
            .unwrap();
    }
    drop(tx);

    let stats = TrainingWorker::run_batch(rx, snapshot, config, embedding_dim).await;
    assert_eq!(stats.epochs, 2);

    // No auto-save files should exist.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert!(entries.is_empty(), "No auto-save files when auto_save_epochs=0");
}

#[tokio::test]
async fn auto_save_skipped_when_no_storage_path() {
    let config = PcnConfig {
        auto_save_epochs: 1,
        storage_path: None,
        ..test_config()
    };
    let embedding_dim = 4;
    let snapshot = Arc::new(RwLock::new(ModelSnapshot::empty()));
    let (tx, rx) = mpsc::channel::<LibraryEvent>(config.channel_capacity);

    tx.send(make_event(vec![0.1; 4], "rust")).await.unwrap();
    tx.send(make_event(vec![0.2; 4], "rust")).await.unwrap();
    drop(tx);

    // Should complete without errors (no storage_path → no auto-save attempt).
    let stats = TrainingWorker::run_batch(rx, snapshot, config, embedding_dim).await;
    assert_eq!(stats.epochs, 1);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-predictive-coding && cargo test trainer::tests::auto_save_creates_files_at_interval -- --exact`
Expected: FAIL (no auto-save files created)

**Step 3: Implement auto-save in train_batch**

Add the import at the top of `trainer.rs` (after line 9):

```rust
use crate::persistence::save_snapshot;
```

In `train_batch()`, after the snapshot swap block (after line 215, before the closing `}` of `if batch_trained > 0`), add:

```rust
// Auto-save periodically if configured.
if config.auto_save_epochs > 0
    && stats.epochs % config.auto_save_epochs == 0
{
    if let Some(ref storage_path) = config.storage_path {
        let path = format!("{}/pcn-auto-{}.json.gz", storage_path, stats.epochs);
        // Re-read the snapshot we just wrote for saving.
        let snap_to_save = snapshot.read().unwrap();
        if let Err(e) = save_snapshot(&snap_to_save, &path, true) {
            warn!(error = %e, path, "Auto-save failed");
        } else {
            debug!(epoch = stats.epochs, path, "Auto-saved snapshot");
        }
    }
}
```

**Step 4: Run all tests to verify they pass**

Run: `cd simse-predictive-coding && cargo test`
Expected: All tests pass (existing + 3 new)

**Step 5: Run clippy**

Run: `cd simse-predictive-coding && cargo clippy -- -D warnings`
Expected: Clean

**Step 6: Commit**

```bash
git add simse-predictive-coding/src/trainer.rs
git commit -m "feat(pcn): implement auto-save after N training epochs"
```

---

### Task 3: Clean up stale comments in snapshot.rs and verify

**Files:**
- Modify: `simse-predictive-coding/src/snapshot.rs:84-116` (remove stale planning comments in `from_network()`)

**Step 1: Remove stale comments**

In `from_network()`, the large comment block from lines 84-116 (starting with `// Capture the input predictor...` through `// Placeholder — will be filled...`) is leftover planning text. Replace that entire block with a single clean comment:

```rust
// Capture the input predictor weights.
let input_predictor = net.input_predictor();
```

**Step 2: Run full test suite**

Run: `cd simse-predictive-coding && cargo test`
Expected: All tests pass

**Step 3: Run clippy**

Run: `cd simse-predictive-coding && cargo clippy -- -D warnings`
Expected: Clean

**Step 4: Commit and push all changes**

```bash
git add simse-predictive-coding/src/snapshot.rs
git commit -m "refactor(pcn): clean up stale planning comments in snapshot.rs"
git push
```
