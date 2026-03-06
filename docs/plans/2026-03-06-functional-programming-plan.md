# Functional Programming Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refactor all 10 Rust crates to pure functional programming style — immutable data via `im` crate, owned-return patterns, Effect enums for I/O, TDD-first.

**Architecture:** Replace `&mut self` with `self -> (Self, T)`, replace std collections with `im` persistent data structures, extract pure free functions, use Effect enums for side effects. Servers use `(State, Request) -> (State, Response)` pattern.

**Tech Stack:** Rust, `im` crate v15 (with serde feature), existing deps unchanged.

---

## Phase 1: Leaf Crates (no internal dependencies)

### Task 1: simse-vsh — VirtualShell (smallest crate, 1,535 LOC, 10 `&mut self` methods)

Start here — smallest crate, establishes patterns for all other crates.

**Files:**
- Modify: `simse-vsh/Cargo.toml`
- Modify: `simse-vsh/src/shell.rs` (367 lines, 9 `&mut self` methods)
- Modify: `simse-vsh/src/server.rs` (425 lines, 1 `&mut self` method + private dispatch)
- Modify: `simse-vsh/src/protocol.rs` (183 lines — collection types in params)
- Test: `simse-vsh/tests/integration.rs` (642 lines)

**Step 1: Add `im` dependency**

In `simse-vsh/Cargo.toml`, add under `[dependencies]`:
```toml
im = { version = "15", features = ["serde"] }
```

**Step 2: Write FP-style unit tests for ShellSession**

Create `simse-vsh/tests/fp_shell.rs`:
```rust
use simse_vsh_engine::shell::{VirtualShell, ShellSession};
use std::collections::HashMap;

#[test]
fn create_session_returns_new_shell() {
    let shell = VirtualShell::new(None);
    let (shell2, session) = shell.create_session(None, None, None).unwrap();

    // Original unchanged
    assert_eq!(shell.list_sessions().len(), 0);
    // New state has the session
    assert_eq!(shell2.list_sessions().len(), 1);
    assert!(session.id.len() > 0);
}

#[test]
fn delete_session_returns_new_shell() {
    let shell = VirtualShell::new(None);
    let (shell2, session) = shell.create_session(None, None, None).unwrap();
    let (shell3, deleted) = shell2.delete_session(&session.id).unwrap();

    assert_eq!(shell3.list_sessions().len(), 0);
    assert!(deleted);
    // shell2 still has the session
    assert_eq!(shell2.list_sessions().len(), 1);
}

#[test]
fn set_env_returns_new_shell() {
    let shell = VirtualShell::new(None);
    let (shell2, session) = shell.create_session(None, None, None).unwrap();
    let shell3 = shell2.set_env(&session.id, "FOO", "bar").unwrap();

    let env = shell3.get_env(&session.id).unwrap();
    assert_eq!(env.get("FOO"), Some(&"bar".to_string()));
}

#[test]
fn set_alias_returns_new_shell() {
    let shell = VirtualShell::new(None);
    let (shell2, session) = shell.create_session(None, None, None).unwrap();
    let shell3 = shell2.set_alias(&session.id, "ll", "ls -la").unwrap();

    let aliases = shell3.get_aliases(&session.id).unwrap();
    assert_eq!(aliases.get("ll"), Some(&"ls -la".to_string()));
}
```

**Step 3: Run tests to verify they fail**

Run: `cd simse-vsh && cargo test --test fp_shell`
Expected: FAIL — methods still take `&mut self` and don't return new state.

**Step 4: Refactor ShellSession and VirtualShell types to use `im` collections**

In `simse-vsh/src/shell.rs`, replace:
```rust
// OLD (line 23-25):
pub struct ShellSession {
    pub env: HashMap<String, String>,
    pub aliases: HashMap<String, String>,
    pub history: Vec<HistoryEntry>,
    // ...
}

// NEW:
pub struct ShellSession {
    pub env: im::HashMap<String, String>,
    pub aliases: im::HashMap<String, String>,
    pub history: im::Vector<HistoryEntry>,
    // ...
}
```

Replace VirtualShell (line 41):
```rust
// OLD:
pub struct VirtualShell {
    sessions: HashMap<String, ShellSession>,
    // ...
}

// NEW:
pub struct VirtualShell {
    sessions: im::HashMap<String, ShellSession>,
    // ...
}
```

**Step 5: Refactor VirtualShell methods from `&mut self` to owned-return**

Change each method signature in `shell.rs`:

```rust
// OLD (line 63):
pub fn create_session(&mut self, name: Option<String>, cwd: Option<String>, env: Option<HashMap<String, String>>) -> Result<&ShellSession, VshError>

// NEW:
pub fn create_session(self, name: Option<String>, cwd: Option<String>, env: Option<HashMap<String, String>>) -> Result<(Self, SessionInfo), VshError>
```

Apply the same pattern for all 9 methods:
- `delete_session` → `(Self, bool)`
- `set_env` → `Self` (no secondary return)
- `delete_env` → `(Self, bool)`
- `set_cwd` → `Self`
- `set_alias` → `Self`
- `exec_in_session` → `(Self, ExecResult)` (needs Effect for OS execution)
- `exec_git_in_session` → `(Self, ExecResult)` (needs Effect)
- `exec_raw` → `(Self, ExecResult)` (needs Effect)

For the exec methods, introduce an Effect enum:
```rust
pub enum VshEffect {
    None,
    Execute {
        command: String,
        cwd: String,
        env: im::HashMap<String, String>,
        timeout_ms: Option<u64>,
        max_output_bytes: Option<usize>,
        stdin: Option<String>,
    },
}
```

The exec methods return `(Self, VshEffect)` — the server loop interprets the effect.

**Step 6: Run tests to verify they pass**

Run: `cd simse-vsh && cargo test --test fp_shell`
Expected: PASS

**Step 7: Refactor VshServer to `(State, Request) -> (State, Response)` pattern**

