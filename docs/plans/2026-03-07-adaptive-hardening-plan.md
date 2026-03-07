# simse-adaptive Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add HNSW indexing, SIMD distance metrics, vector quantization, Rayon parallelism, MMR/RRF fusion, and SoA memory layout to simse-adaptive.

**Architecture:** Tiered index (Flat + HNSW) behind an `IndexBackend` trait. SIMD-accelerated distance for 4 metrics. Scalar/Binary quantization with exact reranking. Rayon for parallel search/batch. MMR diversity + RRF hybrid fusion. All wired into existing Store + JSON-RPC server.

**Tech Stack:** Rust, hnsw_rs, rayon, std::arch SIMD intrinsics, criterion benchmarks.

**Design doc:** `docs/plans/2026-03-07-adaptive-hardening-design.md`

---

### Task 1: Distance Module — Scalar Implementations

Replace `cosine.rs` with a unified `distance.rs` supporting 4 metrics with pure Rust scalar implementations.

**Files:**
- Create: `simse-adaptive/src/distance.rs`
- Delete: `simse-adaptive/src/cosine.rs`
- Modify: `simse-adaptive/src/lib.rs` (swap module declaration)
- Modify: `simse-adaptive/src/store.rs` (update imports)
- Modify: `simse-adaptive/src/deduplication.rs` (update imports)
- Modify: `simse-adaptive/src/recommendation.rs` (update imports)

**Step 1: Write failing tests in distance.rs**

Add unit tests at the bottom of the new `distance.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_identical_vectors() {
        let a = vec![1.0f32, 2.0, 3.0];
        assert!((cosine_distance(&a, &a) - 0.0).abs() < 1e-6);
        assert!((cosine_similarity_score(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_vectors() {
        let a = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 1.0];
        assert!((cosine_similarity_score(&a, &b) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_opposite_vectors() {
        let a = vec![1.0f32, 0.0];
        let b = vec![-1.0f32, 0.0];
        assert!((cosine_similarity_score(&a, &b) - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn euclidean_identical() {
        let a = vec![1.0f32, 2.0, 3.0];
        assert!((euclidean_distance(&a, &a) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn euclidean_known_distance() {
        let a = vec![0.0f32, 0.0];
        let b = vec![3.0f32, 4.0];
        assert!((euclidean_distance(&a, &b) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn dot_product_known() {
        let a = vec![1.0f32, 2.0, 3.0];
        let b = vec![4.0f32, 5.0, 6.0];
        // dot = 4+10+18 = 32, negative_dot = -32
        assert!((dot_product_distance(&a, &b) - (-32.0)).abs() < 1e-6);
    }

    #[test]
    fn manhattan_known() {
        let a = vec![1.0f32, 2.0, 3.0];
        let b = vec![4.0f32, 6.0, 3.0];
        // |3| + |4| + |0| = 7
        assert!((manhattan_distance(&a, &b) - 7.0).abs() < 1e-6);
    }

    #[test]
    fn zero_vector_cosine() {
        let a = vec![0.0f32, 0.0];
        let b = vec![1.0f32, 2.0];
        assert!((cosine_similarity_score(&a, &b) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn empty_vectors() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert!((cosine_similarity_score(&a, &b) - 0.0).abs() < 1e-6);
        assert!((euclidean_distance(&a, &b) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn distance_fn_dispatch() {
        let a = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 1.0];
        let f = DistanceMetric::Cosine.distance_fn();
        let d = f(&a, &b);
        assert!((d - 1.0).abs() < 1e-6); // cosine distance = 1 - 0 = 1
    }

    #[test]
    fn similarity_fn_dispatch() {
        let a = vec![1.0f32, 2.0, 3.0];
        let sim = DistanceMetric::Cosine.similarity(&a, &a);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn compute_magnitude_known() {
        let a = vec![3.0f32, 4.0];
        assert!((compute_magnitude(&a) - 5.0).abs() < 1e-6);
    }

    #[test]
    fn with_magnitude_matches_without() {
        let a = vec![1.0f32, 2.0, 3.0];
        let b = vec![4.0f32, 5.0, 6.0];
        let direct = cosine_similarity_score(&a, &b);
        let mag_a = compute_magnitude(&a);
        let mag_b = compute_magnitude(&b);
        let cached = cosine_similarity_with_magnitude(&a, &b, mag_a, mag_b);
        assert!((direct - cached).abs() < 1e-10);
    }
}
```

**Step 2: Implement distance.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DistanceMetric {
    #[default]
    Cosine,
    Euclidean,
    DotProduct,
    Manhattan,
}

impl DistanceMetric {
    /// Returns a distance function (lower = more similar for Cosine/Euclidean/Manhattan,
    /// more negative = more similar for DotProduct).
    pub fn distance_fn(self) -> fn(&[f32], &[f32]) -> f64 {
        match self {
            Self::Cosine => cosine_distance,
            Self::Euclidean => euclidean_distance,
            Self::DotProduct => dot_product_distance,
            Self::Manhattan => manhattan_distance,
        }
    }

