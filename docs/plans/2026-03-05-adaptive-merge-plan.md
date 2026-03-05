# Merge simse-vector + simse-predictive-coding → simse-adaptive Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rename `simse-vector/` to `simse-adaptive/`, merge all `simse-predictive-coding/` modules into it, unify shared files (error, protocol, server, persistence), and update all cross-crate references.

**Architecture:** `simse-adaptive/` becomes a single Rust crate (`simse-adaptive-engine`) with one binary and one combined JSON-RPC server dispatching both vector store (`store/*`, `catalog/*`, `learning/*`, `graph/*`, `query/*`, `format/*`) and PCN (`pcn/*`, `feed/*`, `predict/*`, `model/*`) methods. The PCN predictor uses VolumeStore embeddings directly.

**Tech Stack:** Rust, serde, tokio, thiserror, JSON-RPC 2.0 / NDJSON stdio

---

### Task 1: Rename simse-vector → simse-adaptive

This is a pure rename — no code changes yet.

**Step 1: Move the directory**

```bash
git mv simse-vector simse-adaptive
```

**Step 2: Update `simse-adaptive/Cargo.toml` — rename package, lib, and binary**

Change:
- `name = "simse-vector-engine"` → `name = "simse-adaptive-engine"`
- `name = "simse_vector_engine"` → `name = "simse_adaptive_engine"`
- binary name `simse-vector-engine` → `simse-adaptive-engine`
- `description` → `"Adaptive learning engine over JSON-RPC 2.0 / NDJSON stdio"`

**Step 3: Update `simse-adaptive/src/main.rs`**

Replace `use simse_vector_engine::` with `use simse_adaptive_engine::` and update the log message to `"simse-adaptive-engine ready"`.

**Step 4: Update root `Cargo.toml`**

In `exclude` list: `"simse-vector"` → `"simse-adaptive"`, remove `"simse-predictive-coding"`.

**Step 5: Update `simse-core/Cargo.toml`**

Change: `simse-vector-engine = { path = "../simse-vector" }` → `simse-adaptive-engine = { path = "../simse-adaptive" }`

**Step 6: Update all `use simse_vector_engine::` in simse-core**

Files to modify (find-and-replace `simse_vector_engine` → `simse_adaptive_engine`):
- `simse-core/src/error.rs`
- `simse-core/src/library/mod.rs`
- `simse-core/src/library/library.rs`
- `simse-core/src/library/shelf.rs`
- `simse-core/src/library/query_dsl.rs`
- `simse-core/src/library/prompt_inject.rs`
- `simse-core/src/library/services.rs`

**Step 7: Update `simse-vector/tests/integration.rs`** (now at `simse-adaptive/tests/integration.rs`)

Replace `use simse_vector_engine::` with `use simse_adaptive_engine::`.

**Step 8: Verify build**

```bash
cd simse-adaptive && cargo build
cd ../simse-core && cargo build
```

Expected: Both build successfully.

**Step 9: Run tests**

```bash
cd simse-adaptive && cargo test
cd ../simse-core && cargo test
```

Expected: All existing tests pass.

**Step 10: Commit**

```bash
git add -A
git commit -m "refactor: rename simse-vector → simse-adaptive"
```

---

### Task 2: Copy PCN modules into simse-adaptive

Copy the PCN-only source files (no conflicts with existing files). These will not compile yet — that's expected.

**Step 1: Copy PCN-only modules**

```bash
cp simse-predictive-coding/src/config.rs simse-adaptive/src/pcn_config.rs
cp simse-predictive-coding/src/encoder.rs simse-adaptive/src/encoder.rs
cp simse-predictive-coding/src/vocabulary.rs simse-adaptive/src/vocabulary.rs
cp simse-predictive-coding/src/network.rs simse-adaptive/src/network.rs
cp simse-predictive-coding/src/layer.rs simse-adaptive/src/layer.rs
cp simse-predictive-coding/src/predictor.rs simse-adaptive/src/predictor.rs
cp simse-predictive-coding/src/trainer.rs simse-adaptive/src/trainer.rs
cp simse-predictive-coding/src/snapshot.rs simse-adaptive/src/snapshot.rs
```

Note: PCN's `config.rs` is copied as `pcn_config.rs` to avoid confusion (vector doesn't have a config.rs).

**Step 2: Copy PCN tests and benchmarks**