In `simse-vsh/src/server.rs`:

```rust
// Pure dispatch function (free function, not a method)
fn dispatch(state: VirtualShell, method: &str, params: serde_json::Value) -> (VirtualShell, Result<serde_json::Value, VshError>) {
    match method {
        "session/create" => {
            let p: SessionCreateParams = serde_json::from_value(params)?;
            let (new_state, info) = state.create_session(p.name, p.cwd, p.env)?;
            (new_state, Ok(serde_json::to_value(info)?))
        }
        // ... all 19 methods
    }
}

// Event loop — single mut binding
pub async fn run(mut state: VirtualShell, transport: NdjsonTransport) -> Result<(), VshError> {
    while let Some(request) = transport.read().await {
        let (new_state, result) = dispatch(state, &request.method, request.params);
        state = new_state;
        // Handle effects for exec methods...
        transport.write_response(request.id, result).await?;
    }
    Ok(())
}
```

**Step 8: Update integration tests**

Modify `simse-vsh/tests/integration.rs` — these test via JSON-RPC so they should mostly pass without changes since the wire protocol is unchanged. Run and fix any compilation errors from API changes in internal types.

Run: `cd simse-vsh && cargo test`
Expected: All 14 tests PASS

**Step 9: Commit**

```bash
git add simse-vsh/
git commit -m "refactor(simse-vsh): pure FP with im crate — owned-return, Effect enum, immutable state"
```

---

### Task 2: simse-vnet — VirtualNetwork (2,614 LOC, ~18 `&mut self` methods)

**Files:**
- Modify: `simse-vnet/Cargo.toml`
- Modify: `simse-vnet/src/mock_store.rs` (304 lines, 4 `&mut self`)
- Modify: `simse-vnet/src/session.rs` (167 lines, 3 `&mut self`)
- Modify: `simse-vnet/src/network.rs` (514 lines, 10 `&mut self`)
- Modify: `simse-vnet/src/server.rs` (475 lines, dispatch)
- Test: `simse-vnet/tests/integration.rs` (488 lines)

**Step 1: Add `im` dependency**

In `simse-vnet/Cargo.toml`:
```toml
im = { version = "15", features = ["serde"] }
```

**Step 2: Write FP-style tests**

Create `simse-vnet/tests/fp_network.rs`:
```rust
use simse_vnet_engine::mock_store::{MockStore, MockResponse};
use simse_vnet_engine::session::{SessionManager, SessionType, Scheme};
use simse_vnet_engine::network::VirtualNetwork;

#[test]
fn mock_register_returns_new_store() {
    let store = MockStore::new();
    let response = MockResponse { status: 200, body: "ok".into(), headers: Default::default() };
    let (store2, id) = store.register(None, "https://api.example.com/*", response, None);

    assert_eq!(store.list().len(), 0);
    assert_eq!(store2.list().len(), 1);
}

#[test]
fn mock_unregister_returns_new_store() {
    let store = MockStore::new();
    let response = MockResponse { status: 200, body: "ok".into(), headers: Default::default() };
    let (store2, id) = store.register(None, "https://api.example.com/*", response, None);
    let store3 = store2.unregister(&id);

    assert_eq!(store3.list().len(), 0);
    assert_eq!(store2.list().len(), 1);
}

#[test]
fn session_create_returns_new_manager() {
    let mgr = SessionManager::new();
    let (mgr2, info) = mgr.create(SessionType::WebSocket, "ws://localhost:8080", Scheme::Ws);

    assert_eq!(mgr.list().len(), 0);
    assert_eq!(mgr2.list().len(), 1);
}

#[test]
fn session_close_returns_new_manager() {
    let mgr = SessionManager::new();
    let (mgr2, info) = mgr.create(SessionType::WebSocket, "ws://localhost:8080", Scheme::Ws);
    let (mgr3, closed) = mgr2.close(&info.id);

    assert!(closed);
    assert_eq!(mgr3.list().len(), 0);
}
```

**Step 3: Run tests to verify fail**

Run: `cd simse-vnet && cargo test --test fp_network`
Expected: FAIL

**Step 4: Refactor MockStore types**

In `simse-vnet/src/mock_store.rs`:
```rust
// Replace Vec<MockDefinition> with im::Vector<MockDefinition>
// Replace Vec<MockHit> with im::Vector<MockHit>
pub struct MockStore {
    mocks: im::Vector<MockDefinition>,
    hits: im::Vector<MockHit>,
}
```

Change methods from `&mut self` to owned-return:
- `register(self, ...) -> (Self, String)` (returns new store + mock ID)
- `unregister(self, id: &str) -> Self`
- `find_match(self, url: &str, method: Option<&str>) -> (Self, Option<MockResponse>)` (updates hit count)
- `clear(self) -> Self`

**Step 5: Refactor SessionManager types**

In `simse-vnet/src/session.rs`:
```rust
pub struct SessionManager {
    sessions: im::HashMap<String, NetSession>,
}
```

Change methods:
- `create(self, ...) -> (Self, SessionInfo)`
- `close(self, id: &str) -> (Self, bool)`
- `record_activity(self, ...) -> Self`

**Step 6: Refactor VirtualNetwork**

In `simse-vnet/src/network.rs`, compose MockStore + SessionManager, thread state through:
```rust
pub struct VirtualNetwork {
    mock_store: MockStore,
    session_manager: SessionManager,
    sandbox: Option<NetSandboxConfig>,
    metrics: NetworkMetrics,
    // ...
}
```

All methods become `self -> (Self, T)`. HTTP request becomes an Effect:
```rust
pub enum VnetEffect {
    None,
    HttpRequest { url: String, method: String, headers: im::HashMap<String, String>, body: Option<String>, timeout_ms: Option<u64> },
    DnsResolve { hostname: String },
}
```

**Step 7: Refactor VnetServer dispatch to pure functions**

