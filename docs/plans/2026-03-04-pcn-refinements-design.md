# PCN Refinements Design

**Goal:** Address three review findings in `simse-predictive-coding`: eliminate per-call network reconstruction in prediction, implement auto-save, and fix the hardcoded inference rate.

**Scope:** `snapshot.rs`, `trainer.rs`, `config.rs`, `network.rs` (accessor only) — all within the existing `simse-predictive-coding` crate.

---

## 1. Cached Network in ModelSnapshot

### Problem

`ModelSnapshot::predict()` creates a full `PredictiveCodingNetwork`, copies all weights from the snapshot, runs inference, and discards the network — every call. For a production-sized model ([512, 256, 64] layers, ~1500-dim input), this allocates and copies tens of thousands of f64 values per prediction.

### Design

Add a `#[serde(skip)]` cached network field to `ModelSnapshot`:

```rust
use std::cell::RefCell;

pub struct ModelSnapshot {
    // ... existing fields ...

    /// Lazily-built network for running inference. Skipped during
    /// serialization — rebuilt on first predict() call.
    #[serde(skip)]
    cached_network: RefCell<Option<PredictiveCodingNetwork>>,
}
```

On `predict()`:
1. Check if `cached_network` is `Some`. If so, use it directly.
2. If `None`, build the network from snapshot data (same logic as today), store it in the `RefCell`, then use it.
3. The `infer()` call already randomizes latent values each time, so reuse is safe.

When a new `ModelSnapshot` is created (via `from_network()` or deserialization), the cache starts empty. The training worker creates a new snapshot each batch, so stale caches are never a problem — the old snapshot (and its cache) is dropped when the `Arc<RwLock>` pointer swaps.

`ModelSnapshot` gains interior mutability but remains logically immutable — the cache is a pure performance optimization that does not change observable behavior.

### Impact on Clone and Serialize

- `#[serde(skip)]` means the cache is not serialized/deserialized. Deserialized snapshots rebuild lazily.
- `Clone` must be manually implemented (or derived with the cache reset to `None`) since `RefCell<Option<PredictiveCodingNetwork>>` does not auto-derive well. We'll implement `Clone` manually to clone data fields and set `cached_network` to `RefCell::new(None)`.

---

## 2. Auto-Save After N Epochs

### Problem

`PcnConfig::auto_save_epochs` (default 100) is declared but never used. The design doc specifies auto-saving snapshots periodically.

### Design

In `TrainingWorker::train_batch()`, after the snapshot swap, check:

```rust
if config.auto_save_epochs > 0
    && stats.epochs % config.auto_save_epochs == 0
    && config.storage_path.is_some()
{
    let path = format!("{}/pcn-auto-{}.json.gz", storage_path, stats.epochs);
    if let Err(e) = save_snapshot(&new_snapshot, &path, true) {
        warn!(error = %e, "Auto-save failed");
    }
}
```

Key decisions:
- Auto-save uses gzip compression (saves disk space, training is not latency-sensitive).
- File naming: `pcn-auto-{epoch}.json.gz` in the configured `storage_path` directory.
- Errors are logged as warnings, never crash the training loop.
- `auto_save_epochs == 0` disables auto-save.

---

## 3. Store inference_rate in ModelSnapshot

### Problem

`ModelSnapshot::predict()` hardcodes `inference_rate: 0.1`. If training used a different rate, prediction behavior diverges.

### Design

Add `inference_rate: f64` to `ModelSnapshot`:

```rust
pub struct ModelSnapshot {
    // ... existing fields ...
    pub inference_rate: f64,
}
```

Capture from network in `from_network()`:

```rust
inference_rate: net.inference_rate(),
```

This requires adding a `pub fn inference_rate(&self) -> f64` accessor to `PredictiveCodingNetwork`.

Use in `predict()`:

```rust
let config = PcnConfig {
    inference_rate: self.inference_rate,
    // ...
};
```

Backward compatibility for deserialization of old snapshots:

```rust
#[serde(default = "default_inference_rate")]
pub inference_rate: f64,

fn default_inference_rate() -> f64 { 0.1 }
```

Update `ModelSnapshot::empty()` to set `inference_rate: 0.1`.

---

## Testing

- **Cached network**: Test that second `predict()` call produces identical results (cache hit). Test that `Clone` produces independent snapshot. Test that serialization round-trip works (cache rebuilt).
- **Auto-save**: Test that files appear in storage_path after training N epochs. Test that `auto_save_epochs == 0` produces no files. Test that missing `storage_path` skips auto-save.
- **Inference rate**: Test that snapshot captures `inference_rate` from network. Test that deserialized snapshot without `inference_rate` defaults to 0.1.