```bash
cp simse-predictive-coding/tests/pcn_integration.rs simse-adaptive/tests/pcn_integration.rs
mkdir -p simse-adaptive/benches
cp simse-predictive-coding/benches/pcn_benchmarks.rs simse-adaptive/benches/pcn_benchmarks.rs
```

**Step 3: Update imports in copied files**

In all copied files, replace:
- `crate::config::` → `crate::pcn_config::`
- `crate::error::PcnError` → `crate::error::AdaptiveError` (will exist after Task 3)
- `crate::persistence::` references for PCN snapshot functions (will be merged in Task 4)

Do NOT try to compile yet — imports will break until Tasks 3-5 complete.

**Step 4: Commit**

```bash
git add simse-adaptive/src/pcn_config.rs simse-adaptive/src/encoder.rs simse-adaptive/src/vocabulary.rs simse-adaptive/src/network.rs simse-adaptive/src/layer.rs simse-adaptive/src/predictor.rs simse-adaptive/src/trainer.rs simse-adaptive/src/snapshot.rs simse-adaptive/tests/pcn_integration.rs simse-adaptive/benches/pcn_benchmarks.rs
git commit -m "feat(simse-adaptive): copy PCN modules into adaptive crate"
```

---

### Task 3: Merge error.rs

Combine `VectorError` and `PcnError` into a single `AdaptiveError` enum.

**Step 1: Update `simse-adaptive/src/error.rs`**

Rename `VectorError` → `AdaptiveError`. Add all PCN variants. Update `code()` to include PCN codes. Update `to_json_rpc_error()` to use `"adaptiveCode"` key.

The new enum should contain all existing vector variants plus:
- `InvalidConfig(String)`
- `TrainingFailed(String)`
- `InferenceTimeout`
- `ModelCorrupt(String)`
- `VocabularyOverflow(String)`
- `InvalidParams(String)`
- `Json(serde_json::Error)` (add `#[from]`)