    /// Returns a similarity score (higher = more similar). Range depends on metric.
    /// Cosine: [-1, 1], DotProduct: unbounded, Euclidean/Manhattan: 1/(1+d).
    pub fn similarity(self, a: &[f32], b: &[f32]) -> f64 {
        match self {
            Self::Cosine => cosine_similarity_score(a, b),
            Self::DotProduct => dot_product_similarity(a, b),
            Self::Euclidean => {
                let d = euclidean_distance(a, b);
                1.0 / (1.0 + d)
            }
            Self::Manhattan => {
                let d = manhattan_distance(a, b);
                1.0 / (1.0 + d)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Cosine
// ---------------------------------------------------------------------------

pub fn cosine_similarity_score(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..a.len() {
        let ai = a[i] as f64;
        let bi = b[i] as f64;
        dot += ai * bi;
        na += ai * ai;
        nb += bi * bi;
    }
    let denom = na.sqrt() * nb.sqrt();
    if denom == 0.0 { return 0.0; }
    let r = dot / denom;
    if !r.is_finite() { return 0.0; }
    r.clamp(-1.0, 1.0)
}

pub fn cosine_distance(a: &[f32], b: &[f32]) -> f64 {
    1.0 - cosine_similarity_score(a, b)
}

pub fn compute_magnitude(embedding: &[f32]) -> f64 {
    let mut sum = 0.0f64;
    for &v in embedding {
        let vf = v as f64;
        sum += vf * vf;
    }
    sum.sqrt()
}

pub fn cosine_similarity_with_magnitude(a: &[f32], b: &[f32], mag_a: f64, mag_b: f64) -> f64 {
    if a.len() != b.len() || a.is_empty() { return 0.0; }
    let denom = mag_a * mag_b;
    if denom == 0.0 { return 0.0; }
    let mut dot = 0.0f64;
    for i in 0..a.len() {
        dot += (a[i] as f64) * (b[i] as f64);
    }
    let r = dot / denom;
    if !r.is_finite() { return 0.0; }
    r.clamp(-1.0, 1.0)
}

// ---------------------------------------------------------------------------
// Euclidean (L2)
// ---------------------------------------------------------------------------

pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() { return 0.0; }
    let mut sum = 0.0f64;
    for i in 0..a.len() {
        let d = (a[i] as f64) - (b[i] as f64);
        sum += d * d;
    }
    sum.sqrt()
}

// ---------------------------------------------------------------------------
// Dot Product
// ---------------------------------------------------------------------------

pub fn dot_product_distance(a: &[f32], b: &[f32]) -> f64 {
    -dot_product_similarity(a, b)
}

pub fn dot_product_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() { return 0.0; }
    let mut dot = 0.0f64;
    for i in 0..a.len() {
        dot += (a[i] as f64) * (b[i] as f64);
    }
    dot
}

// ---------------------------------------------------------------------------
// Manhattan (L1)
// ---------------------------------------------------------------------------

pub fn manhattan_distance(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() { return 0.0; }
    let mut sum = 0.0f64;
    for i in 0..a.len() {
        sum += ((a[i] as f64) - (b[i] as f64)).abs();
    }
    sum
}
```

**Step 3: Update lib.rs — swap cosine for distance**

Replace `pub mod cosine;` with `pub mod distance;`.

**Step 4: Update all imports**

In `store.rs`, `deduplication.rs`, `recommendation.rs`, and anywhere else that imports from `cosine`:
- Replace `use crate::cosine::*` with `use crate::distance::*`

The public API is the same: `cosine_similarity_score` (was `cosine_similarity`), `compute_magnitude`, `cosine_similarity_with_magnitude`.

Note: `cosine_similarity` is renamed to `cosine_similarity_score` to distinguish from `DistanceMetric::Cosine`. Add a `pub use` alias if needed for backwards compat:
```rust
pub use cosine_similarity_score as cosine_similarity;
```

**Step 5: Run tests**

```bash
cd simse-adaptive && cargo test
```

Expected: All existing tests pass + new distance unit tests pass.

**Step 6: Delete cosine.rs**

Remove `simse-adaptive/src/cosine.rs`.

**Step 7: Run tests again**

```bash
cd simse-adaptive && cargo test
```

Expected: All tests pass.

**Step 8: Commit**

```bash
git add -A simse-adaptive/src/distance.rs simse-adaptive/src/lib.rs simse-adaptive/src/store.rs simse-adaptive/src/deduplication.rs simse-adaptive/src/recommendation.rs
git rm simse-adaptive/src/cosine.rs
git commit -m "refactor(simse-adaptive): replace cosine.rs with unified distance module

Four metrics: Cosine, Euclidean, DotProduct, Manhattan.
DistanceMetric enum with distance_fn() and similarity() dispatch."
```

---

### Task 2: SIMD Distance Acceleration

Add SIMD-accelerated paths for all distance metrics using `std::arch` with runtime feature detection.

**Files:**
- Modify: `simse-adaptive/src/distance.rs` (add SIMD implementations)

**Step 1: Write SIMD vs scalar equivalence tests**

Add to the `#[cfg(test)]` block in `distance.rs`:

```rust
#[test]
fn simd_cosine_matches_scalar() {
    let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
    let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();
    let scalar = cosine_similarity_score(&a, &b);
    let simd = simd_cosine_similarity(&a, &b);
    assert!((scalar - simd).abs() < 1e-5, "scalar={scalar} simd={simd}");
}

#[test]
fn simd_euclidean_matches_scalar() {
    let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
    let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();
    let scalar = euclidean_distance(&a, &b);
    let simd = simd_euclidean_distance(&a, &b);
    assert!((scalar - simd).abs() < 1e-4, "scalar={scalar} simd={simd}");
}

#[test]
fn simd_dot_product_matches_scalar() {
    let a: Vec<f32> = (0..384).map(|i| (i as f32) * 0.01).collect();
    let b: Vec<f32> = (0..384).map(|i| ((384 - i) as f32) * 0.01).collect();
    let scalar = dot_product_similarity(&a, &b);
    let simd = simd_dot_product(&a, &b);
    assert!((scalar - simd).abs() < 1e-3, "scalar={scalar} simd={simd}");
}

#[test]
fn simd_short_vector() {
    // Vectors shorter than SIMD width should still work (scalar fallback)
    let a = vec![1.0f32, 2.0];
    let b = vec![3.0f32, 4.0];
    let scalar = cosine_similarity_score(&a, &b);
    let simd = simd_cosine_similarity(&a, &b);
    assert!((scalar - simd).abs() < 1e-6);
}
```

**Step 2: Implement SIMD functions**

Add SIMD-accelerated functions that detect features at runtime and dispatch:

```rust
/// SIMD-accelerated cosine similarity. Falls back to scalar.
pub fn simd_cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma") {
            return unsafe { avx2_cosine_similarity(a, b) };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        // NEON is always available on aarch64
        return unsafe { neon_cosine_similarity(a, b) };
    }
    cosine_similarity_score(a, b)
}
```

Implement `#[target_feature(enable = "avx2,fma")]` functions for x86_64 and `#[target_feature(enable = "neon")]` for aarch64. Each processes 8 floats (AVX2) or 4 floats (NEON) per iteration with a scalar tail loop.

Similarly for `simd_euclidean_distance`, `simd_dot_product`, `simd_manhattan_distance`.

**Step 3: Update DistanceMetric to use SIMD paths**

Replace the scalar function pointers in `distance_fn()` and `similarity()` with the SIMD-dispatching versions:

```rust
impl DistanceMetric {
    pub fn similarity(self, a: &[f32], b: &[f32]) -> f64 {
        match self {
            Self::Cosine => simd_cosine_similarity(a, b),
            Self::DotProduct => simd_dot_product(a, b),
            Self::Euclidean => 1.0 / (1.0 + simd_euclidean_distance(a, b)),
            Self::Manhattan => 1.0 / (1.0 + simd_manhattan_distance(a, b)),
        }
    }
}
```

**Step 4: Run tests**

```bash
cd simse-adaptive && cargo test
```

Expected: All tests pass including SIMD equivalence tests.

**Step 5: Commit**

```bash
git add simse-adaptive/src/distance.rs
git commit -m "perf(simse-adaptive): add SIMD-accelerated distance metrics

AVX2+FMA on x86_64, NEON on aarch64, scalar fallback.
Runtime feature detection — no build-time flags needed."
```

---

### Task 3: SoA Vector Storage

Contiguous embedding storage for cache-friendly distance computation.

**Files:**
- Create: `simse-adaptive/src/vector_storage.rs`
- Modify: `simse-adaptive/src/lib.rs` (add module)

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut vs = VectorStorage::new(3);
        vs.insert("a", &[1.0, 2.0, 3.0]);
        assert_eq!(vs.len(), 1);
        assert_eq!(vs.get("a"), Some(&[1.0f32, 2.0, 3.0][..]));
    }