Same pattern as Task 1 Step 7:
```rust
fn dispatch(state: VirtualNetwork, method: &str, params: Value) -> (VirtualNetwork, Result<Value, VnetError>)
```

**Step 8: Run all tests**

Run: `cd simse-vnet && cargo test`
Expected: All 16 tests PASS (integration tests use JSON-RPC, wire format unchanged)

**Step 9: Commit**

```bash
git add simse-vnet/
git commit -m "refactor(simse-vnet): pure FP with im crate — owned-return, Effect enum"
```

---

### Task 3: simse-vfs — VirtualFilesystem (7,370 LOC, 11 `&mut self` methods)

**Files:**
- Modify: `simse-vfs/Cargo.toml`
- Modify: `simse-vfs/src/vfs.rs` (2,309 lines, 10 `&mut self` methods)
- Modify: `simse-vfs/src/server.rs` (1,208 lines)
- Modify: `simse-vfs/src/backend.rs` (112 lines — FsBackend trait)
- Modify: `simse-vfs/src/local_backend.rs` (137 lines)
- Test: `simse-vfs/tests/integration.rs` (1,535 lines)

**Step 1: Add `im` dependency**

```toml
im = { version = "15", features = ["serde"] }
```

**Step 2: Write FP-style tests for VirtualFs**

Create `simse-vfs/tests/fp_vfs.rs`:
```rust
use simse_vfs_engine::vfs::VirtualFs;

#[test]
fn write_file_returns_new_vfs() {
    let vfs = VirtualFs::new();
    let vfs2 = vfs.write_file("/test.txt", "hello").unwrap();

    // Original unchanged
    assert!(vfs.read_file("/test.txt").is_err());
    // New state has the file
    assert_eq!(vfs2.read_file("/test.txt").unwrap(), "hello");
}

#[test]
fn mkdir_returns_new_vfs() {
    let vfs = VirtualFs::new();
    let vfs2 = vfs.mkdir("/a/b/c", true).unwrap();

    assert!(vfs.stat("/a").is_err());
    assert!(vfs2.stat("/a/b/c").is_ok());
}

#[test]
fn delete_file_returns_new_vfs() {
    let vfs = VirtualFs::new();
    let vfs2 = vfs.write_file("/test.txt", "hello").unwrap();
    let (vfs3, deleted) = vfs2.delete_file("/test.txt").unwrap();

    assert!(deleted);
    assert!(vfs3.read_file("/test.txt").is_err());
    // vfs2 still has it
    assert_eq!(vfs2.read_file("/test.txt").unwrap(), "hello");
}

#[test]
fn transaction_returns_new_vfs() {
    let vfs = VirtualFs::new();
    let ops = vec![
        TransactionOp::WriteFile { path: "/a.txt".into(), content: "a".into() },
        TransactionOp::WriteFile { path: "/b.txt".into(), content: "b".into() },
    ];
    let vfs2 = vfs.transaction(ops).unwrap();

    assert_eq!(vfs2.read_file("/a.txt").unwrap(), "a");
    assert_eq!(vfs2.read_file("/b.txt").unwrap(), "b");
    assert!(vfs.read_file("/a.txt").is_err());
}
```

**Step 3: Run tests — verify fail**

Run: `cd simse-vfs && cargo test --test fp_vfs`

**Step 4: Refactor VirtualFs to `im` collections**

In `simse-vfs/src/vfs.rs` (lines 224-233):
```rust
pub struct VirtualFs {
    nodes: im::HashMap<String, InternalNode>,
    history: im::HashMap<String, im::Vector<HistoryEntryInternal>>,
    pending_events: im::Vector<VfsEvent>,
    // ... scalar fields
}
```

**Step 5: Refactor VirtualFs methods to owned-return**

