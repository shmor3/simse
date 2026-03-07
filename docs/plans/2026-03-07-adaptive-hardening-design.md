# simse-adaptive Hardening Design

**Date:** 2026-03-07
**Goal:** Make simse-adaptive as solid as (or better than) ruvector — adding HNSW indexing, SIMD distance computation, vector quantization, parallelism, and advanced search features while preserving all existing unique capabilities (PCN, learning, graphs, topics, recommendations).

**Approach:** Lean on proven crates for hard algorithms (HNSW, Rayon), build everything else ourselves.

---

## 1. Tiered Index Architecture

An `IndexBackend` trait abstracts vector indexing, with two implementations:

```rust
trait IndexBackend: Send + Sync {
    fn insert(&mut self, id: &str, embedding: &[f32]);
    fn insert_batch(&mut self, entries: &[(&str, &[f32])]);
    fn remove(&mut self, id: &str);
    fn search(&self, query: &[f32], k: usize, metric: DistanceMetric) -> Vec<(String, f32)>;
    fn len(&self) -> usize;
    fn rebuild(&mut self);
}
```

- **FlatIndex** — O(N) brute force, optimal for <=1K vectors. Enhanced with SIMD distance and SoA memory layout.
- **HnswIndex** — Approximate NN via `hnsw_rs`. Configurable `m`, `ef_construction`, `ef_search`. Optimal for 1K+ vectors.

**Auto-selection:** Defaults to Flat for <=1K entries, HNSW for >1K. Switchable at runtime via `store/setIndexStrategy` (triggers rebuild).

---

## 2. Distance Metrics & SIMD

Four distance metrics:

```rust
enum DistanceMetric {
    Cosine,      // 1 - cos(a, b), default
    Euclidean,   // L2 distance
    DotProduct,  // negative dot product
    Manhattan,   // L1 distance
}
```

Each metric has:
- **Scalar fallback** — pure Rust, always available
- **SIMD accelerated** — via `std::arch` intrinsics with runtime feature detection

SIMD targets:
- x86_64: AVX2 + FMA (8 floats/iter), SSE4.1 fallback (4 floats/iter)
- aarch64: NEON (4 floats/iter)

Runtime dispatch via `#[target_feature]` and `is_x86_feature_detected!()`. No external SIMD crate — `std::arch` is stable Rust.

Replaces existing `cosine.rs` with unified `distance.rs` module. Magnitude cache extends to support all metrics.

---

## 3. Vector Quantization

Two quantization schemes, selected per-store:

```rust
enum Quantization {
    None,     // Full f32 (default)
    Scalar,   // f32 -> u8 (4x compression)
    Binary,   // f32 -> 1-bit sign (32x compression)
}
```

**Scalar (uint8):** `(val - min) / (max - min) * 255`. Per-vector min/scale stored for decode. SIMD distance on u8 values.

**Binary:** Sign-bit quantization packed into u64. Hamming distance via `popcnt`. 32x compression, ideal for fast candidate pre-filtering before exact reranking.

**Integration:**
- `QuantizedStore` sits alongside raw f32 embeddings
- Quantized distance for candidate retrieval, exact f32 reranking for top results
- Raw embeddings always kept (persistence, PCN, exact rerank)
- Quantization is runtime-only — persistence format unchanged

No Product Quantization for now (PQ requires codebook training). Scalar + Binary cover the practical sweet spot.

---

## 4. Batch Parallelism & Memory Layout

**Rayon parallelism:**
- Flat index search: `par_chunks` over vectors, merge top-k
- Batch insert: parallel quantization + index updates
- Deduplication: parallelize O(N^2) pairwise comparison
- Batch distance: one query against N vectors in parallel

Rayon opt-in via `parallel` feature flag (default on).

**SoA memory layout:**

```rust
struct VectorStorage {
    ids: Vec<String>,
    embeddings: Vec<f32>,  // contiguous: [v0_d0, v0_d1, ..., v1_d0, ...]
    dimensions: usize,
}
```

Cache-friendly sequential access during scans. `Entry` struct stays for metadata/text — `VectorStorage` is the hot-path companion.

---

## 5. MMR & Advanced Search

**Maximal Marginal Relevance:**

```
MMR(d) = lambda * sim(query, d) - (1 - lambda) * max(sim(d, d_selected))
```

- `lambda = 1.0` = pure relevance (current behavior)
- Configurable per-query via `mmrLambda` parameter

**Reciprocal Rank Fusion (RRF):**

```
score = sum(1 / (k + rank_i))
```

Replaces weighted sum as default hybrid fusion. More robust to score scale differences. Weighted sum kept as option (`fusionMethod: "weighted" | "rrf"`).

---

## 6. New JSON-RPC Surface

New methods:

| Method | Description |
|--------|-------------|
| `store/setIndexStrategy` | Switch between Flat/HNSW at runtime |
| `store/setQuantization` | Change quantization (triggers rebuild) |
| `store/getIndexStats` | Index type, vector count, memory usage, dimensions |

Extended parameters on existing methods:

- `metric` (default `"cosine"`) on all search methods
- `mmrLambda` (default `null` = disabled) on search/advancedSearch/recommend

---

## 7. Module Structure

**New files:**

```
simse-adaptive/src/
  distance.rs        ~400 lines  # DistanceMetric, SIMD + scalar
  quantization.rs    ~300 lines  # Scalar + Binary quantization
  index.rs           ~500 lines  # IndexBackend trait, FlatIndex, HnswIndex
  fusion.rs          ~250 lines  # MMR reranking, RRF fusion
  vector_storage.rs  ~150 lines  # SoA contiguous storage
```

**Modified files:**

```
store.rs       # IndexBackend dispatch replaces direct scan
cosine.rs      # Deleted, replaced by distance.rs
server.rs      # 3 new methods, extended params
protocol.rs    # New request/response types
lib.rs         # Module declarations
Cargo.toml     # Add rayon, hnsw_rs
```

**New dependencies:**

| Crate | Purpose |
|-------|---------|
| `rayon` | Parallel batch operations |
| `hnsw_rs` | HNSW index |

---

## 8. Testing & Benchmarks

**Unit tests per module:**
- `distance.rs` — 4 metrics, SIMD vs scalar equivalence, edge cases
- `quantization.rs` — encode/decode roundtrip, hamming correctness
- `index.rs` — FlatIndex + HnswIndex both satisfy IndexBackend contract
- `fusion.rs` — MMR correctness, RRF ranking

**Integration tests (JSON-RPC):**
- Auto-selection (Flat vs HNSW by size)
- Runtime strategy switch with result equivalence
- Quantized search recall vs unquantized
- Distance metric variations
- MMR diversity increases as lambda decreases
- Batch parallelism correctness

**Criterion benchmarks:**
- Flat vs HNSW at 1K/10K/50K/100K vectors (384 dims)
- SIMD vs scalar at 128/384/768/1536 dims
- Quantized vs unquantized (recall + latency)
- Batch insert throughput

---

## 9. What Stays Unchanged

- PCN (predictive coding network)
- Learning engine (per-user adaptive weights)
- Graph index (directed weighted edges)
- Topic catalog (hierarchical, fuzzy resolution)
- Deduplication logic (algorithm unchanged, just parallelized)
- Recommendation scoring (multi-signal)
- Text search / BM25
- Persistence format (raw f32 always saved)
- JSON-RPC transport / NDJSON stdio