    #[test]
    fn insert_batch_and_get() {
        let mut vs = VectorStorage::new(2);
        vs.insert_batch(&[("a", &[1.0, 2.0]), ("b", &[3.0, 4.0])]);
        assert_eq!(vs.len(), 2);
        assert_eq!(vs.get("a"), Some(&[1.0f32, 2.0][..]));
        assert_eq!(vs.get("b"), Some(&[3.0f32, 4.0][..]));
    }

    #[test]
    fn remove() {
        let mut vs = VectorStorage::new(2);
        vs.insert("a", &[1.0, 2.0]);
        vs.insert("b", &[3.0, 4.0]);
        vs.remove("a");
        assert_eq!(vs.len(), 1);
        assert_eq!(vs.get("a"), None);
        assert_eq!(vs.get("b"), Some(&[3.0f32, 4.0][..]));
    }

    #[test]
    fn iter_embeddings() {
        let mut vs = VectorStorage::new(2);
        vs.insert("a", &[1.0, 2.0]);
        vs.insert("b", &[3.0, 4.0]);
        let all: Vec<_> = vs.iter().collect();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn clear() {
        let mut vs = VectorStorage::new(2);
        vs.insert("a", &[1.0, 2.0]);
        vs.clear();
        assert_eq!(vs.len(), 0);
    }

    #[test]
    fn contiguous_layout() {
        let mut vs = VectorStorage::new(3);
        vs.insert("a", &[1.0, 2.0, 3.0]);
        vs.insert("b", &[4.0, 5.0, 6.0]);
        // Internal buffer should be contiguous [1,2,3,4,5,6]
        assert_eq!(vs.raw_embeddings().len(), 6);
    }
}
```

**Step 2: Implement VectorStorage**

```rust
use std::collections::HashMap;

/// Structure-of-Arrays vector storage for cache-friendly distance computation.
/// Embeddings stored contiguously: [v0_d0, v0_d1, ..., v1_d0, v1_d1, ...]
pub struct VectorStorage {
    ids: Vec<String>,
    embeddings: Vec<f32>,
    id_to_index: HashMap<String, usize>,
    dimensions: usize,
}
```

Methods: `new(dims)`, `insert(id, embedding)`, `insert_batch(entries)`, `remove(id)`, `get(id) -> Option<&[f32]>`, `iter() -> impl Iterator<Item = (&str, &[f32])>`, `len()`, `clear()`, `raw_embeddings() -> &[f32]`, `dimensions()`.

On `remove()`, swap-remove the last entry into the vacated slot to keep embeddings contiguous (O(1) removal).

**Step 3: Run tests**

```bash
cd simse-adaptive && cargo test vector_storage
```

**Step 4: Commit**

```bash
git add simse-adaptive/src/vector_storage.rs simse-adaptive/src/lib.rs
git commit -m "feat(simse-adaptive): add SoA VectorStorage for cache-friendly scans"
```

---

### Task 4: IndexBackend Trait + FlatIndex

**Files:**
- Create: `simse-adaptive/src/index.rs`
- Modify: `simse-adaptive/src/lib.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_flat() -> FlatIndex {
        FlatIndex::new(3)
    }