10 methods change:
- `write_file(self, path, content) -> Result<Self, VfsError>` (was missing from `&mut self` list — it's called internally)
- `append_file(self, path, content) -> Result<Self, VfsError>`
- `delete_file(self, path) -> Result<(Self, bool), VfsError>`
- `mkdir(self, path, recursive) -> Result<Self, VfsError>`
- `rmdir(self, path, recursive) -> Result<(Self, bool), VfsError>`
- `rename(self, old, new) -> Result<Self, VfsError>`
- `checkout(self, path, version) -> Result<Self, VfsError>`
- `restore(self, snap) -> Result<Self, VfsError>`
- `clear(self) -> Self`
- `transaction(self, ops) -> Result<Self, VfsError>`
- `drain_events(self) -> (Self, im::Vector<VfsEvent>)`

**Step 6: Refactor FsBackend trait**

In `simse-vfs/src/backend.rs`, change trait methods that take `&mut self` to use Effect pattern:
```rust
pub enum VfsEffect {
    WriteToDisk { path: String, content: Vec<u8> },
    DeleteFromDisk { path: String },
    CreateDir { path: String, recursive: bool },
    // ...
}
```

DiskFs methods are already `&self` — minimal changes needed. LocalFsBackend wraps them.

**Step 7: Refactor VfsServer dispatch**

Same `(State, Request) -> (State, Response)` pattern.

**Step 8: Run all tests**

Run: `cd simse-vfs && cargo test`
Expected: All tests PASS

**Step 9: Commit**

```bash
git add simse-vfs/
git commit -m "refactor(simse-vfs): pure FP with im crate — immutable file tree, owned-return"
```

---

### Task 4: simse-adaptive — Vector Store & PCN (15,649 LOC, 76 `&mut self` methods)

This is the largest leaf crate. Break into sub-tasks by module.

#### Task 4a: Cataloging, InvertedIndex, TopicCatalog, TextCache modules

**Files:**
- Modify: `simse-adaptive/Cargo.toml`
- Modify: `simse-adaptive/src/cataloging.rs` (1,021 lines, 10 `&mut self`)
- Modify: `simse-adaptive/src/inverted_index.rs` (337 lines, 3 `&mut self`)
- Modify: `simse-adaptive/src/topic_catalog.rs` (431 lines, 6 `&mut self`)
- Modify: `simse-adaptive/src/text_cache.rs` (271 lines, 4 `&mut self`)

**Step 1: Add `im` dependency**

```toml
im = { version = "15", features = ["serde"] }
```

**Step 2: Write FP tests for TopicIndex, MetadataIndex, MagnitudeCache**

Create `simse-adaptive/tests/fp_cataloging.rs`:
```rust
#[test]
fn topic_index_add_entry_returns_new_index() {
    let idx = TopicIndex::new();
    let idx2 = idx.add_entry("vol1", "machine learning basics", &HashMap::new());

    assert_eq!(idx.entry_count(), 0);
    assert_eq!(idx2.entry_count(), 1);
}

#[test]
fn inverted_index_add_returns_new_index() {
    let idx = BM25Index::new();
    let idx2 = idx.add_entry("doc1", "hello world");

    assert_eq!(idx.doc_count(), 0);
    assert_eq!(idx2.doc_count(), 1);
}

#[test]
fn topic_catalog_register_returns_new_catalog() {
    let cat = TopicCatalog::new();
    let cat2 = cat.register_volume("vol1", "ml/basics");

    assert!(cat.get_topic("vol1").is_none());
    assert_eq!(cat2.get_topic("vol1"), Some("ml/basics"));
}

#[test]
fn lru_cache_put_returns_new_cache() {
    let cache = LruCache::new(10);
    let cache2 = cache.put("key1", "value1");

    assert!(cache.get("key1").is_none());
    assert_eq!(cache2.get("key1"), Some("value1".to_string()));
}
```

**Step 3: Run tests — fail**

Run: `cd simse-adaptive && cargo test --test fp_cataloging`

**Step 4: Refactor cataloging.rs types**

Replace all HashMap/HashSet/Vec fields with `im` equivalents:
- `TopicIndex`: 5 fields (lines 136-144)
- `MetadataIndex`: 2 fields (lines 471-473)
- `MagnitudeCache`: 1 field (line 559)

**Step 5: Refactor methods to owned-return**

10 methods in cataloging.rs change from `&mut self` to `self -> Self` or `self -> (Self, T)`.

**Step 6: Refactor inverted_index.rs, topic_catalog.rs, text_cache.rs**

Same pattern — replace collections, change method signatures.
- inverted_index: 3 HashMap fields → im::HashMap, 3 methods
- topic_catalog: 4 HashMap fields → im::HashMap, 6 methods
- text_cache: Vec + HashMap → im::Vector + im::HashMap, 4 methods (note: LruCache `get` returns `(Self, Option<String>)` since it updates access order)

**Step 7: Run tests**

Run: `cd simse-adaptive && cargo test --test fp_cataloging`
Expected: PASS

**Step 8: Commit**

```bash
git add simse-adaptive/src/cataloging.rs simse-adaptive/src/inverted_index.rs simse-adaptive/src/topic_catalog.rs simse-adaptive/src/text_cache.rs simse-adaptive/tests/fp_cataloging.rs simse-adaptive/Cargo.toml
git commit -m "refactor(simse-adaptive): FP cataloging, inverted index, topic catalog, LRU cache"
```

#### Task 4b: Graph module

**Files:**
- Modify: `simse-adaptive/src/graph.rs` (1,178 lines, 3 `&mut self`)

**Step 1: Write FP tests**

Create `simse-adaptive/tests/fp_graph.rs`:
```rust
#[test]
fn add_edge_returns_new_graph() {
    let graph = GraphIndex::new();
    let edge = Edge { from: "a".into(), to: "b".into(), weight: 1.0, edge_type: EdgeType::Explicit };
    let graph2 = graph.add_edge(edge);

    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph2.edge_count(), 1);
}

#[test]
fn remove_node_returns_new_graph() {
    let graph = GraphIndex::new();
    let edge = Edge { from: "a".into(), to: "b".into(), weight: 1.0, edge_type: EdgeType::Explicit };
    let graph2 = graph.add_edge(edge);
    let graph3 = graph2.remove_node("a");

    assert_eq!(graph3.edge_count(), 0);
    assert_eq!(graph2.edge_count(), 1);
}
```

**Step 2: Refactor GraphIndex**

Replace fields (lines 101-102):
```rust
pub struct GraphIndex {
    adjacency: im::HashMap<String, im::Vector<Edge>>,
    reverse: im::HashMap<String, im::Vector<Edge>>,
}
```

3 methods → owned-return.

**Step 3: Run tests, commit**

```bash
git commit -m "refactor(simse-adaptive): FP graph index"
```

#### Task 4c: Learning engine

**Files:**
- Modify: `simse-adaptive/src/learning.rs` (1,366 lines, 4 `&mut self`)

**Step 1: Write FP tests**

```rust
#[test]
fn record_feedback_returns_new_engine() {
    let engine = LearningEngine::new(Default::default());
    let engine2 = engine.record_feedback("entry1", true, 1000);
    // Original unchanged
    // New state has feedback recorded
}
```

**Step 2: Refactor**

Replace ~8 collection fields (lines 74-218) with `im` equivalents. 4 methods → owned-return.

**Step 3: Commit**

```bash
git commit -m "refactor(simse-adaptive): FP learning engine"
```

#### Task 4d: PCN layers, network, encoder, vocabulary

**Files:**
- Modify: `simse-adaptive/src/layer.rs` (738 lines, 5 `&mut self`)
- Modify: `simse-adaptive/src/network.rs` (828 lines, 7 `&mut self`)
- Modify: `simse-adaptive/src/encoder.rs` (370 lines, 2 `&mut self`)
- Modify: `simse-adaptive/src/vocabulary.rs` (412 lines, 2 `&mut self`)
- Modify: `simse-adaptive/src/snapshot.rs` (500 lines — Vec fields for weights)

**Step 1: Write FP tests for PcnLayer**

```rust
#[test]
fn predict_returns_new_layer_and_output() {
    let layer = PcnLayer::new(10, 5);
    let (layer2, output) = layer.predict(&input);
    // layer unchanged, layer2 has updated internal state
}
```

**Step 2: Refactor PcnLayer**

```rust
pub struct PcnLayer {
    values: im::Vector<f64>,  // was Vec<f64>
    errors: im::Vector<f64>,  // was Vec<f64>
    // weights stay as Vec<f64> with // PERF: comment for inner-loop dot products
}
```

Note: `predict`, `compute_errors`, `update_weights` return new layers. `// PERF:` exception: inner-loop dot product accumulators use `let mut sum: f64 = 0.0`.

**Step 3: Refactor PcnNetwork, encoder, vocabulary**

- PcnNetwork: `layers: im::Vector<PcnLayer>`, 7 methods → owned-return
- Encoder: `vocab_mut` removed, `encode` returns `(Self, Result<...>)`
- VocabularyManager: `topics/tags: im::Vector<String>`, maps → `im::HashMap`, 2 methods → owned-return

**Step 4: Commit**

```bash
git commit -m "refactor(simse-adaptive): FP PCN layers, network, encoder, vocabulary"
```

#### Task 4e: AdaptiveStore (main store)

**Files:**
- Modify: `simse-adaptive/src/store.rs` (2,402 lines, 12 `&mut self`)
- Modify: `simse-adaptive/src/persistence.rs` (1,027 lines — snapshot types)

**Step 1: Write FP tests**

```rust
#[test]
fn store_add_and_delete_immutable() {
    let store = AdaptiveStore::new();
    let (store2, id) = store.add("test", &[0.1, 0.2, 0.3], None, None).unwrap();

    assert_eq!(store.count(), 0);
    assert_eq!(store2.count(), 1);

    let (store3, deleted) = store2.delete(&id);
    assert!(deleted);
    assert_eq!(store3.count(), 0);
    assert_eq!(store2.count(), 1);
}
```

**Step 2: Refactor AdaptiveStore**

```rust
pub struct AdaptiveStore {
    volumes: im::Vector<Volume>,
    access_stats: im::HashMap<String, AccessStats>,
    topic_index: TopicIndex,       // already refactored in 4a
    metadata_index: MetadataIndex, // already refactored in 4a
    inverted_index: BM25Index,     // already refactored in 4a
    topic_catalog: TopicCatalog,   // already refactored in 4a
    magnitude_cache: MagnitudeCache,
    text_cache: LruCache,
    graph: GraphIndex,             // already refactored in 4b
    learning: LearningEngine,      // already refactored in 4c
    // ...
}
```

12 methods → owned-return. `initialize` and `save` use Effect:
```rust
pub enum AdaptiveEffect {
    None,
    SaveToDisk { path: String, snapshot: StorageSnapshot },
    LoadFromDisk { path: String },
}
```

**Step 3: Commit**

```bash
git commit -m "refactor(simse-adaptive): FP adaptive store with Effect enum"
```

#### Task 4f: Server dispatch

**Files:**
- Modify: `simse-adaptive/src/server.rs` (940 lines)

**Step 1: Refactor dispatch to pure functions**

```rust
fn dispatch(state: AdaptiveStore, method: &str, params: Value) -> (AdaptiveStore, Result<Value, AdaptiveError>)
```

**Step 2: Update integration tests**

Modify `simse-adaptive/tests/integration.rs` (1,242 lines) and `tests/pcn_integration.rs` (322 lines).

Run: `cd simse-adaptive && cargo test`
Expected: All ~200 tests PASS

**Step 3: Commit**

```bash
git commit -m "refactor(simse-adaptive): FP server dispatch, integration tests updated"
```

---

## Phase 1 Verification

**Run all Phase 1 crates:**
```bash
cd simse-vsh && cargo test
cd simse-vnet && cargo test
cd simse-vfs && cargo test
cd simse-adaptive && cargo test
```

All must pass before Phase 2.

---

## Phase 2: Composition Crates

### Task 5: simse-acp — ACP Engine (8,788 LOC, 2 `&mut self` methods)

This crate has very few `&mut self` methods but heavy `Arc<Mutex<HashMap>>` usage for concurrent state. The main refactor is converting internal collections to `im` and making the server dispatch pure.

**Files:**
- Modify: `simse-acp/Cargo.toml`
- Modify: `simse-acp/src/client.rs` (1,452 lines — connection pool, circuit breakers)
- Modify: `simse-acp/src/connection.rs` (1,453 lines — pending requests)
- Modify: `simse-acp/src/resilience.rs` (824 lines — circuit breaker, health monitor)
- Modify: `simse-acp/src/server.rs` (1,046 lines — dispatch)
- Modify: `simse-acp/src/stream.rs` (805 lines)
- Test: `simse-acp/tests/integration.rs` (735 lines)

**Step 1: Add `im` dependency**

**Step 2: Write FP tests**

Focus on AcpClient state and server dispatch:
```rust
#[test]
fn dispatch_returns_new_state() {
    let state = AcpServerState::default();
    let (state2, response) = dispatch(state.clone(), "health", Value::Null);
    // state unchanged, response contains health info
}
```

**Step 3: Refactor AcpClient collections**

```rust
// client.rs line 179-185 — replace HashMap with im::HashMap
pub struct AcpClient {
    connections: im::HashMap<String, Arc<AcpConnection>>,
    circuit_breakers: im::HashMap<String, CircuitBreaker>,
    health_monitors: im::HashMap<String, HealthMonitor>,
    // session_cache stays Arc<Mutex> because AcpConnection readers share it
}
```

Note: `AcpConnection` uses `Arc<Mutex<HashMap>>` for `pending` requests because the reader task and caller share state. These stay as `Arc<Mutex>` — they're infrastructure for async I/O, not business data.

**Step 4: Refactor server dispatch**

```rust
fn dispatch(state: AcpServerState, method: &str, params: Value) -> (AcpServerState, Result<Value, AcpError>)
```

With Effect enum for connection spawning:
```rust
pub enum AcpEffect {
    None,
    SpawnConnection { config: ConnectionConfig },
    SendToConnection { server: String, request: JsonRpcRequest },
    KillConnection { server: String },
}
```

**Step 5: Run tests, commit**

```bash
git add simse-acp/
git commit -m "refactor(simse-acp): FP dispatch, im collections for client state"
```

---

### Task 6: simse-mcp — MCP Engine (8,833 LOC, 13 `&mut self` methods)

**Files:**
- Modify: `simse-mcp/Cargo.toml`
- Modify: `simse-mcp/src/client.rs` (1,799 lines, 5 `&mut self`)
- Modify: `simse-mcp/src/mcp_server.rs` (1,245 lines, 8 `&mut self`)
- Modify: `simse-mcp/src/rpc_server.rs` (1,106 lines, 1 `&mut self`)
- Test: `simse-mcp/tests/integration.rs` (949 lines)

**Step 1: Add `im`, write FP tests**

```rust
#[test]
fn register_tool_returns_new_server() {
    let server = McpServer::new("test", "1.0");
    let server2 = server.register_tool(definition, handler);

    assert_eq!(server.list_tools().len(), 0);
    assert_eq!(server2.list_tools().len(), 1);
}
```

**Step 2: Refactor McpServer**

```rust
pub struct McpServer {
    tools: im::HashMap<String, RegisteredTool>,
    resources: im::HashMap<String, RegisteredResource>,
    prompts: im::HashMap<String, RegisteredPrompt>,
    roots: im::Vector<Root>,
}
```

8 methods → owned-return:
- `register_tool_fn(self, ...) -> Self`
- `unregister_tool(self, name) -> (Self, bool)`
- `register_resource_fn(self, ...) -> Self`
- `unregister_resource(self, uri) -> (Self, bool)`
- `register_prompt_fn(self, ...) -> Self`
- `unregister_prompt(self, name) -> (Self, bool)`
- `set_roots(self, roots) -> Self`

**Step 3: Refactor McpClient**

```rust
pub struct McpClient {
    connections: im::HashMap<String, ConnectedServer>,
    circuit_breakers: im::HashMap<String, CircuitBreaker>,
    health_monitors: im::HashMap<String, HealthMonitor>,
    roots: im::Vector<Root>,
}
```

5 methods → owned-return. Connect/disconnect use Effect:
```rust
pub enum McpEffect {
    None,
    SpawnStdioTransport { config: StdioTransportConfig },
    SpawnHttpTransport { config: HttpTransportConfig },
    DisconnectTransport { server: String },
}
```

**Step 4: Refactor rpc_server dispatch**

Pure `(State, Request) -> (State, Response)`.

**Step 5: Run tests, commit**

```bash
git add simse-mcp/
git commit -m "refactor(simse-mcp): FP client/server, im registries, Effect enum"
```

---

### Task 7: simse-sandbox — Unified Sandbox (5,532 LOC, ~21 `&mut self` methods)

**Files:**
- Modify: `simse-sandbox/Cargo.toml`
- Modify: `simse-sandbox/src/sandbox.rs` (490 lines, 6 `&mut self`)
- Modify: `simse-sandbox/src/server.rs` (2,228 lines, many private `&mut self`)
- Modify: `simse-sandbox/src/ssh/pool.rs` (280 lines)
- Modify: `simse-sandbox/src/ssh/fs_backend.rs` (664 lines)
- Modify: `simse-sandbox/src/ssh/shell_backend.rs` (211 lines)
- Modify: `simse-sandbox/src/ssh/net_backend.rs` (416 lines)
- Test: `simse-sandbox/tests/integration.rs` (427 lines)

**Step 1: Add `im`, write FP tests**

**Step 2: Refactor Sandbox orchestrator**

The Sandbox composes VFS + VSH + VNet (already refactored in Phase 1). Thread their new immutable state:

```rust
pub struct Sandbox {
    vfs: Option<VirtualFs>,       // already immutable from Task 3
    vsh: Option<VirtualShell>,    // already immutable from Task 1
    vnet: Option<VirtualNetwork>, // already immutable from Task 2
    config: BackendConfig,
}
```

Remove `vfs_mut`, `vsh_mut`, `vnet_mut` — callers get owned sub-state, do operation, return new Sandbox:

```rust
impl Sandbox {
    pub fn with_vfs<F, T>(self, f: F) -> Result<(Self, T), SandboxError>
    where F: FnOnce(VirtualFs) -> Result<(VirtualFs, T), VfsError>
    {
        let vfs = self.vfs.ok_or(SandboxError::not_initialized("vfs"))?;
        let (new_vfs, result) = f(vfs)?;
        Ok((Self { vfs: Some(new_vfs), ..self }, result))
    }
}
```

**Step 3: Refactor server dispatch**

63 methods become pure functions. Biggest change — all VFS/VSH/VNet handlers call `state.with_vfs(|vfs| vfs.write_file(...))` pattern.

**Step 4: SSH backends**

SSH backends use Effect enum for russh channel operations (inherently async I/O):
```rust
pub enum SshEffect {
    SftpRead { path: String },
    SftpWrite { path: String, content: Vec<u8> },
    ExecCommand { command: String },
    // ...
}
```

**Step 5: Run tests, commit**

```bash
git add simse-sandbox/
git commit -m "refactor(simse-sandbox): FP orchestrator, pure dispatch, SSH effects"
```

---

## Phase 2 Verification

```bash
cd simse-acp && cargo test
cd simse-mcp && cargo test
cd simse-sandbox && cargo test
```

All must pass before Phase 3.

---

## Phase 3: Orchestration

### Task 8: simse-core — Core Orchestration (18,030 LOC, 21 `&mut self` methods)

Break into sub-tasks by module.

#### Task 8a: Conversation

**Files:**
- Modify: `simse-core/Cargo.toml`
- Modify: `simse-core/src/conversation.rs` (325 lines, 9 `&mut self`)
- Test: `simse-core/tests/conversation.rs` (307 lines)

**Step 1: Add `im`, write FP tests**

```rust
#[test]
fn add_user_returns_new_conversation() {
    let conv = Conversation::new(Default::default());
    let conv2 = conv.add_user("hello");

    assert_eq!(conv.messages().len(), 0);
    assert_eq!(conv2.messages().len(), 1);
}

#[test]
fn compact_returns_new_conversation() {
    let conv = Conversation::new(Default::default());
    let conv2 = conv.add_user("msg1").add_assistant("reply1").add_user("msg2");
    let conv3 = conv2.compact("summary of conversation");

    assert!(conv3.messages().len() < conv2.messages().len());
}
```

**Step 2: Refactor**

```rust
pub struct Conversation {
    messages: im::Vector<ConversationMessage>,
    system_prompt: Option<String>,
    config: ConversationConfig,
}
```

9 methods → owned-return. Chain-friendly API:
```rust
impl Conversation {
    pub fn add_user(self, content: &str) -> Self { ... }
    pub fn add_assistant(self, content: &str) -> Self { ... }
    pub fn compact(self, summary: &str) -> Self { ... }
    pub fn clear(self) -> Self { ... }
}
```

**Step 3: Commit**

```bash
git commit -m "refactor(simse-core): FP Conversation — immutable messages"
```

#### Task 8b: TaskList

**Files:**
- Modify: `simse-core/src/tasks.rs` (432 lines, 3 `&mut self`)
- Test: `simse-core/tests/tasks.rs` (849 lines)

**Step 1: Write FP tests, refactor**

```rust
pub struct TaskList {
    tasks: im::HashMap<String, TaskItem>,
}
```

3 methods → owned-return:
- `create(self, input) -> (Self, TaskItem)`
- `delete(self, id) -> (Self, bool)`
- `clear(self) -> Self`

**Step 2: Commit**

```bash
git commit -m "refactor(simse-core): FP TaskList"
```

#### Task 8c: ToolRegistry

**Files:**
- Modify: `simse-core/src/tools/registry.rs` (462 lines, 2 `&mut self`)

**Step 1: Refactor**

```rust
pub struct ToolRegistry {
    tools: im::HashMap<String, RegisteredTool>,
    metrics: im::HashMap<String, MetricsEntry>, // was Mutex<HashMap> — no longer needed since state is threaded
}
```

- `register(self, ...) -> Self`
- `unregister(self, name) -> (Self, bool)`
- `execute` stays `&self` (pure query) but returns `(Self, ToolResult)` with updated metrics

**Step 2: Commit**

```bash
git commit -m "refactor(simse-core): FP ToolRegistry — removed Mutex"
```

#### Task 8d: Chain, Hooks, Events, SessionManager

**Files:**
- Modify: `simse-core/src/chain/chain.rs` (914 lines, 4 `&mut self`)
- Modify: `simse-core/src/hooks.rs` (525 lines)
- Modify: `simse-core/src/events.rs` (263 lines)
- Modify: `simse-core/src/server/session.rs` (254 lines)

**Step 1: Refactor Chain**

```rust
// Builder methods return Self
pub fn set_name(self, name: impl Into<String>) -> Self
pub fn add_step(self, step: ChainStepConfig) -> Result<Self, SimseError>
pub fn clear(self) -> Self
```

**Step 2: Refactor HookSystem, EventBus, SessionManager**

All use `im` collections internally, owned-return pattern.

EventBus: `publish` returns `(Self, Vec<Effect>)` where Effect represents notifications to fire.

SessionManager: `create/fork/delete` return `(Self, Session)` or `(Self, bool)`.

**Step 3: Commit**

```bash
git commit -m "refactor(simse-core): FP chain, hooks, events, sessions"
```

#### Task 8e: CoreContext and RPC Server

**Files:**
- Modify: `simse-core/src/context.rs` (73 lines)
- Modify: `simse-core/src/rpc_server.rs` (2,833 lines, 2 `&mut self`)
- Modify: `simse-core/src/agentic_loop.rs` (1,021 lines, 1 `&mut self`)

**Step 1: Refactor CoreContext**

CoreContext composes all the above. Each field is now immutable; operations return new CoreContext:

```rust
pub struct CoreContext {
    pub conversation: Conversation,
    pub tasks: TaskList,
    pub tools: ToolRegistry,
    pub hooks: HookSystem,
    pub events: EventBus,
    pub sessions: SessionManager,
    pub config: AppConfig,
    pub logger: Logger,
}
```

**Step 2: Refactor RPC server dispatch**

48 methods become pure functions:
```rust
fn dispatch(state: CoreContext, method: &str, params: Value) -> (CoreContext, Result<Value, SimseError>)
```

**Step 3: Refactor AgenticLoop**

`run_agentic_loop` becomes a pure step function:
```rust
fn agentic_step(state: LoopState, response: GenerateResponse) -> (LoopState, Vec<AgenticEffect>)
```

The async loop shell interprets effects (generate, tool execute, etc.).

**Step 4: Update all 31 test files**

Run: `cd simse-core && cargo test`
Expected: All 779+ tests PASS

**Step 5: Commit**

```bash
git commit -m "refactor(simse-core): FP context, RPC dispatch, agentic loop"
```

---

## Phase 3 Verification

```bash
cd simse-core && cargo test
```

All 779+ tests must pass.

---

## Phase 4: UI Layer

### Task 9: simse-ui-core (6,239 LOC, ~31 `&mut self` methods)

**Files:**
- Modify: `simse-ui-core/Cargo.toml`
- Modify: `simse-ui-core/src/state/conversation.rs` (503 lines, 7 `&mut self`)
- Modify: `simse-ui-core/src/state/permission_manager.rs` (858 lines, 6 `&mut self`)
- Modify: `simse-ui-core/src/input/keybindings.rs` (368 lines, 2 `&mut self`)
- Modify: `simse-ui-core/src/config/settings_state.rs` (857 lines, 16 `&mut self`)
- Test: `simse-ui-core/tests/integration.rs` (440 lines)

**Step 1: Add `im`, write FP tests**

ConversationBuffer wraps simse-core's Conversation (already refactored in Task 8a). PermissionManager and SettingsFormState are pure state machines — natural fit.

```rust
#[test]
fn settings_move_down_returns_new_state() {
    let state = SettingsFormState::new();
    let state2 = state.move_down();

    assert_eq!(state.selected_index(), 0);
    assert_eq!(state2.selected_index(), 1);
}
```

**Step 2: Refactor all modules**

All 31 methods change from `&mut self` to `self -> Self` or `self -> (Self, Action)`.

SettingsFormState already returns `SettingsAction` — just change `&mut self` to `self`:
```rust
pub fn enter(self) -> (Self, SettingsAction) { ... }
pub fn toggle(self) -> (Self, SettingsAction) { ... }
```

**Step 3: Commit**

```bash
git add simse-ui-core/
git commit -m "refactor(simse-ui-core): FP state machines — owned-return for all UI state"
```

---

### Task 10: simse-tui (21,907 LOC, ~50+ `&mut self` methods)

**Files:**
- Modify: `simse-tui/Cargo.toml`
- Modify: `simse-tui/src/app.rs` — App model
- Modify: `simse-tui/src/event_loop.rs` (1,700 lines)
- Modify: `simse-tui/src/autocomplete.rs` (200 lines, 6 `&mut self`)
- Modify: `simse-tui/src/at_mention.rs` (250 lines, 6 `&mut self`)
- Modify: `simse-tui/src/spinner.rs` (150 lines, 2 `&mut self`)
- Modify: `simse-tui/src/dialogs/permission.rs` (634 lines)
- Modify: `simse-tui/src/dialogs/confirm.rs` (637 lines)
- Modify: `simse-tui/src/overlays/settings.rs` (867 lines)
- Modify: `simse-tui/src/overlays/librarian.rs` (1,590 lines, 10 `&mut self`)
- Modify: `simse-tui/src/overlays/ollama_wizard.rs` (973 lines, 8 `&mut self`)
- Modify: `simse-tui/src/overlays/setup.rs` (918 lines, 7 `&mut self`)
- Tests: 21 files (4,372 lines)

**Step 1: Add `im`, write FP tests for small components first**

```rust
#[test]
fn autocomplete_activate_returns_new_state() {
    let ac = Autocomplete::new();
    let ac2 = ac.activate("/he", &commands);

    assert!(!ac.is_active());
    assert!(ac2.is_active());
}

#[test]
fn spinner_tick_returns_new_spinner() {
    let s = Spinner::new();
    let (s2, changed) = s.tick();
    // s unchanged, s2 has advanced frame
}
```

**Step 2: Refactor small components**

Autocomplete, AtMention, Spinner — 14 methods total, all `self -> Self` or `self -> (Self, T)`.

**Step 3: Refactor dialogs**

Permission and Confirm dialogs — navigation methods return new state.

**Step 4: Refactor overlays**

Librarian, OllamaWizard, Setup — 25 methods → owned-return. Settings overlay already delegates to SettingsFormState (refactored in Task 9).

**Step 5: Refactor App model and event loop**

The TUI already uses Elm Architecture. Make it explicit:
```rust
// App update returns new App
fn update(app: App, msg: AppMessage) -> (App, Vec<TuiEffect>)

// Event loop — single mut binding
async fn run(mut app: App) {
    loop {
        let event = next_event().await;
        let msg = event_to_message(event);
        let (new_app, effects) = update(app, msg);
        app = new_app;
        for effect in effects {
            interpret_effect(&app, effect).await;
        }
        render(&app);
    }
}
```

**Step 6: Update all 21 test files**

Run: `cd simse-tui && cargo test`
Expected: All tests PASS

**Step 7: Commit**

```bash
git add simse-tui/
git commit -m "refactor(simse-tui): FP Elm Architecture — pure update, Effect interpretation"
```

---

## Phase 4 Verification

```bash
cd simse-ui-core && cargo test
cd simse-tui && cargo test
```

---

## Final Verification

### Task 11: Full workspace build and test

**Step 1: Build all crates**

```bash
cargo build --release
```

Expected: Clean build, no warnings.

**Step 2: Run all tests**

```bash
cd simse-adaptive && cargo test
cd simse-vfs && cargo test
cd simse-acp && cargo test
cd simse-mcp && cargo test
cd simse-vsh && cargo test
cd simse-vnet && cargo test
cd simse-core && cargo test
cd simse-ui-core && cargo test
cd simse-tui && cargo test
cd simse-sandbox && cargo test
```

Expected: All ~1,500+ tests PASS across all crates.

**Step 3: Verify no `&mut self` remains (except PERF exceptions)**

```bash
rg "&mut self" simse-*/src/ --type rust -c
```

Expected: Only hits in files with documented `// PERF:` exceptions (PCN layer dot products, tokio I/O handles).

**Step 4: Commit any final cleanup**

```bash
git commit -m "chore: final FP refactor verification — all tests passing"
```

---

## Summary

| Phase | Tasks | Crates | Est. Methods Changed |
|-------|-------|--------|---------------------|
| 1 | 1-4 (4a-4f) | vsh, vnet, vfs, adaptive | ~109 |
| 2 | 5-7 | acp, mcp, sandbox | ~36 |
| 3 | 8 (8a-8e) | core | ~21 |
| 4 | 9-10 | ui-core, tui | ~81 |
| Final | 11 | all | verification |
| **Total** | **11 tasks** | **10 crates** | **~247 methods** |
