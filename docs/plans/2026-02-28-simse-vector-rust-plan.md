# simse-vector Rust Engine Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor simse-vector from pure TypeScript to a Rust subprocess engine, moving all storage, search, and computation to Rust while keeping LLM-dependent modules in TS.

**Architecture:** Single Rust binary (`simse-vector-engine`) communicates with TS via JSON-RPC 2.0 over NDJSON stdio. Rust owns in-memory volume storage, all indexes, file persistence (gzip v2), text search, cosine similarity, deduplication, recommendation, patron learning, and topic catalog. TS obtains embeddings from external providers and passes pre-computed vectors to Rust. LLM-dependent modules (librarian, librarian-registry, circulation-desk, library-services) stay in TS.

**Tech Stack:** Rust 2021 edition, serde/serde_json, base64, regex, flate2 (gzip), thiserror, tracing. TypeScript with Bun runtime. Biome for formatting/linting.

**Reference pattern:** `simse-vfs/engine/` (Rust) + `simse-vfs/src/client.ts` (TS JSON-RPC client)

---

## Context: Key Patterns

Before implementing, understand these patterns used throughout simse:

1. **Factory functions, not classes:** Every module exports `createXxx()` returning a frozen readonly interface
2. **ESM imports with `.js` extensions:** `import { foo } from './bar.js'`
3. **`import type` for type-only imports:** enforced by `verbatimModuleSyntax`
4. **Biome formatting:** tabs, single quotes, semicolons, organized imports
5. **Error hierarchy:** `createVectorError()` base, domain-specific factories, duck-typed guards on `code` field
6. **VFS engine reference:** `simse-vfs/engine/` has the exact Rust structure to follow (main.rs, lib.rs, server.rs, transport.rs, protocol.rs, error.rs, etc.)

---

### Task 1: Rust Crate Scaffold

**Files:**
- Create: `simse-vector/engine/Cargo.toml`
- Create: `simse-vector/engine/src/main.rs`
- Create: `simse-vector/engine/src/lib.rs`
- Create: `simse-vector/engine/src/error.rs`
- Create: `simse-vector/engine/src/types.rs`
- Create: `simse-vector/engine/src/transport.rs`
- Create: `simse-vector/engine/src/protocol.rs`

**What to build:**

The Cargo.toml should be modeled on `simse-vfs/engine/Cargo.toml`:

```toml
[package]
name = "simse-vector-engine"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Vector store engine over JSON-RPC 2.0 / NDJSON stdio"

[lib]
name = "simse_vector_engine"
path = "src/lib.rs"

[[bin]]
name = "simse-vector-engine"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"
thiserror = "2"
base64 = "0.22"
flate2 = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
```