    #[test]
    fn flat_insert_and_search() {
        let mut idx = make_flat();
        idx.insert("a", &[1.0, 0.0, 0.0]);
        idx.insert("b", &[0.0, 1.0, 0.0]);
        idx.insert("c", &[0.9, 0.1, 0.0]);
        let results = idx.search(&[1.0, 0.0, 0.0], 2, DistanceMetric::Cosine);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "a"); // most similar
    }

    #[test]
    fn flat_remove() {
        let mut idx = make_flat();
        idx.insert("a", &[1.0, 0.0, 0.0]);
        idx.insert("b", &[0.0, 1.0, 0.0]);
        idx.remove("a");
        assert_eq!(idx.len(), 1);
        let results = idx.search(&[1.0, 0.0, 0.0], 10, DistanceMetric::Cosine);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "b");
    }

    #[test]
    fn flat_batch_insert() {
        let mut idx = make_flat();
        idx.insert_batch(&[
            ("a", &[1.0, 0.0, 0.0]),
            ("b", &[0.0, 1.0, 0.0]),
        ]);
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn flat_euclidean_search() {
        let mut idx = make_flat();
        idx.insert("a", &[0.0, 0.0, 0.0]);
        idx.insert("b", &[1.0, 1.0, 1.0]);
        let results = idx.search(&[0.0, 0.0, 0.0], 2, DistanceMetric::Euclidean);
        // "a" should be first (distance 0, similarity 1.0)
        assert_eq!(results[0].0, "a");
    }

    #[test]
    fn flat_empty_search() {
        let idx = make_flat();
        let results = idx.search(&[1.0, 0.0, 0.0], 10, DistanceMetric::Cosine);
        assert!(results.is_empty());
    }
}
```

**Step 2: Implement IndexBackend trait + FlatIndex**

```rust
use crate::distance::DistanceMetric;
use crate::vector_storage::VectorStorage;

/// Result of an index search: (id, similarity_score).
pub type SearchResult = (String, f64);

/// Backend trait for vector indexing strategies.
pub trait IndexBackend: Send + Sync {
    fn insert(&mut self, id: &str, embedding: &[f32]);
    fn insert_batch(&mut self, entries: &[(&str, &[f32])]);
    fn remove(&mut self, id: &str);
    fn search(&self, query: &[f32], k: usize, metric: DistanceMetric) -> Vec<SearchResult>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool { self.len() == 0 }
    fn rebuild(&mut self);
    fn contains(&self, id: &str) -> bool;
}

/// Brute-force flat index using VectorStorage.
pub struct FlatIndex {
    storage: VectorStorage,
}
```

`FlatIndex` wraps `VectorStorage`. Search iterates all embeddings via `storage.iter()`, computes `metric.similarity(query, embedding)`, collects into a BinaryHeap or sorted Vec, returns top-k.

**Step 3: Run tests**

```bash
cd simse-adaptive && cargo test index
```

**Step 4: Commit**

```bash
git add simse-adaptive/src/index.rs simse-adaptive/src/lib.rs
git commit -m "feat(simse-adaptive): add IndexBackend trait and FlatIndex implementation"
```

---

### Task 5: HNSW Index

**Files:**
- Modify: `simse-adaptive/Cargo.toml` (add hnsw_rs dependency)
- Modify: `simse-adaptive/src/index.rs` (add HnswIndex)

**Step 1: Add dependency**

```toml
[dependencies]
hnsw_rs = "0.3"
```

**Step 2: Write failing tests**

```rust
#[test]
fn hnsw_insert_and_search() {
    let mut idx = HnswIndex::new(3, HnswConfig::default());
    idx.insert("a", &[1.0, 0.0, 0.0]);
    idx.insert("b", &[0.0, 1.0, 0.0]);
    idx.insert("c", &[0.9, 0.1, 0.0]);
    let results = idx.search(&[1.0, 0.0, 0.0], 2, DistanceMetric::Cosine);
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, "a");
}

#[test]
fn hnsw_remove_and_search() {
    let mut idx = HnswIndex::new(3, HnswConfig::default());
    idx.insert("a", &[1.0, 0.0, 0.0]);
    idx.insert("b", &[0.0, 1.0, 0.0]);
    idx.remove("a");
    let results = idx.search(&[1.0, 0.0, 0.0], 10, DistanceMetric::Cosine);
    // "a" should not appear
    assert!(results.iter().all(|(id, _)| id != "a"));
}

#[test]
fn hnsw_batch_insert() {
    let mut idx = HnswIndex::new(3, HnswConfig::default());
    idx.insert_batch(&[
        ("a", &[1.0, 0.0, 0.0]),
        ("b", &[0.0, 1.0, 0.0]),
        ("c", &[0.0, 0.0, 1.0]),
    ]);
    assert_eq!(idx.len(), 3);
}

#[test]
fn hnsw_rebuild() {
    let mut idx = HnswIndex::new(3, HnswConfig::default());
    idx.insert("a", &[1.0, 0.0, 0.0]);
    idx.remove("a");
    idx.insert("b", &[0.0, 1.0, 0.0]);
    idx.rebuild(); // should compact removed entries
    assert_eq!(idx.len(), 1);
}
```

**Step 3: Implement HnswIndex**

```rust
pub struct HnswConfig {
    pub m: usize,              // max connections per node (default 16)
    pub ef_construction: usize, // build-time search width (default 200)
    pub ef_search: usize,      // query-time search width (default 50)
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self { m: 16, ef_construction: 200, ef_search: 50 }
    }
}

pub struct HnswIndex {
    config: HnswConfig,
    dimensions: usize,
    // hnsw_rs graph + id mappings
    // removed set for soft-delete
}
```

Wrap `hnsw_rs::Hnsw`. Map string IDs to internal integer indices. `remove()` marks IDs as removed (soft-delete); `rebuild()` reconstructs the graph without removed entries.

**Step 4: Run tests**

```bash
cd simse-adaptive && cargo test index
```

**Step 5: Commit**

```bash
git add simse-adaptive/Cargo.toml simse-adaptive/src/index.rs
git commit -m "feat(simse-adaptive): add HnswIndex wrapping hnsw_rs