**Step 2: Update all `VectorError` references in simse-adaptive/src/**

Find-and-replace `VectorError` → `AdaptiveError` in all files under `simse-adaptive/src/`.

**Step 3: Update simse-core references**

In `simse-core/src/error.rs`: change `Vector(#[from] simse_adaptive_engine::error::VectorError)` → `Adaptive(#[from] simse_adaptive_engine::error::AdaptiveError)` and update the variant name throughout simse-core.

**Step 4: Verify build**

```bash
cd simse-adaptive && cargo check
cd ../simse-core && cargo check
```

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor(simse-adaptive): merge VectorError + PcnError → AdaptiveError"
```

---

### Task 4: Merge protocol.rs and persistence.rs

**Step 1: Update `simse-adaptive/src/protocol.rs`**

- Rename `VECTOR_ERROR` → `ADAPTIVE_ERROR` (keep value `-32000`)
- Add the PCN param structs: `PcnInitializeParams`, `FeedEventParams`, `PredictConfidenceParams`, `PredictAnomaliesParams`, `ModelSnapshotParams`, `ModelRestoreParams`, `ModelConfigureParams`
- Add imports for `crate::pcn_config::PcnConfig` and `crate::encoder::LibraryEvent`
- Keep all existing vector protocol types

**Step 2: Update `simse-adaptive/src/persistence.rs`**

Add the PCN snapshot save/load functions from `simse-predictive-coding/src/persistence.rs`. The vector persistence already handles base64 + gzip for embeddings. The PCN persistence handles model snapshot serialization. They operate on different data types so they coexist cleanly.

**Step 3: Update all `VECTOR_ERROR` references in server.rs**

Replace `VECTOR_ERROR` → `ADAPTIVE_ERROR`.

**Step 4: Update PCN modules to use merged files**

In the copied PCN modules, update `use crate::protocol::*` (they already import from `crate::protocol`, just need the renamed constants). Update `use crate::persistence::` to point to the merged persistence module.

**Step 5: Add new dependencies to `simse-adaptive/Cargo.toml`**

Add from PCN's Cargo.toml:
- `tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }`
- `rand = "0.8"`

Add to `[dev-dependencies]`:
- `criterion = { version = "0.5", features = ["html_reports"] }`

Add the `[[bench]]` section:
```toml
[[bench]]
name = "pcn_benchmarks"
harness = false
```

**Step 6: Update `simse-adaptive/src/lib.rs`**

Add the new PCN module declarations:
```rust
pub mod pcn_config;
pub mod encoder;
pub mod vocabulary;
pub mod network;
pub mod layer;
pub mod predictor;
pub mod trainer;
pub mod snapshot;
```

Also add `#![allow(clippy::needless_range_loop)]` at the crate root (from PCN).

**Step 7: Verify build**

```bash
cd simse-adaptive && cargo check
```

Expected: Compiles (all modules can resolve their imports).

**Step 8: Commit**

```bash
git add -A
git commit -m "feat(simse-adaptive): merge protocol, persistence, and wire up PCN modules"
```

---

### Task 5: Merge server.rs — combined JSON-RPC dispatcher

**Step 1: Update `simse-adaptive/src/server.rs`**

Rename `VectorServer` → `AdaptiveServer`. Add PCN fields:
- `snapshot: Arc<RwLock<ModelSnapshot>>`
- `predictor: Option<Predictor>`
- `event_tx: Option<mpsc::Sender<LibraryEvent>>`
- `pcn_initialized: bool`
- `pcn_config: Option<PcnConfig>`
- `embedding_dim: usize`

The `run()` method changes from sync to `async` (PCN needs tokio). Update `dispatch()` to `async` and add all PCN method routes:
- `"pcn/initialize"` → `handle_pcn_initialize`
- `"pcn/dispose"` → `handle_pcn_dispose`
- `"pcn/health"` → `handle_pcn_health`
- `"feed/event"` → `handle_feed_event`
- `"predict/confidence"` → `handle_predict_confidence`
- `"predict/anomalies"` → `handle_predict_anomalies`
- `"model/stats"` → `handle_model_stats`
- `"model/snapshot"` → `handle_model_snapshot`
- `"model/restore"` → `handle_model_restore`
- `"model/reset"` → `handle_model_reset`

Move all PCN handler methods from the old `PcnServer` into `AdaptiveServer`. The PCN error path uses the same `ADAPTIVE_ERROR` code.

**Step 2: Update `simse-adaptive/src/main.rs`**

Change from sync to async (add `#[tokio::main]`), launch `AdaptiveServer`.

**Step 3: Update integration tests**

- `simse-adaptive/tests/integration.rs`: update `use simse_adaptive_engine::server::AdaptiveServer` (was `VectorServer`)
- `simse-adaptive/tests/pcn_integration.rs`: update to use `AdaptiveServer` instead of `PcnServer`

**Step 4: Verify build + tests**

```bash
cd simse-adaptive && cargo test
cd ../simse-core && cargo test
```

Expected: All tests pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "feat(simse-adaptive): unified AdaptiveServer with vector + PCN dispatch"
```

---

### Task 6: Delete simse-predictive-coding + update CLAUDE.md

**Step 1: Delete the old crate**

```bash
rm -rf simse-predictive-coding
```

**Step 2: Update CLAUDE.md**

- Remove `simse-predictive-coding` from repository layout
- Rename all `simse-vector` references to `simse-adaptive`
- Update the "Other Rust Crates" section: rename `simse-vector/` tree to `simse-adaptive/`, add PCN modules to the file tree
- Remove the "Predictive Coding Engine" section (merged into adaptive)
- Update build commands: `build:vector-engine` → `build:adaptive-engine`
- Update test commands: `cd simse-vector && cargo test` → `cd simse-adaptive && cargo test`
- Update Key Patterns if needed

**Step 3: Verify everything builds**

```bash
cd simse-adaptive && cargo test
cd ../simse-core && cargo test
```

**Step 4: Commit**

```bash
git add -A
git commit -m "chore: delete simse-predictive-coding, update CLAUDE.md for simse-adaptive"
```

---

### Task 7: Final verification

**Step 1: Full workspace build**

```bash
cargo build --workspace
cargo test --workspace
```

**Step 2: Standalone adaptive build**

```bash
cd simse-adaptive && cargo test
cd simse-adaptive && cargo clippy -- -D warnings
```

**Step 3: Confirm no stale references**

```bash
grep -r "simse.vector" --include="*.rs" --include="*.toml" --include="*.md" .
grep -r "simse.pcn" --include="*.rs" --include="*.toml" --include="*.md" .
grep -r "simse-predictive" --include="*.rs" --include="*.toml" --include="*.md" .
```

Expected: No matches (except possibly in docs/plans/ design docs, which is fine).

**Step 4: Push**

```bash
git push origin main
```