**types.rs** — Core data structures:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub id: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub metadata: HashMap<String, String>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lookup {
    pub volume: Volume,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextLookup {
    pub volume: Volume,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedLookup {
    pub volume: Volume,
    pub score: f64,
    pub scores: ScoreBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub vector: Option<f64>,
    pub text: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateCheckResult {
    #[serde(rename = "isDuplicate")]
    pub is_duplicate: bool,
    #[serde(rename = "existingVolume")]
    pub existing_volume: Option<Volume>,
    pub similarity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateVolumes {
    pub representative: Volume,
    pub duplicates: Vec<Volume>,
    #[serde(rename = "averageSimilarity")]
    pub average_similarity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicInfo {
    pub topic: String,
    #[serde(rename = "entryCount")]
    pub entry_count: usize,
    #[serde(rename = "entryIds")]
    pub entry_ids: Vec<String>,
    pub parent: Option<String>,
    pub children: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicCatalogSection {
    pub topic: String,
    pub parent: Option<String>,
    pub children: Vec<String>,
    #[serde(rename = "volumeCount")]
    pub volume_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub volume: Volume,
    pub score: f64,
    pub scores: RecommendationScores,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationScores {
    pub vector: Option<f64>,
    pub recency: Option<f64>,
    pub frequency: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightProfile {
    pub vector: Option<f64>,
    pub recency: Option<f64>,
    pub frequency: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataFilter {
    pub key: String,
    pub value: Option<serde_json::Value>, // string or string[]
    pub mode: Option<String>,             // "eq", "neq", "contains", etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub after: Option<u64>,
    pub before: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatronProfile {
    #[serde(rename = "queryHistory")]
    pub query_history: Vec<QueryRecord>,
    #[serde(rename = "adaptedWeights")]
    pub adapted_weights: RequiredWeightProfile,
    #[serde(rename = "interestEmbedding")]
    pub interest_embedding: Option<Vec<f32>>,
    #[serde(rename = "totalQueries")]
    pub total_queries: usize,
    #[serde(rename = "lastUpdated")]
    pub last_updated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryRecord {
    pub embedding: Vec<f32>,
    pub timestamp: u64,
    #[serde(rename = "resultCount")]
    pub result_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredWeightProfile {
    pub vector: f64,
    pub recency: f64,
    pub frequency: f64,
}

// Search options for advanced search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    #[serde(rename = "queryEmbedding")]
    pub query_embedding: Option<Vec<f32>>,
    #[serde(rename = "similarityThreshold")]
    pub similarity_threshold: Option<f64>,
    pub text: Option<TextSearchOptions>,
    pub metadata: Option<Vec<MetadataFilter>>,
    #[serde(rename = "dateRange")]
    pub date_range: Option<DateRange>,
    #[serde(rename = "maxResults")]
    pub max_results: Option<usize>,
    #[serde(rename = "rankBy")]
    pub rank_by: Option<String>,
    #[serde(rename = "fieldBoosts")]
    pub field_boosts: Option<FieldBoosts>,
    #[serde(rename = "rankWeights")]
    pub rank_weights: Option<RankWeights>,
    #[serde(rename = "topicFilter")]
    pub topic_filter: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextSearchOptions {
    pub query: String,
    pub mode: Option<String>,
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldBoosts {
    pub text: Option<f64>,
    pub metadata: Option<f64>,
    pub topic: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankWeights {
    pub vector: Option<f64>,
    pub text: Option<f64>,
    pub metadata: Option<f64>,
    pub recency: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendOptions {
    #[serde(rename = "queryEmbedding")]
    pub query_embedding: Option<Vec<f32>>,
    pub weights: Option<WeightProfile>,
    #[serde(rename = "maxResults")]
    pub max_results: Option<usize>,
    #[serde(rename = "minScore")]
    pub min_score: Option<f64>,
    pub metadata: Option<Vec<MetadataFilter>>,
    pub topics: Option<Vec<String>>,
    #[serde(rename = "dateRange")]
    pub date_range: Option<DateRange>,
}
```

**error.rs** — modeled on `simse-vfs/engine/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VectorError {
    #[error("Store not initialized: call store/initialize first")]
    NotInitialized,
    #[error("Empty text: cannot add empty text")]
    EmptyText,
    #[error("Empty embedding: cannot add volume with empty embedding")]
    EmptyEmbedding,
    #[error("Volume not found: {0}")]
    NotFound(String),
    #[error("Duplicate detected: similarity {0:.4}")]
    Duplicate(f64),
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Storage corruption: {0}")]
    Corruption(String),
}

impl VectorError {
    pub fn code(&self) -> &str {
        match self {
            Self::NotInitialized => "STACKS_NOT_LOADED",
            Self::EmptyText => "STACKS_EMPTY_TEXT",
            Self::EmptyEmbedding => "STACKS_EMPTY_EMBEDDING",
            Self::NotFound(_) => "MEMORY_ENTRY_NOT_FOUND",
            Self::Duplicate(_) => "STACKS_DUPLICATE",
            Self::InvalidRegex(_) => "STACKS_INVALID_REGEX",
            Self::Io(_) => "STACKS_IO",
            Self::Serialization(_) => "STACKS_SERIALIZATION",
            Self::Corruption(_) => "STACKS_CORRUPT",
        }
    }
}
```

**transport.rs** and **protocol.rs** — copy from `simse-vfs/engine/src/transport.rs` and `simse-vfs/engine/src/protocol.rs` respectively. They are generic JSON-RPC infrastructure.

**main.rs:**

```rust
use simse_vector_engine::server::VectorServer;
use simse_vector_engine::transport::NdjsonTransport;

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let transport = NdjsonTransport::new();
    let mut server = VectorServer::new(transport);

    tracing::info!("simse-vector-engine ready");

    if let Err(e) = server.run() {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
```

**lib.rs:**

```rust
pub mod error;
pub mod types;
pub mod transport;
pub mod protocol;
pub mod cosine;
pub mod text_search;
pub mod inverted_index;
pub mod cataloging;
pub mod deduplication;
pub mod recommendation;
pub mod learning;
pub mod topic_catalog;
pub mod query_dsl;
pub mod text_cache;
pub mod persistence;
pub mod prompt_injection;
pub mod store;
pub mod server;
```

**Step 1:** Create the directory and all files listed above.

**Step 2:** Run `cd simse-vector/engine && cargo check` — should compile with no errors (server module will be a stub at this point).

**Step 3:** Commit: `feat(vector-engine): scaffold Rust crate with types, errors, transport, protocol`

---

### Task 2: Cosine Similarity + Magnitude Cache

**Files:**
- Create: `simse-vector/engine/src/cosine.rs`
- Create: `simse-vector/engine/src/cataloging.rs` (magnitude cache portion only)

**What to build:**

Port `simse-vector/src/cosine.ts` (32 lines) to Rust. Uses `f32` vectors for memory efficiency (embeddings are f32 precision anyway). Also implement the magnitude cache from `cataloging.ts`.

**cosine.rs:**

```rust
/// Compute cosine similarity between two f32 vectors.
/// Returns 0.0 for zero-magnitude vectors or dimension mismatches.
/// Result clamped to [-1.0, 1.0].
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot: f64 = 0.0;
    let mut norm_a: f64 = 0.0;
    let mut norm_b: f64 = 0.0;

    for i in 0..a.len() {
        let ai = a[i] as f64;
        let bi = b[i] as f64;
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        return 0.0;
    }

    let result = dot / denom;
    if !result.is_finite() {
        return 0.0;
    }
    result.clamp(-1.0, 1.0)
}

/// Compute the magnitude (L2 norm) of a vector.
pub fn compute_magnitude(embedding: &[f32]) -> f64 {
    let mut sum: f64 = 0.0;
    for &v in embedding {
        let vf = v as f64;
        sum += vf * vf;
    }
    sum.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_vectors() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-10);
    }

    #[test]
    fn orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-10);
    }

    #[test]
    fn opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-10);
    }

    #[test]
    fn empty_vectors() {
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }

    #[test]
    fn mismatched_lengths() {
        assert_eq!(cosine_similarity(&[1.0], &[1.0, 2.0]), 0.0);
    }

    #[test]
    fn zero_magnitude() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 2.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn magnitude_basic() {
        let v = vec![3.0, 4.0];
        assert!((compute_magnitude(&v) - 5.0).abs() < 1e-10);
    }
}
```

**Step 1:** Create `cosine.rs` with tests.

**Step 2:** Run `cargo test cosine` — all 7 tests should pass.

**Step 3:** Commit: `feat(vector-engine): cosine similarity and magnitude computation`

---

### Task 3: Text Search (Levenshtein, N-gram, Fuzzy, Token)

**Files:**
- Create: `simse-vector/engine/src/text_search.rs`

**What to build:**

Port `simse-vector/src/text-search.ts` (389 lines) to Rust. Includes:
- `levenshtein_distance` — Wagner-Fischer O(min(a,b)) space
- `levenshtein_similarity` — normalized 0-1
- `ngrams` — character-level n-gram extraction
- `ngram_similarity` — Sorensen-Dice coefficient
- `tokenize` — lowercase word splitting
- `token_overlap_score` — Jaccard index
- `fuzzy_score` — composite: best-window Levenshtein + bigram + token overlap
- `matches_metadata_filter` — all 16 metadata filter modes
- `score_text` — dispatch to fuzzy/substring/exact/regex/token

Include comprehensive `#[cfg(test)]` unit tests (15+ tests) covering all search modes and metadata filter operators.

**Key behavior to match exactly:**
- `levenshtein_distance("kitten", "sitting")` = 3
- `levenshtein_similarity("", "")` = 1.0
- `fuzzy_score` short-circuits to 1.0 when candidate contains query (for queries >= 3 chars)
- `tokenize` strips non-letter/non-number chars, lowercases
- Metadata `between` mode takes an array of exactly 2 strings, parses as numbers
- Regex filter uses Rust `regex::Regex` (cached in a simple LRU)

**Step 1:** Create `text_search.rs` with all functions and tests.

**Step 2:** Run `cargo test text_search` — all tests pass.

**Step 3:** Commit: `feat(vector-engine): text search with Levenshtein, n-gram, fuzzy, token, metadata matching`

---

### Task 4: Inverted Index (BM25)

**Files:**
- Create: `simse-vector/engine/src/inverted_index.rs`

**What to build:**

Port `simse-vector/src/inverted-index.ts` (233 lines). BM25 ranking via an inverted term index:
- `InvertedIndex` struct with token→doc_id mappings, document frequencies, doc lengths
- `add_entry(id, text)` — tokenize and index
- `remove_entry(id, text)` — remove from index
- `bm25_search(query, k1=1.2, b=0.75)` → sorted results
- `clear()`

Use the same `tokenize()` function from `text_search.rs`.

Include 8+ tests covering: basic indexing, BM25 scoring, removal, clear, empty queries.

**Step 1:** Create `inverted_index.rs` with tests.

**Step 2:** Run `cargo test inverted_index` — all tests pass.

**Step 3:** Commit: `feat(vector-engine): BM25 inverted index`

---

### Task 5: Cataloging (Topic Index + Metadata Index)

**Files:**
- Create: `simse-vector/engine/src/cataloging.rs`

**What to build:**

Port the indexing structures from `simse-vector/src/cataloging.ts` (603 lines):

**TopicIndex:**
- Extracts topics from `metadata.topic` (if present) or auto-extracts from text (top N frequent words, excluding stop words)
- Maintains topic→entry_id and entry_id→topics mappings
- Hierarchical: `code/rust` is a child of `code`
- `get_entries(topic)` returns IDs including descendants
- `get_all_topics()` returns `TopicInfo` with hierarchy
- `merge_topic(from, to)` moves all entries
- `get_children(topic)` returns direct children
- `get_related_topics(topic)` returns co-occurring topics sorted by count

**MetadataIndex:**
- O(1) lookup by (key, value) → Set<id>
- `add_entry(id, metadata)`, `remove_entry(id, metadata)`
- `get_entries(key, value)` → Set<id>

**MagnitudeCache:**
- HashMap<String, f64> caching computed magnitudes
- `get(id)`, `set(id, embedding)`, `remove(id)`, `clear()`

Include 12+ tests covering topic extraction, hierarchy, merge, metadata lookup, magnitude cache.

**Step 1:** Create `cataloging.rs` with tests.

**Step 2:** Run `cargo test cataloging` — all tests pass.

**Step 3:** Commit: `feat(vector-engine): topic index, metadata index, magnitude cache`

---

### Task 6: Deduplication + Recommendation + Patron Learning

**Files:**
- Create: `simse-vector/engine/src/deduplication.rs`
- Create: `simse-vector/engine/src/recommendation.rs`
- Create: `simse-vector/engine/src/learning.rs`

**What to build:**

**deduplication.rs** — Port `deduplication.ts` (120 lines):
- `check_duplicate(embedding, volumes, threshold)` → `DuplicateCheckResult`
- `find_duplicate_volumes(volumes, threshold)` → `Vec<DuplicateVolumes>`
- Greedy clustering, O(N^2), sorted by timestamp

**recommendation.rs** — Port `recommendation.ts` (136 lines):
- `normalize_weights(weights)` → `RequiredWeightProfile`
- `recency_score(timestamp, half_life_ms, now)` → f64 (exponential decay)
- `frequency_score(access_count, max_access_count)` → f64 (log scaling)
- `compute_recommendation_score(input, weights)` → combined score

**learning.rs** — Port `patron-learning.ts` (814 lines):
- `LearningEngine` struct with query history, adapted weights, interest embedding
- `record_query(embedding, selected_ids)` — track queries and adapt weights
- `record_feedback(entry_id, relevant)` — explicit feedback
- `get_profile()` → `PatronProfile`
- `serialize()` / `restore()` for persistence
- `prune_entries(valid_ids)` — remove stale references
- `clear()`

Include 10+ tests per module.

**Step 1:** Create all three files with tests.

**Step 2:** Run `cargo test deduplication recommendation learning` — all tests pass.

**Step 3:** Commit: `feat(vector-engine): deduplication, recommendation scoring, patron learning`

---

### Task 7: Topic Catalog + Query DSL + Text Cache + Prompt Injection

**Files:**
- Create: `simse-vector/engine/src/topic_catalog.rs`
- Create: `simse-vector/engine/src/query_dsl.rs`
- Create: `simse-vector/engine/src/text_cache.rs`
- Create: `simse-vector/engine/src/prompt_injection.rs`

**What to build:**

**topic_catalog.rs** — Port `topic-catalog.ts` (172 lines):
- Hierarchical topic tree with Levenshtein fuzzy matching (threshold 0.85)
- `resolve(topic)` — check aliases, exact match, fuzzy match, or register new
- `relocate(volume_id, new_topic)`, `merge(source, target)`
- `sections()` → `Vec<TopicCatalogSection>`
- `volumes(topic)` → `Vec<String>`
- `add_alias(alias, canonical)`, `register_volume(id, topic)`, `remove_volume(id)`
- `get_topic_for_volume(id)` → `Option<String>`

**query_dsl.rs** — Port `query-dsl.ts` (161 lines):
- Parse DSL strings like `topic:code AND tag:rust` into `SearchOptions`
- Support: topic filters, metadata filters, text search, min score

**text_cache.rs** — Port `text-cache.ts` (123 lines):
- Simple LRU cache for text search results
- `put(key, results)`, `get(key)`, `clear()`, `remove(key)`

**prompt_injection.rs** — Port `prompt-injection.ts` (94 lines):
- `format_memory_context(lookups, options)` → String
- Structured or natural format for injecting search results into prompts

Include 8+ tests per module.

**Step 1:** Create all four files with tests.

**Step 2:** Run `cargo test topic_catalog query_dsl text_cache prompt_injection` — all tests pass.

**Step 3:** Commit: `feat(vector-engine): topic catalog, query DSL, text cache, prompt injection`

---

### Task 8: Persistence (Binary Format + Gzip)

**Files:**
- Create: `simse-vector/engine/src/persistence.rs`

**What to build:**

Port `stacks-serialize.ts` (228 lines) and `preservation.ts` (85 lines). Must be **format-compatible** with the existing TS implementation so Rust can read TS-written stores.

**Binary entry format per key-value pair:**
```
[4B text-len][text UTF-8][4B emb-b64-len][emb base64][4B meta-json-len][meta JSON]
[8B timestamp (two 32-bit BE halves)][4B accessCount][8B lastAccessed (two 32-bit BE halves)]
```

**Functions:**
- `encode_embedding(embedding: &[f32])` → base64 string (Float32 LE bytes → base64)
- `decode_embedding(encoded: &str)` → `Vec<f32>`
- `serialize_entry(volume, access_stats)` → `Vec<u8>`
- `deserialize_entry(id, data)` → `Option<(Volume, AccessStats)>`
- `serialize_to_storage(volumes, access_stats, learning_state)` → `HashMap<String, Vec<u8>>`
- `deserialize_from_storage(data)` → `DeserializedData`
- Learning state key: `"__learning"` (JSON serialized)

**Important:** Float32 encoding must match the JS `Float32Array` byte order (little-endian on all modern platforms, which Rust's `f32::to_le_bytes()` matches).

Include 10+ tests covering: encode/decode roundtrip, entry serialization roundtrip, learning state, corrupt data handling.

**Step 1:** Create `persistence.rs` with tests.

**Step 2:** Run `cargo test persistence` — all tests pass.

**Step 3:** Commit: `feat(vector-engine): binary persistence format compatible with TS v2`

---

### Task 9: Volume Store (Core State Manager)

**Files:**
- Create: `simse-vector/engine/src/store.rs`

**What to build:**

The central stateful module. Port `stacks.ts` (914 lines) and parts of `stacks-search.ts` (490 lines) and `stacks-recommend.ts` (203 lines). This is the largest single module.

**VolumeStore struct:**
- `volumes: Vec<Volume>`
- `topic_index: TopicIndex`
- `metadata_index: MetadataIndex`
- `magnitude_cache: MagnitudeCache`
- `inverted_index: InvertedIndex`
- `topic_catalog: TopicCatalog`
- `learning_engine: Option<LearningEngine>`
- `access_stats: HashMap<String, AccessStats>`
- `config: StoreConfig`
- `initialized: bool`
- `dirty: bool`

**StoreConfig:**
- `storage_path: Option<String>` (directory for file persistence)
- `duplicate_threshold: f64`
- `duplicate_behavior: DuplicateBehavior` (Skip/Warn/Error)
- `max_regex_pattern_length: usize`
- `learning_enabled: bool`
- `learning_options: LearningOptions`
- `recency_half_life_ms: u64`

**Methods (matching TS stacks.ts API):**
- `initialize(storage_path)` — load from disk if path provided
- `dispose()` — save if dirty, cleanup
- `save()` — serialize and write to disk
- `add(text, embedding, metadata)` → `Result<String, VectorError>` (with duplicate detection)
- `add_batch(entries)` → `Result<Vec<String>, VectorError>`
- `delete(id)` → `bool`
- `delete_batch(ids)` → `usize`
- `clear()`
- `search(query_embedding, max_results, threshold)` → `Vec<Lookup>`
- `text_search(options)` → `Vec<TextLookup>`
- `filter_by_metadata(filters)` → `Vec<Volume>`
- `filter_by_date_range(range)` → `Vec<Volume>`
- `advanced_search(options)` → `Vec<AdvancedLookup>`
- `recommend(options)` → `Vec<Recommendation>`
- `get_by_id(id)` → `Option<Volume>`
- `get_all()` → `Vec<Volume>`
- `get_topics()` → `Vec<TopicInfo>`
- `filter_by_topic(topics)` → `Vec<Volume>`
- `find_duplicates(threshold)` → `Vec<DuplicateVolumes>`
- `check_duplicate(embedding)` → `DuplicateCheckResult`
- `size()` → `usize`
- `is_dirty()` → `bool`
- Learning: `record_query(embedding, selected_ids)`, `record_feedback(entry_id, relevant)`, `get_profile()` → `Option<PatronProfile>`
- Topic catalog: `catalog_resolve(topic)`, `catalog_relocate(volume_id, new_topic)`, `catalog_merge(source, target)`, `catalog_sections()`, `catalog_volumes(topic)`

Internal helpers: `index_volume()`, `deindex_volume()`, `track_access()`, `rebuild_indexes()`

Include 20+ unit tests covering: add/delete/search lifecycle, duplicate detection (skip/warn/error), text search modes, metadata filtering, date range filtering, advanced search, recommendation, topic operations, learning integration, persistence round-trip.

**Step 1:** Create `store.rs` with tests.

**Step 2:** Run `cargo test store` — all tests pass.

**Step 3:** Commit: `feat(vector-engine): VolumeStore with full CRUD, search, recommendation, persistence`

---

### Task 10: Server Dispatcher (JSON-RPC Method Routing)

**Files:**
- Create: `simse-vector/engine/src/server.rs`

**What to build:**

Port the JSON-RPC dispatcher pattern from `simse-vfs/engine/src/server.rs`. Routes JSON-RPC methods to VolumeStore operations.

**Methods to route:**

| JSON-RPC Method | Store Method | Params → Result |
|---|---|---|
| `store/initialize` | `initialize(path)` | `{storagePath?}` → `{}` |
| `store/dispose` | `dispose()` | `{}` → `{}` |
| `store/save` | `save()` | `{}` → `{}` |
| `store/add` | `add(text, embedding, metadata)` | `{text, embedding, metadata?}` → `{id}` |
| `store/addBatch` | `add_batch(entries)` | `{entries}` → `{ids}` |
| `store/delete` | `delete(id)` | `{id}` → `{deleted}` |
| `store/deleteBatch` | `delete_batch(ids)` | `{ids}` → `{count}` |
| `store/clear` | `clear()` | `{}` → `{}` |
| `store/search` | `search(...)` | `{queryEmbedding, maxResults?, threshold?}` → `{results}` |
| `store/textSearch` | `text_search(...)` | `{query, mode?, threshold?}` → `{results}` |
| `store/advancedSearch` | `advanced_search(...)` | full SearchOptions → `{results}` |
| `store/recommend` | `recommend(...)` | RecommendOptions → `{results}` |
| `store/getById` | `get_by_id(id)` | `{id}` → `{volume?}` |
| `store/getAll` | `get_all()` | `{}` → `{volumes}` |
| `store/getTopics` | `get_topics()` | `{}` → `{topics}` |
| `store/filterByTopic` | `filter_by_topic(topics)` | `{topics}` → `{volumes}` |
| `store/filterByMetadata` | `filter_by_metadata(filters)` | `{filters}` → `{volumes}` |
| `store/filterByDateRange` | `filter_by_date_range(range)` | `{after?, before?}` → `{volumes}` |
| `store/size` | `size()` | `{}` → `{count}` |
| `store/checkDuplicate` | `check_duplicate(embedding)` | `{embedding, threshold?}` → result |
| `store/findDuplicates` | `find_duplicates(threshold)` | `{threshold?}` → `{groups}` |
| `catalog/resolve` | `catalog_resolve(topic)` | `{topic}` → `{resolved}` |
| `catalog/relocate` | `catalog_relocate(...)` | `{volumeId, newTopic}` → `{}` |
| `catalog/merge` | `catalog_merge(...)` | `{source, target}` → `{}` |
| `catalog/sections` | `catalog_sections()` | `{}` → `{sections}` |
| `catalog/volumes` | `catalog_volumes(topic)` | `{topic}` → `{volumeIds}` |
| `learning/recordQuery` | `record_query(...)` | `{embedding, selectedIds}` → `{}` |
| `learning/recordFeedback` | `record_feedback(...)` | `{entryId, relevant}` → `{}` |
| `learning/profile` | `get_profile()` | `{}` → `{profile?}` |
| `query/parse` | `parse_query(dsl)` | `{dsl}` → `{searchOptions}` |
| `format/memoryContext` | `format_memory_context(...)` | `{lookups, options?}` → `{text}` |

Error responses use the standard JSON-RPC error format with `data.vectorCode` set to the `VectorError.code()` value.

**Step 1:** Create `server.rs`.

**Step 2:** Run `cargo test server` and `cargo build` — all pass, binary compiles.

**Step 3:** Commit: `feat(vector-engine): JSON-RPC server dispatcher with all method routes`

---

### Task 11: Rust Integration Tests

**Files:**
- Create: `simse-vector/engine/tests/integration.rs`

**What to build:**

End-to-end tests that spawn the Rust binary and communicate via JSON-RPC, similar to `simse-vfs/engine/tests/integration.rs`.

**Test helper:**
```rust
struct VectorProcess {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}
```

**Tests (12+):**
1. `initialize_and_add` — init, add volume with embedding, get by ID
2. `search_by_embedding` — add 3 volumes, search, verify ordering
3. `text_search_fuzzy` — add volumes, fuzzy text search
4. `text_search_bm25` — BM25 search mode
5. `metadata_filtering` — add with metadata, filter by eq/contains/regex
6. `date_range_filtering` — filter by timestamp range
7. `duplicate_detection` — add similar volumes, check duplicate
8. `delete_and_batch` — add, delete, verify gone
9. `advanced_search` — combined vector + text + metadata search
10. `topic_catalog_operations` — resolve, relocate, merge, sections
11. `recommendation` — recommend with weights
12. `learning_profile` — record queries, check profile
13. `persistence_round_trip` — init with path, add volumes, dispose, re-init, verify data
14. `error_before_init` — operations fail before initialize
15. `unknown_method` — returns method-not-found error

**Step 1:** Create `integration.rs`.

**Step 2:** Run `cargo test --test integration` — all tests pass.

**Step 3:** Commit: `test(vector-engine): integration tests for JSON-RPC protocol`

---

### Task 12: TypeScript JSON-RPC Client

**Files:**
- Create: `simse-vector/src/client.ts`

**What to build:**

Port the pattern from `simse-vfs/src/client.ts` (the existing VFS client). This is a thin JSON-RPC 2.0 client that spawns the Rust binary as a child process.

**Interface:**
```typescript
export interface VectorClientOptions {
    readonly enginePath: string;
    readonly timeoutMs?: number;   // default 60_000
    readonly logger?: Logger;
}

export interface VectorClient {
    readonly request: <T>(method: string, params?: unknown) => Promise<T>;
    readonly dispose: () => Promise<void>;
    readonly isHealthy: boolean;
}

export function createVectorClient(options: VectorClientOptions): Promise<VectorClient>;
```

**Implementation details:**
- Spawn `enginePath` binary with `node:child_process` `spawn()` (no shell)
- NDJSON line buffering on stdout
- Pending request map (id → {resolve, reject, timer})
- Error mapping: JSON-RPC error.data.vectorCode → createStacksError/createLibraryError
- Stderr routing to logger
- Health check: child process liveness
- Dispose: send empty line, kill process, clear pending requests

**Step 1:** Create `client.ts`.

**Step 2:** Verify TypeScript compiles: `cd simse-vector && npx tsc --noEmit`

**Step 3:** Commit: `feat(vector): TypeScript JSON-RPC client for Rust engine`

---

### Task 13: Rewrite Stacks as Thin Async Client

**Files:**
- Modify: `simse-vector/src/stacks.ts`
- Modify: `simse-vector/src/lib.ts`

**What to build:**

Replace the 914-line stacks.ts with a thin async wrapper that delegates to the Rust engine via the VectorClient.

**Key changes:**
- `createStacks()` now accepts `enginePath` option and spawns the Rust engine
- All methods become async (they send JSON-RPC requests)
- The `Stacks` interface changes: `search`, `textSearch`, `filterByMetadata`, `filterByDateRange`, `advancedSearch`, `getAll`, `getById`, `getTopics`, `filterByTopic`, `findDuplicates`, `checkDuplicate`, `recommend` all become async (return Promise)
- `load()` sends `store/initialize` to Rust
- `save()` sends `store/save`
- `dispose()` sends `store/dispose` then kills the subprocess
- The `StorageBackend` interface is no longer used by Stacks (Rust handles persistence directly)

**New StacksOptions:**
```typescript
export interface StacksOptions {
    readonly enginePath: string;
    readonly storagePath?: string;  // directory for Rust persistence
    readonly duplicateThreshold?: number;
    readonly duplicateBehavior?: 'skip' | 'warn' | 'error';
    readonly maxRegexPatternLength?: number;
    readonly learning?: LearningOptions;
    readonly recency?: RecencyOptions;
    readonly logger?: Logger;
}
```

**Updated Stacks interface** (all query methods become async):
```typescript
export interface Stacks {
    readonly load: () => Promise<void>;
    readonly save: () => Promise<void>;
    readonly dispose: () => Promise<void>;
    readonly add: (text: string, embedding: readonly number[], metadata?: Record<string, string>) => Promise<string>;
    readonly addBatch: (...) => Promise<string[]>;
    readonly delete: (id: string) => Promise<boolean>;
    readonly deleteBatch: (ids: readonly string[]) => Promise<number>;
    readonly clear: () => Promise<void>;
    // These change from sync to async:
    readonly search: (queryEmbedding: readonly number[], maxResults: number, threshold: number) => Promise<Lookup[]>;
    readonly textSearch: (options: TextSearchOptions) => Promise<TextLookup[]>;
    readonly filterByMetadata: (filters: readonly MetadataFilter[]) => Promise<Volume[]>;
    readonly filterByDateRange: (range: DateRange) => Promise<Volume[]>;
    readonly advancedSearch: (options: SearchOptions) => Promise<AdvancedLookup[]>;
    readonly getAll: () => Promise<Volume[]>;
    readonly getById: (id: string) => Promise<Volume | undefined>;
    readonly getTopics: () => Promise<TopicInfo[]>;
    readonly filterByTopic: (topics: readonly string[]) => Promise<Volume[]>;
    readonly findDuplicates: (threshold?: number) => Promise<DuplicateVolumes[]>;
    readonly checkDuplicate: (embedding: readonly number[]) => Promise<DuplicateCheckResult>;
    readonly recommend: (options?: RecommendOptions) => Promise<Recommendation[]>;
    readonly learningEngine: undefined; // No longer exposed
    readonly learningProfile: Promise<PatronProfile | undefined>;
    readonly size: Promise<number>;
    readonly isDirty: boolean; // Tracked locally
}
```

**Embedding transport:** f32 arrays are sent as regular JSON number arrays in the JSON-RPC params. The Rust side deserializes them into `Vec<f32>`.

**Update lib.ts:** Add `VectorClient`, `VectorClientOptions` exports. Remove exports that moved to Rust (the pure function exports like `cosineSimilarity`, `levenshteinDistance`, etc. are no longer exported from the TS package since they live in Rust now). Keep type exports.

**Step 1:** Rewrite `stacks.ts` as thin client.

**Step 2:** Update `lib.ts` exports.

**Step 3:** Verify TypeScript compiles.

**Step 4:** Commit: `refactor(vector): rewrite stacks as thin async client over Rust engine`

---

### Task 14: Update Library Facade for Async Stacks

**Files:**
- Modify: `simse-vector/src/library.ts`
- Modify: `simse-vector/src/shelf.ts`

**What to build:**

The Library facade (`library.ts`, 816 lines) currently calls sync methods on stacks (search, textSearch, etc.). These are now async. Add `await` to all stacks calls.

**Changes in library.ts:**
- `searchFn`: `const results = await store.search(...)` (was sync)
- `textSearch`: `const results = await store.textSearch(...)` → return type becomes `Promise<TextLookup[]>`
- `filterByMetadata`: `await store.filterByMetadata(...)` → return type becomes `Promise<Volume[]>`
- `filterByDateRange`: `await store.filterByDateRange(...)` → return type becomes `Promise<Volume[]>`
- `advancedSearch`: `const results = await store.advancedSearch(...)` (already async but inner call was sync)
- `queryDsl`: `await store.advancedSearch(...)` + `await store.filterByTopic(...)`
- `getById`: `await store.getById(...)` → return type becomes `Promise<Volume | undefined>`
- `getAll`: `await store.getAll()` → return type becomes `Promise<Volume[]>`
- `getTopics`: `await store.getTopics()` → return type becomes `Promise<TopicInfo[]>`
- `filterByTopic`: `await store.filterByTopic(...)` → return type becomes `Promise<Volume[]>`
- `findDuplicates`: `await store.findDuplicates(...)` → return type becomes `Promise<DuplicateVolumes[]>`
- `recommend`: `await store.recommend(...)` (already async)
- `compendium`: `await store.getById(...)` for gathering volumes
- `recordFeedback`: need to call through JSON-RPC now
- `size`: becomes async getter or method
- `shelves()`: `await store.getAll()` then filter

The `Library` interface in `library.ts` needs updating — several sync methods become async.

Also update `createLibrary()` signature:
- Remove `storage: StorageBackend` from options (Rust handles persistence)
- Add `enginePath: string`
- Add `storagePath?: string`

**shelf.ts** changes:
- `volumes()` becomes async since `library.getAll()` is now async

**Step 1:** Update `library.ts` — add await to all stacks calls, update interface.

**Step 2:** Update `shelf.ts` — make volumes() async.

**Step 3:** Verify TypeScript compiles.

**Step 4:** Commit: `refactor(vector): update library and shelf for async stacks`

---

### Task 15: Update Consumers for Async Library API

**Files:**
- Modify: `simse-vector/src/circulation-desk.ts`
- Modify: `simse-vector/src/library-services.ts`
- Modify: `simse-vector/src/librarian-registry.ts`
- Modify: `src/ai/tools/builtin-tools.ts`
- Modify: `src/ai/mcp/mcp-server.ts`

**What to build:**

Update all consumers that call now-async Library/Stacks methods.

**circulation-desk.ts:**
- `getVolumesForTopic(topic)` callback is now async → change type to `() => Promise<Volume[]>`
- `checkDuplicate` was already async
- `addVolume` was already async

**library-services.ts:**
- Already uses `await library.search(...)` — should be fine
- Check `library.isInitialized` and `library.size` — if size becomes async, need await

**librarian-registry.ts:**
- Check for any sync calls to library that are now async

**builtin-tools.ts (in main src/):**
- `registerLibraryTools`: library.search already awaited, library.getTopics now needs await, library.filterByTopic now needs await

**mcp-server.ts (in main src/):**
- Similar: add await to any library method calls that changed

**Step 1:** Update all consumer files.

**Step 2:** Run `bun run typecheck` — clean.

**Step 3:** Commit: `refactor: update library consumers for async stacks API`

---

### Task 16: Delete Modules Moved to Rust

**Files:**
- Delete: `simse-vector/src/cosine.ts`
- Delete: `simse-vector/src/text-search.ts`
- Delete: `simse-vector/src/inverted-index.ts`
- Delete: `simse-vector/src/cataloging.ts`
- Delete: `simse-vector/src/deduplication.ts`
- Delete: `simse-vector/src/recommendation.ts`
- Delete: `simse-vector/src/patron-learning.ts`
- Delete: `simse-vector/src/topic-catalog.ts`
- Delete: `simse-vector/src/stacks-search.ts`
- Delete: `simse-vector/src/stacks-recommend.ts`
- Delete: `simse-vector/src/stacks-persistence.ts`
- Delete: `simse-vector/src/stacks-serialize.ts`
- Delete: `simse-vector/src/preservation.ts`
- Delete: `simse-vector/src/query-dsl.ts`
- Delete: `simse-vector/src/text-cache.ts`
- Delete: `simse-vector/src/prompt-injection.ts`
- Delete: `simse-vector/src/storage.ts`
- Modify: `simse-vector/src/lib.ts` — remove all exports from deleted files
- Modify: `simse-vector/src/errors.ts` — keep (still needed for TS error mapping)
- Modify: `simse-vector/src/logger.ts` — keep (still needed)

**What to build:**

Delete all 17 TS files whose functionality moved to Rust. Update `lib.ts` to only export:
- Types from `types.ts` (all type exports preserved)
- Client: `VectorClient`, `VectorClientOptions`, `createVectorClient`
- Stacks: `Stacks`, `StacksOptions`, `createStacks`
- Library: `Library`, `LibraryOptions`, `createLibrary`
- Shelf: `createShelf`
- Librarian: `Librarian`, `LibrarianOptions`, `createLibrarian`, `createDefaultLibrarian`
- LibrarianDefinition: all exports
- LibrarianRegistry: all exports
- CirculationDesk: `CirculationDeskOptions`, `createCirculationDesk`
- LibraryServices: `LibraryServices`, `LibraryServicesOptions`, `createLibraryServices`
- Errors: all error factories and guards
- Logger: `Logger`, `EventBus`, `createNoopLogger`

**Step 1:** Delete the 17 files.

**Step 2:** Update `lib.ts` to remove deleted imports and add new client exports.

**Step 3:** Fix any remaining import errors in kept files (librarian.ts, etc. may import from deleted files — check and fix).

**Step 4:** Run `bun run typecheck` — clean.

**Step 5:** Commit: `refactor(vector): remove TS modules moved to Rust engine`

---

### Task 17: Update Tests for Async API

**Files:**
- Modify: `simse-vector/tests/stacks.test.ts`
- Modify: `simse-vector/tests/library.test.ts`
- Modify: `simse-vector/tests/library-types.test.ts`
- Modify: `simse-vector/tests/library-services.test.ts`
- Modify: `simse-vector/tests/library-errors.test.ts`
- Modify: `simse-vector/tests/e2e-library-pipeline.test.ts`
- Modify: `simse-vector/tests/hierarchical-library-integration.test.ts`
- Modify: `tests/builtin-tools.test.ts`

**What to build:**

Mechanical transformation similar to VFS refactor Task 14. For every test file:

1. Add `ENGINE_PATH` constant:
   ```typescript
   const ENGINE_PATH = fileURLToPath(
       new URL('../engine/target/debug/simse-vector-engine.exe', import.meta.url),
   );
   ```

2. Make `beforeEach` async, create stacks/library with `enginePath`:
   ```typescript
   beforeEach(async () => {
       stacks = createStacks({ enginePath: ENGINE_PATH, ... });
       await stacks.load();
   });
   ```

3. Add `afterEach` for cleanup:
   ```typescript
   afterEach(async () => {
       await stacks?.dispose();
   });
   ```

4. Add `await` to all previously-sync methods that are now async:
   - `store.search(...)` → `await store.search(...)`
   - `store.textSearch(...)` → `await store.textSearch(...)`
   - `store.getAll()` → `await store.getAll()`
   - `store.getById(id)` → `await store.getById(id)`
   - `store.getTopics()` → `await store.getTopics()`
   - `store.filterByTopic(...)` → `await store.filterByTopic(...)`
   - `store.filterByMetadata(...)` → `await store.filterByMetadata(...)`
   - `store.filterByDateRange(...)` → `await store.filterByDateRange(...)`
   - `store.findDuplicates(...)` → `await store.findDuplicates(...)`
   - `store.checkDuplicate(...)` → `await store.checkDuplicate(...)`
   - `store.recommend(...)` → `await store.recommend(...)`
   - `library.getAll()` → `await library.getAll()`
   - etc.

5. Change sync error assertions to async:
   ```typescript
   // Before:
   expect(() => store.search(...)).toThrow();
   // After:
   await expect(store.search(...)).rejects.toThrow();
   ```

6. Remove `StorageBackend` mock creation — stacks now manages its own persistence via Rust.

7. Update `createStacks()` and `createLibrary()` calls to use new options shape.

**Step 1:** Update all 8 test files.

**Step 2:** Build the Rust engine: `cd simse-vector/engine && cargo build`

**Step 3:** Run tests: `bun test simse-vector/tests/`

**Step 4:** Fix any failures (likely protocol alignment issues — types, field names, etc.).

**Step 5:** Commit: `test(vector): update all tests for async Rust-backed stacks`

---

### Task 18: Build Scripts + Final Verification

**Files:**
- Modify: `simse-vector/package.json`
- Modify: `simse-vector/.gitignore`
- Modify: `biome.json` (if needed)

**What to build:**

**package.json** — add engine build scripts:
```json
{
  "scripts": {
    "build:engine": "cd engine && cargo build --release",
    "build:engine:debug": "cd engine && cargo build",
    "test:engine": "cd engine && cargo test"
  }
}
```

**.gitignore** — ensure `engine/target/` and `engine/Cargo.lock` are ignored (may already be there).

**biome.json** — ensure `simse-vector/tests/**` is in the lint override for `noExplicitAny` (may already be there from VFS refactor).

**Final verification steps:**

1. `cd simse-vector/engine && cargo test` — all Rust tests pass
2. `cd simse-vector/engine && cargo build` — debug binary builds
3. `bun run typecheck` — no TS errors
4. `bun run lint` — no lint errors
5. `bun test simse-vector/tests/` — all TS tests pass
6. `bun test tests/builtin-tools.test.ts` — consumer tests pass
7. `bun test` — full test suite passes

**Step 1:** Update package.json and .gitignore.

**Step 2:** Run all verification commands.

**Step 3:** Fix any remaining issues.

**Step 4:** Commit: `chore(vector): build scripts and final verification`

---

## Summary

| Task | Module | Lines Ported | Est. Complexity |
|------|--------|-------------|-----------------|
| 1 | Scaffold + types + transport | - | Medium |
| 2 | Cosine + magnitude | ~60 | Low |
| 3 | Text search | ~389 | High |
| 4 | Inverted index (BM25) | ~233 | Medium |
| 5 | Cataloging (3 indexes) | ~603 | High |
| 6 | Dedup + recommendation + learning | ~1,070 | High |
| 7 | Topic catalog + DSL + cache + prompt | ~550 | Medium |
| 8 | Persistence (binary format) | ~313 | High |
| 9 | Volume store (core) | ~1,607 | Very High |
| 10 | Server dispatcher | - | Medium |
| 11 | Rust integration tests | - | Medium |
| 12 | TS JSON-RPC client | ~200 | Medium |
| 13 | Rewrite stacks.ts | -914/+300 | High |
| 14 | Update library + shelf | ~816 modified | High |
| 15 | Update consumers | ~6 files | Medium |
| 16 | Delete moved modules | -17 files | Low |
| 17 | Update tests | ~3,462 modified | Very High |
| 18 | Build scripts + verification | - | Low |