Configurable m, ef_construction, ef_search. Soft-delete with rebuild."
```

---

### Task 6: Vector Quantization

**Files:**
- Create: `simse-adaptive/src/quantization.rs`
- Modify: `simse-adaptive/src/lib.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_encode_decode_roundtrip() {
        let v = vec![0.0f32, 0.5, 1.0, -1.0, 0.25];
        let encoded = ScalarQuantizer::fit_encode(&v);
        let decoded = encoded.decode();
        for (a, b) in v.iter().zip(decoded.iter()) {
            assert!((a - b).abs() < 0.01, "a={a} b={b}");
        }
    }

    #[test]
    fn scalar_distance_approximation() {
        let a = vec![1.0f32, 2.0, 3.0, 4.0];
        let b = vec![1.1f32, 2.1, 2.9, 4.2];
        let qa = ScalarQuantizer::fit_encode(&a);
        let qb = ScalarQuantizer::fit_encode(&b);
        let exact_cos = crate::distance::cosine_similarity_score(&a, &b);
        let approx_cos = qa.approximate_cosine(&qb);
        assert!((exact_cos - approx_cos).abs() < 0.05);
    }

    #[test]
    fn binary_encode() {
        let v = vec![1.0f32, -1.0, 0.5, -0.5, 0.0, 1.0, -1.0, 0.0];
        let encoded = BinaryQuantizer::encode(&v);
        // positive dims: 0,2,4,5 -> bits set (0 is sign threshold)
        assert_eq!(encoded.len(), 1); // 8 dims fits in 1 u64
    }

    #[test]
    fn binary_hamming_distance() {
        let a = vec![1.0f32, -1.0, 1.0, -1.0];
        let b = vec![1.0f32, 1.0, 1.0, -1.0]; // differs in dim 1
        let qa = BinaryQuantizer::encode(&a);
        let qb = BinaryQuantizer::encode(&b);
        let dist = BinaryQuantizer::hamming_distance(&qa, &qb);
        assert_eq!(dist, 1);
    }

    #[test]
    fn binary_identical_vectors() {
        let a = vec![1.0f32, -1.0, 0.5];
        let qa = BinaryQuantizer::encode(&a);
        let dist = BinaryQuantizer::hamming_distance(&qa, &qa);
        assert_eq!(dist, 0);
    }

    #[test]
    fn quantization_enum_none() {
        assert!(matches!(Quantization::None, Quantization::None));
    }
}
```

**Step 2: Implement quantization.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Quantization {
    #[default]
    None,
    Scalar,
    Binary,
}

/// Scalar quantization: f32 -> u8 (4x compression).
pub struct ScalarQuantized {
    pub data: Vec<u8>,
    pub min: f32,
    pub scale: f32,
}

pub struct ScalarQuantizer;

impl ScalarQuantizer {
    pub fn fit_encode(values: &[f32]) -> ScalarQuantized { ... }
}

impl ScalarQuantized {
    pub fn decode(&self) -> Vec<f32> { ... }
    pub fn approximate_cosine(&self, other: &ScalarQuantized) -> f64 { ... }
}

/// Binary quantization: f32 -> 1-bit sign (32x compression).
pub struct BinaryQuantizer;

impl BinaryQuantizer {
    pub fn encode(values: &[f32]) -> Vec<u64> { ... }
    pub fn hamming_distance(a: &[u64], b: &[u64]) -> u32 { ... }
}
```

Hamming distance uses `count_ones()` which the compiler maps to `popcnt` on x86 and `vcnt` on ARM.

**Step 3: Run tests**

```bash
cd simse-adaptive && cargo test quantization
```

**Step 4: Commit**

```bash
git add simse-adaptive/src/quantization.rs simse-adaptive/src/lib.rs
git commit -m "feat(simse-adaptive): add scalar and binary vector quantization

Scalar: f32->u8, 4x compression, approximate cosine.
Binary: sign-bit, 32x compression, hamming distance via popcnt."
```

---

### Task 7: MMR + RRF Fusion

**Files:**
- Create: `simse-adaptive/src/fusion.rs`
- Modify: `simse-adaptive/src/lib.rs`

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmr_lambda_one_equals_plain_ranking() {
        let candidates = vec![
            ("a".into(), vec![1.0f32, 0.0], 0.9),
            ("b".into(), vec![0.0, 1.0], 0.5),
            ("c".into(), vec![0.8, 0.2], 0.85),
        ];
        let query = vec![1.0f32, 0.0];
        let result = mmr_rerank(&candidates, &query, 3, 1.0, DistanceMetric::Cosine);
        // lambda=1 means pure relevance, order should be a, c, b
        assert_eq!(result[0].0, "a");
        assert_eq!(result[1].0, "c");
    }

    #[test]
    fn mmr_lambda_zero_maximizes_diversity() {
        let candidates = vec![
            ("a".into(), vec![1.0f32, 0.0], 0.9),
            ("c".into(), vec![0.95, 0.05], 0.89), // very similar to a
            ("b".into(), vec![0.0, 1.0], 0.5),     // very different from a
        ];
        let query = vec![1.0f32, 0.0];
        let result = mmr_rerank(&candidates, &query, 3, 0.0, DistanceMetric::Cosine);
        // After selecting "a" first (arbitrary tie-break), lambda=0 should pick
        // the most diverse next: "b" (orthogonal to a)
        assert_eq!(result[0].0, "a");
        assert_eq!(result[1].0, "b");
    }

    #[test]
    fn rrf_basic_ranking() {
        let list_a = vec![("x".into(), 1), ("y".into(), 2), ("z".into(), 3)];
        let list_b = vec![("y".into(), 1), ("x".into(), 2), ("z".into(), 3)];
        let result = reciprocal_rank_fusion(&[&list_a, &list_b], 60);
        // "y" ranks 2+1=3 combined, "x" ranks 1+2=3, tie-break doesn't matter
        // Both x and y should be above z
        let z_pos = result.iter().position(|(id, _)| id == "z").unwrap();
        assert_eq!(z_pos, 2);
    }

    #[test]
    fn rrf_single_list() {
        let list = vec![("a".into(), 1), ("b".into(), 2)];
        let result = reciprocal_rank_fusion(&[&list], 60);
        assert_eq!(result[0].0, "a");
        assert_eq!(result[1].0, "b");
    }

    #[test]
    fn rrf_empty() {
        let result: Vec<(String, f64)> = reciprocal_rank_fusion(&[], 60);
        assert!(result.is_empty());
    }
}
```

**Step 2: Implement fusion.rs**

```rust
use crate::distance::DistanceMetric;

/// Maximal Marginal Relevance reranking.
/// candidates: (id, embedding, relevance_score)
/// Returns reranked (id, mmr_score) list.
pub fn mmr_rerank(
    candidates: &[(String, Vec<f32>, f64)],
    query: &[f32],
    k: usize,
    lambda: f64,
    metric: DistanceMetric,
) -> Vec<(String, f64)> { ... }

/// Reciprocal Rank Fusion.
/// lists: each list is [(id, rank)] where rank is 1-indexed.
/// k: RRF parameter (default 60).
pub fn reciprocal_rank_fusion(
    lists: &[&[(String, usize)]],
    k: usize,
) -> Vec<(String, f64)> { ... }
```

**Step 3: Run tests**

```bash
cd simse-adaptive && cargo test fusion
```

**Step 4: Commit**

```bash
git add simse-adaptive/src/fusion.rs simse-adaptive/src/lib.rs
git commit -m "feat(simse-adaptive): add MMR reranking and RRF hybrid fusion"
```

---

### Task 8: Add Rayon Parallelism

**Files:**
- Modify: `simse-adaptive/Cargo.toml` (add rayon)
- Modify: `simse-adaptive/src/index.rs` (parallel flat search)
- Modify: `simse-adaptive/src/deduplication.rs` (parallel pairwise)

**Step 1: Add dependency**

```toml
[dependencies]
rayon = "1"
```

**Step 2: Parallelize FlatIndex search**

In `FlatIndex::search()`, replace the sequential iterator with Rayon's `par_chunks`:

```rust
use rayon::prelude::*;

// In FlatIndex::search():
let chunk_size = (self.storage.len() / rayon::current_num_threads()).max(64);
let results: Vec<SearchResult> = self.storage
    .par_iter_chunks(chunk_size)
    .flat_map(|chunk| {
        chunk.iter()
            .map(|(id, emb)| (id.to_string(), metric.similarity(query, emb)))
            .collect::<Vec<_>>()
    })
    .collect();
```

Note: `VectorStorage` needs a `par_iter_chunks()` method or we iterate in parallel over index ranges.

**Step 3: Parallelize deduplication**

In `find_duplicate_volumes`, parallelize the inner similarity comparisons using Rayon for the O(N²) pairwise scan.

**Step 4: Run tests**

```bash
cd simse-adaptive && cargo test
```

All existing tests must still pass — Rayon should not change results, only speed.

**Step 5: Commit**

```bash
git add simse-adaptive/Cargo.toml simse-adaptive/src/index.rs simse-adaptive/src/deduplication.rs
git commit -m "perf(simse-adaptive): add Rayon parallelism for flat search and deduplication"
```

---

### Task 9: Store Integration

Wire IndexBackend, DistanceMetric, Quantization, and MMR into the existing Store.

**Files:**
- Modify: `simse-adaptive/src/store.rs`
- Modify: `simse-adaptive/src/types.rs` (add new config/param fields)

**Step 1: Extend StoreConfig**

```rust
pub struct StoreConfig {
    // ... existing fields ...
    pub index_strategy: IndexStrategy,
    pub quantization: Quantization,
    pub hnsw_config: HnswConfig,
    pub default_metric: DistanceMetric,
    pub auto_index_threshold: usize, // default 1000
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum IndexStrategy {
    #[default]
    Auto,
    Flat,
    Hnsw,
}
```

**Step 2: Add IndexBackend to Store**

Add a `Box<dyn IndexBackend>` field to `Store`. On `add()`, insert into both `volumes` and the index. On `search()`, delegate to the index backend instead of the inline loop. On `delete()`, remove from the index.

The `search()` method gains an optional `metric` parameter (defaults to `self.config.default_metric`).

**Step 3: Add MMR support to search methods**

Search and recommend methods gain an optional `mmr_lambda: Option<f64>` parameter. When set, results are reranked through `fusion::mmr_rerank()` before returning.

**Step 4: Auto-selection logic**

On `initialize` with `IndexStrategy::Auto`:
- If loading from disk with >threshold entries, use HNSW
- On `add()`, if entry count crosses threshold, rebuild with HNSW

**Step 5: Run tests**

```bash
cd simse-adaptive && cargo test
```

All existing integration tests must pass — the default behavior (Cosine, Flat, no quantization, no MMR) is unchanged.

**Step 6: Commit**

```bash
git add simse-adaptive/src/store.rs simse-adaptive/src/types.rs
git commit -m "feat(simse-adaptive): wire IndexBackend, metrics, quantization, MMR into Store

Auto index selection (Flat <1K, HNSW >1K). Optional metric and mmrLambda
on search methods. Quantization configurable per-store."
```

---

### Task 10: Server Integration — New JSON-RPC Methods

**Files:**
- Modify: `simse-adaptive/src/server.rs`
- Modify: `simse-adaptive/src/protocol.rs`

**Step 1: Add protocol types**

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetIndexStrategyParams {
    pub strategy: IndexStrategy,
    pub hnsw_config: Option<HnswConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetQuantizationParams {
    pub quantization: Quantization,
}

// Extend SearchParams with:
pub struct SearchParams {
    // ... existing fields ...
    pub metric: Option<DistanceMetric>,
    pub mmr_lambda: Option<f64>,
}
```

**Step 2: Add dispatch entries in server.rs**

```rust
"store/setIndexStrategy" => self.with_state_transition(|s| handle_set_index_strategy(s, req.params)),
"store/setQuantization" => self.with_state_transition(|s| handle_set_quantization(s, req.params)),
"store/getIndexStats" => self.with_state(|s| handle_get_index_stats(s)),
```

**Step 3: Implement handlers**

- `handle_set_index_strategy`: Parse params, call `store.set_index_strategy()`, triggers rebuild
- `handle_set_quantization`: Parse params, call `store.set_quantization()`, triggers rebuild
- `handle_get_index_stats`: Return JSON with index type, vector count, dimensions, memory estimate

**Step 4: Update search handler**

Pass `metric` and `mmr_lambda` from `SearchParams` through to `store.search()`.

**Step 5: Run tests**

```bash
cd simse-adaptive && cargo test
```

**Step 6: Commit**

```bash
git add simse-adaptive/src/server.rs simse-adaptive/src/protocol.rs
git commit -m "feat(simse-adaptive): add setIndexStrategy, setQuantization, getIndexStats JSON-RPC methods"
```

---

### Task 11: Integration Tests

**Files:**
- Modify: `simse-adaptive/tests/integration.rs`

**Step 1: Add integration tests**

```rust
#[test]
fn test_index_strategy_switch() {
    let mut proc = VectorProcess::spawn();
    proc.call("store/initialize", json!({}));

    // Add entries and verify flat search works
    for i in 0..5 {
        proc.call("store/add", json!({
            "text": format!("entry {i}"),
            "embedding": vec![i as f64 * 0.1; 8]
        }));
    }
    let result = proc.call("store/search", json!({
        "queryEmbedding": vec![0.1; 8],
        "maxResults": 3
    }));
    assert_eq!(result["results"].as_array().unwrap().len(), 3);

    // Switch to HNSW
    proc.call("store/setIndexStrategy", json!({ "strategy": "hnsw" }));

    // Search again — should still return results
    let result = proc.call("store/search", json!({
        "queryEmbedding": vec![0.1; 8],
        "maxResults": 3
    }));
    assert_eq!(result["results"].as_array().unwrap().len(), 3);
}

#[test]
fn test_distance_metrics() {
    let mut proc = VectorProcess::spawn();
    proc.call("store/initialize", json!({}));

    proc.call("store/add", json!({
        "text": "close",
        "embedding": vec![1.0, 0.0, 0.0]
    }));
    proc.call("store/add", json!({
        "text": "far",
        "embedding": vec![0.0, 1.0, 0.0]
    }));

    // Cosine search
    let cosine = proc.call("store/search", json!({
        "queryEmbedding": vec![1.0, 0.0, 0.0],
        "maxResults": 2,
        "metric": "cosine"
    }));

    // Euclidean search
    let euclidean = proc.call("store/search", json!({
        "queryEmbedding": vec![1.0, 0.0, 0.0],
        "maxResults": 2,
        "metric": "euclidean"
    }));

    // Both should rank "close" first
    assert!(cosine["results"][0]["score"].as_f64().unwrap() > 0.9);
    assert!(euclidean["results"][0]["score"].as_f64().unwrap() > 0.5);
}

#[test]
fn test_mmr_diversity() {
    let mut proc = VectorProcess::spawn();
    proc.call("store/initialize", json!({}));

    // Add vectors: a, c similar; b different
    proc.call("store/add", json!({ "text": "a", "embedding": vec![1.0, 0.0, 0.0] }));
    proc.call("store/add", json!({ "text": "b", "embedding": vec![0.0, 1.0, 0.0] }));
    proc.call("store/add", json!({ "text": "c", "embedding": vec![0.95, 0.05, 0.0] }));

    // Without MMR — a and c should be top 2
    let no_mmr = proc.call("store/search", json!({
        "queryEmbedding": vec![1.0, 0.0, 0.0],
        "maxResults": 3
    }));

    // With MMR lambda=0.3 — should promote b higher
    let with_mmr = proc.call("store/search", json!({
        "queryEmbedding": vec![1.0, 0.0, 0.0],
        "maxResults": 3,
        "mmrLambda": 0.3
    }));

    // b should be ranked higher with MMR than without
    let b_rank_no_mmr = no_mmr["results"].as_array().unwrap()
        .iter().position(|r| r["entry"]["text"] == "b").unwrap();
    let b_rank_mmr = with_mmr["results"].as_array().unwrap()
        .iter().position(|r| r["entry"]["text"] == "b").unwrap();
    assert!(b_rank_mmr < b_rank_no_mmr, "MMR should promote diverse result b");
}

#[test]
fn test_get_index_stats() {
    let mut proc = VectorProcess::spawn();
    proc.call("store/initialize", json!({}));
    proc.call("store/add", json!({ "text": "x", "embedding": vec![1.0, 2.0, 3.0] }));

    let stats = proc.call("store/getIndexStats", json!({}));
    assert_eq!(stats["vectorCount"].as_u64().unwrap(), 1);
    assert_eq!(stats["dimensions"].as_u64().unwrap(), 3);
    assert!(stats["indexType"].as_str().is_some());
}

#[test]
fn test_quantization_search() {
    let mut proc = VectorProcess::spawn();
    proc.call("store/initialize", json!({}));

    // Add entries
    for i in 0..20 {
        let emb: Vec<f64> = (0..32).map(|d| ((i * 32 + d) as f64) * 0.01).collect();
        proc.call("store/add", json!({ "text": format!("entry {i}"), "embedding": emb }));
    }

    // Search without quantization
    let baseline = proc.call("store/search", json!({
        "queryEmbedding": vec![0.5; 32],
        "maxResults": 5
    }));

    // Enable scalar quantization
    proc.call("store/setQuantization", json!({ "quantization": "scalar" }));

    // Search with quantization — top result should match
    let quantized = proc.call("store/search", json!({
        "queryEmbedding": vec![0.5; 32],
        "maxResults": 5
    }));

    let base_top = baseline["results"][0]["entry"]["id"].as_str().unwrap();
    let quant_top = quantized["results"][0]["entry"]["id"].as_str().unwrap();
    assert_eq!(base_top, quant_top, "Quantized search should return same top result");
}
```

**Step 2: Run integration tests**

```bash
cd simse-adaptive && cargo test --test integration
```

**Step 3: Commit**

```bash
git add simse-adaptive/tests/integration.rs
git commit -m "test(simse-adaptive): add integration tests for indexing, metrics, MMR, quantization"
```

---

### Task 12: Criterion Benchmarks

**Files:**
- Create: `simse-adaptive/benches/index_benchmarks.rs`
- Modify: `simse-adaptive/Cargo.toml` (add bench entry)

**Step 1: Add bench entry to Cargo.toml**

```toml
[[bench]]
name = "index_benchmarks"
harness = false
```

**Step 2: Write benchmarks**

```rust
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use simse_adaptive_engine::distance::*;
use simse_adaptive_engine::index::*;
use simse_adaptive_engine::quantization::*;
use rand::Rng;

fn random_vectors(n: usize, dims: usize) -> Vec<Vec<f32>> {
    let mut rng = rand::rng();
    (0..n).map(|_| (0..dims).map(|_| rng.random::<f32>()).collect()).collect()
}

fn bench_distance_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("distance");
    for dims in [128, 384, 768, 1536] {
        let a: Vec<f32> = (0..dims).map(|i| i as f32 * 0.001).collect();
        let b: Vec<f32> = (0..dims).map(|i| (dims - i) as f32 * 0.001).collect();

        group.bench_with_input(BenchmarkId::new("cosine_scalar", dims), &dims, |bench, _| {
            bench.iter(|| cosine_similarity_score(&a, &b));
        });
        group.bench_with_input(BenchmarkId::new("cosine_simd", dims), &dims, |bench, _| {
            bench.iter(|| simd_cosine_similarity(&a, &b));
        });
        group.bench_with_input(BenchmarkId::new("euclidean", dims), &dims, |bench, _| {
            bench.iter(|| simd_euclidean_distance(&a, &b));
        });
        group.bench_with_input(BenchmarkId::new("dot_product", dims), &dims, |bench, _| {
            bench.iter(|| simd_dot_product(&a, &b));
        });
    }
    group.finish();
}

fn bench_flat_vs_hnsw(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    let dims = 384;

    for n in [1_000, 10_000, 50_000] {
        let vectors = random_vectors(n, dims);
        let query: Vec<f32> = (0..dims).map(|i| i as f32 * 0.001).collect();

        // Flat index
        let mut flat = FlatIndex::new(dims);
        for (i, v) in vectors.iter().enumerate() {
            flat.insert(&format!("v{i}"), v);
        }
        group.bench_with_input(BenchmarkId::new("flat", n), &n, |bench, _| {
            bench.iter(|| flat.search(&query, 10, DistanceMetric::Cosine));
        });

        // HNSW index
        let mut hnsw = HnswIndex::new(dims, HnswConfig::default());
        for (i, v) in vectors.iter().enumerate() {
            hnsw.insert(&format!("v{i}"), v);
        }
        group.bench_with_input(BenchmarkId::new("hnsw", n), &n, |bench, _| {
            bench.iter(|| hnsw.search(&query, 10, DistanceMetric::Cosine));
        });
    }
    group.finish();
}

fn bench_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization");
    let dims = 384;
    let v: Vec<f32> = (0..dims).map(|i| (i as f32) * 0.01 - 1.92).collect();

    group.bench_function("scalar_encode", |bench| {
        bench.iter(|| ScalarQuantizer::fit_encode(&v));
    });
    group.bench_function("binary_encode", |bench| {
        bench.iter(|| BinaryQuantizer::encode(&v));
    });
    group.finish();
}

criterion_group!(benches, bench_distance_metrics, bench_flat_vs_hnsw, bench_quantization);
criterion_main!(benches);
```

**Step 3: Run benchmarks**

```bash
cd simse-adaptive && cargo bench --bench index_benchmarks
```

**Step 4: Commit**

```bash
git add simse-adaptive/benches/index_benchmarks.rs simse-adaptive/Cargo.toml
git commit -m "bench(simse-adaptive): add criterion benchmarks for distance, indexing, quantization"
```

---

### Task 13: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

Update the simse-adaptive section to document:
- New modules: distance, index, quantization, fusion, vector_storage
- New dependencies: rayon, hnsw_rs
- New JSON-RPC methods: setIndexStrategy, setQuantization, getIndexStats
- Extended search params: metric, mmrLambda

**Step 1: Update CLAUDE.md**

Add to the simse-adaptive module listing.

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with simse-adaptive hardening modules"
```

---

## Task Summary

| Task | Description | Est. Lines |
|------|-------------|-----------|
| 1 | Distance module (scalar, replaces cosine.rs) | ~200 |
| 2 | SIMD acceleration (AVX2, NEON) | ~300 |
| 3 | SoA VectorStorage | ~150 |
| 4 | IndexBackend trait + FlatIndex | ~250 |
| 5 | HnswIndex (wraps hnsw_rs) | ~250 |
| 6 | Scalar + Binary quantization | ~300 |
| 7 | MMR + RRF fusion | ~200 |
| 8 | Rayon parallelism | ~100 |
| 9 | Store integration | ~300 |
| 10 | Server integration (3 new methods) | ~150 |
| 11 | Integration tests | ~200 |
| 12 | Criterion benchmarks | ~100 |
| 13 | CLAUDE.md update | ~20 |

**Total: ~2,500 lines of new/modified code across 13 tasks.**

Dependencies between tasks:
- Task 1 must complete before Tasks 2, 4, 7, 9
- Task 3 must complete before Task 4
- Task 4 must complete before Tasks 5, 8, 9
- Task 6 must complete before Task 9
- Task 7 must complete before Task 9
- Task 9 must complete before Task 10
- Task 10 must complete before Task 11
- All tasks before Task 13
