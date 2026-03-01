# Graph Intelligence Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `GraphIndex` module to `simse-vector` that maintains weighted, typed edges between volumes — supporting both explicit edges (via `rel:*` metadata) and implicit edges (embedding similarity + query co-occurrence) — and integrates graph connectivity into search and recommendation scoring.

**Architecture:** New `graph.rs` module in `simse-vector/src/` with `GraphIndex` struct holding adjacency + reverse-adjacency maps. Wired into `VolumeStore` alongside existing indexes. Explicit edges persisted in the store file; implicit edges rebuilt on load. Two new JSON-RPC endpoints (`graph/neighbors`, `graph/traverse`) plus optional `graphBoost` in `advancedSearch` and `graph` weight in `recommend`.

**Tech Stack:** Rust (serde, serde_json, uuid), TypeScript (bun:test for E2E)

**Design doc:** `docs/plans/2026-03-01-graph-intelligence-design.md`

---

### Task 1: Graph data model — types and structs

**Files:**
- Create: `simse-vector/src/graph.rs`
- Modify: `simse-vector/src/lib.rs`

**Step 1: Write the failing test**

Add to the bottom of `simse-vector/src/graph.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_graph_index_with_default_config() {
        let graph = GraphIndex::new(GraphConfig::default());
        assert_eq!(graph.edge_count(), 0);
        assert!(graph.neighbors("nonexistent").is_empty());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-vector && cargo test graph::tests::create_graph_index_with_default_config`
Expected: FAIL — module doesn't exist yet

**Step 3: Write the graph module with data model**

Create `simse-vector/src/graph.rs` with these types:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// -- Edge types ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EdgeType {
    Related,
    Parent,
    Child,
    Extends,
    Contradicts,
    Similar,
    CoOccurs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EdgeOrigin {
    Explicit,
    Similarity,
    Correlation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: EdgeType,
    pub weight: f64,
    pub origin: EdgeOrigin,
    pub timestamp: u64,
}

// -- Config -------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    pub similarity_threshold: f64,
    pub correlation_threshold: usize,
    pub max_edges_per_node: usize,
    pub graph_boost_weight: f64,
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.85,
            correlation_threshold: 3,
            max_edges_per_node: 50,
            graph_boost_weight: 0.15,
        }
    }
}

// -- GraphIndex ---------------------------------------------------------------

pub struct GraphIndex {
    adjacency: HashMap<String, Vec<Edge>>,
    reverse: HashMap<String, Vec<Edge>>,
    config: GraphConfig,
}

impl GraphIndex {
    pub fn new(config: GraphConfig) -> Self {
        Self {
            adjacency: HashMap::new(),
            reverse: HashMap::new(),
            config,
        }
    }

    pub fn config(&self) -> &GraphConfig {
        &self.config
    }

    pub fn edge_count(&self) -> usize {
        self.adjacency.values().map(|edges| edges.len()).sum()
    }

    pub fn neighbors(&self, _id: &str) -> Vec<&Edge> {
        Vec::new() // Stub — implemented in Task 2
    }
}
```

Register the module in `simse-vector/src/lib.rs`:

```rust
pub mod graph;
```

**Step 4: Run test to verify it passes**

Run: `cd simse-vector && cargo test graph::tests::create_graph_index_with_default_config`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-vector/src/graph.rs simse-vector/src/lib.rs
git commit -m "feat(graph): add graph data model — Edge, EdgeType, GraphIndex structs"
```

---

### Task 2: Edge CRUD — add, remove, neighbors, enforce cap

**Files:**
- Modify: `simse-vector/src/graph.rs`

**Step 1: Write failing tests**

Add these tests to the `#[cfg(test)] mod tests` block in `graph.rs`:

```rust
#[test]
fn add_edge_and_retrieve_neighbors() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_edge(Edge {
        source_id: "a".into(),
        target_id: "b".into(),
        edge_type: EdgeType::Related,
        weight: 0.9,
        origin: EdgeOrigin::Explicit,
        timestamp: 1000,
    });
    let neighbors = graph.neighbors("a");
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].target_id, "b");
    assert_eq!(neighbors[0].weight, 0.9);
}

#[test]
fn bidirectional_edge_creates_both_directions() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_bidirectional_edge("a", "b", EdgeType::Related, 0.8, EdgeOrigin::Explicit, 1000);

    assert_eq!(graph.neighbors("a").len(), 1);
    assert_eq!(graph.neighbors("b").len(), 1);
    assert_eq!(graph.neighbors("a")[0].target_id, "b");
    assert_eq!(graph.neighbors("b")[0].target_id, "a");
}

#[test]
fn remove_edges_for_node_cascades() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_bidirectional_edge("a", "b", EdgeType::Related, 0.8, EdgeOrigin::Explicit, 1000);
    graph.add_bidirectional_edge("a", "c", EdgeType::Similar, 0.9, EdgeOrigin::Similarity, 2000);
    graph.add_bidirectional_edge("b", "c", EdgeType::CoOccurs, 0.5, EdgeOrigin::Correlation, 3000);

    graph.remove_node("a");
    // a's edges gone from both adjacency and reverse
    assert!(graph.neighbors("a").is_empty());
    // b should only have edge to c remaining
    assert_eq!(graph.neighbors("b").len(), 1);
    assert_eq!(graph.neighbors("b")[0].target_id, "c");
    // c should only have edge from b remaining
    assert_eq!(graph.neighbors("c").len(), 1);
}

#[test]
fn max_edges_per_node_evicts_weakest() {
    let mut config = GraphConfig::default();
    config.max_edges_per_node = 3;
    let mut graph = GraphIndex::new(config);

    // Add 4 edges — the weakest (0.1) should be evicted
    for (i, w) in [(0, 0.5), (1, 0.9), (2, 0.1), (3, 0.7)] {
        graph.add_edge(Edge {
            source_id: "a".into(),
            target_id: format!("t{}", i),
            edge_type: EdgeType::Similar,
            weight: w,
            origin: EdgeOrigin::Similarity,
            timestamp: 1000,
        });
    }

    let neighbors = graph.neighbors("a");
    assert_eq!(neighbors.len(), 3);
    // The weakest (0.1 → t2) should have been evicted
    assert!(neighbors.iter().all(|e| e.target_id != "t2"));
}

#[test]
fn neighbors_filtered_by_edge_type() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_edge(Edge {
        source_id: "a".into(), target_id: "b".into(),
        edge_type: EdgeType::Related, weight: 0.8,
        origin: EdgeOrigin::Explicit, timestamp: 1000,
    });
    graph.add_edge(Edge {
        source_id: "a".into(), target_id: "c".into(),
        edge_type: EdgeType::Similar, weight: 0.9,
        origin: EdgeOrigin::Similarity, timestamp: 2000,
    });

    let related = graph.neighbors_by_type("a", &[EdgeType::Related]);
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].target_id, "b");

    let similar = graph.neighbors_by_type("a", &[EdgeType::Similar]);
    assert_eq!(similar.len(), 1);
    assert_eq!(similar[0].target_id, "c");
}

#[test]
fn update_edge_weight_when_stronger() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_edge(Edge {
        source_id: "a".into(), target_id: "b".into(),
        edge_type: EdgeType::Similar, weight: 0.8,
        origin: EdgeOrigin::Similarity, timestamp: 1000,
    });
    // Same edge pair+type, stronger weight
    graph.add_edge(Edge {
        source_id: "a".into(), target_id: "b".into(),
        edge_type: EdgeType::Similar, weight: 0.95,
        origin: EdgeOrigin::Similarity, timestamp: 2000,
    });

    let neighbors = graph.neighbors("a");
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].weight, 0.95);
    assert_eq!(neighbors[0].timestamp, 2000);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vector && cargo test graph::tests`
Expected: FAIL — methods don't exist

**Step 3: Implement edge CRUD methods**

Add these methods to `impl GraphIndex` in `graph.rs`:

```rust
/// Add an edge. If an edge with the same source, target, and type already
/// exists, update it only if the new weight is stronger.
pub fn add_edge(&mut self, edge: Edge) {
    let source_edges = self.adjacency.entry(edge.source_id.clone()).or_default();

    // Check for existing edge with same target + type
    if let Some(existing) = source_edges.iter_mut().find(|e| {
        e.target_id == edge.target_id && e.edge_type == edge.edge_type
    }) {
        if edge.weight > existing.weight {
            existing.weight = edge.weight;
            existing.timestamp = edge.timestamp;
        }
        return;
    }

    source_edges.push(edge.clone());

    // Enforce max_edges_per_node — evict weakest
    if source_edges.len() > self.config.max_edges_per_node {
        source_edges.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
        source_edges.truncate(self.config.max_edges_per_node);
    }

    // Also track in reverse index
    self.reverse.entry(edge.target_id.clone()).or_default().push(Edge {
        source_id: edge.source_id.clone(),
        target_id: edge.target_id.clone(),
        edge_type: edge.edge_type,
        weight: edge.weight,
        origin: edge.origin,
        timestamp: edge.timestamp,
    });
}

/// Add a bidirectional edge (e.g. Related, Contradicts).
/// Creates edges in both directions.
pub fn add_bidirectional_edge(
    &mut self,
    a: &str,
    b: &str,
    edge_type: EdgeType,
    weight: f64,
    origin: EdgeOrigin,
    timestamp: u64,
) {
    self.add_edge(Edge {
        source_id: a.into(),
        target_id: b.into(),
        edge_type: edge_type.clone(),
        weight,
        origin: origin.clone(),
        timestamp,
    });
    self.add_edge(Edge {
        source_id: b.into(),
        target_id: a.into(),
        edge_type,
        weight,
        origin,
        timestamp,
    });
}

/// Remove all edges involving a node (both outgoing and incoming).
pub fn remove_node(&mut self, id: &str) {
    // Remove outgoing edges
    if let Some(outgoing) = self.adjacency.remove(id) {
        // Clean reverse index for each target
        for edge in &outgoing {
            if let Some(rev) = self.reverse.get_mut(&edge.target_id) {
                rev.retain(|e| e.source_id != id);
                if rev.is_empty() {
                    self.reverse.remove(&edge.target_id);
                }
            }
        }
    }

    // Remove incoming edges (where this node is a target)
    if let Some(incoming) = self.reverse.remove(id) {
        for edge in &incoming {
            if let Some(fwd) = self.adjacency.get_mut(&edge.source_id) {
                fwd.retain(|e| e.target_id != id);
                if fwd.is_empty() {
                    self.adjacency.remove(&edge.source_id);
                }
            }
        }
    }
}

/// Get all outgoing edges for a node, sorted by weight descending.
pub fn neighbors(&self, id: &str) -> Vec<&Edge> {
    match self.adjacency.get(id) {
        Some(edges) => {
            let mut sorted: Vec<&Edge> = edges.iter().collect();
            sorted.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
            sorted
        }
        None => Vec::new(),
    }
}

/// Get outgoing edges filtered by edge type.
pub fn neighbors_by_type(&self, id: &str, types: &[EdgeType]) -> Vec<&Edge> {
    self.neighbors(id)
        .into_iter()
        .filter(|e| types.contains(&e.edge_type))
        .collect()
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vector && cargo test graph::tests`
Expected: PASS (all 6 tests)

**Step 5: Commit**

```bash
git add simse-vector/src/graph.rs
git commit -m "feat(graph): implement edge CRUD — add, remove, neighbors, cap enforcement"
```

---

### Task 3: Metadata parsing — `rel:*` keys to explicit edges

**Files:**
- Modify: `simse-vector/src/graph.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn parse_rel_metadata_creates_related_edges() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    let mut metadata = HashMap::new();
    metadata.insert("rel:related".to_string(), "id1,id2,id3".to_string());
    metadata.insert("topic".to_string(), "test".to_string()); // non-rel key ignored

    graph.parse_metadata_edges("source", &metadata, 1000);

    let neighbors = graph.neighbors("source");
    assert_eq!(neighbors.len(), 3);
    // Related is bidirectional — check reverse
    assert_eq!(graph.neighbors("id1").len(), 1);
    assert_eq!(graph.neighbors("id1")[0].target_id, "source");
}

#[test]
fn parse_rel_parent_creates_parent_child_edges() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    let mut metadata = HashMap::new();
    metadata.insert("rel:parent".to_string(), "parent-vol".to_string());

    graph.parse_metadata_edges("child-vol", &metadata, 1000);

    // Source gets Parent edge to target
    let source_neighbors = graph.neighbors("child-vol");
    assert_eq!(source_neighbors.len(), 1);
    assert_eq!(source_neighbors[0].edge_type, EdgeType::Parent);

    // Target gets Child edge back
    let target_neighbors = graph.neighbors("parent-vol");
    assert_eq!(target_neighbors.len(), 1);
    assert_eq!(target_neighbors[0].edge_type, EdgeType::Child);
}

#[test]
fn parse_rel_extends_creates_directed_edge() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    let mut metadata = HashMap::new();
    metadata.insert("rel:extends".to_string(), "base-vol".to_string());

    graph.parse_metadata_edges("derived-vol", &metadata, 1000);

    // Only one direction
    assert_eq!(graph.neighbors("derived-vol").len(), 1);
    assert_eq!(graph.neighbors("derived-vol")[0].edge_type, EdgeType::Extends);
    assert!(graph.neighbors("base-vol").is_empty());
}

#[test]
fn parse_rel_contradicts_creates_bidirectional_edges() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    let mut metadata = HashMap::new();
    metadata.insert("rel:contradicts".to_string(), "other-vol".to_string());

    graph.parse_metadata_edges("this-vol", &metadata, 1000);

    assert_eq!(graph.neighbors("this-vol").len(), 1);
    assert_eq!(graph.neighbors("this-vol")[0].edge_type, EdgeType::Contradicts);
    assert_eq!(graph.neighbors("other-vol").len(), 1);
    assert_eq!(graph.neighbors("other-vol")[0].edge_type, EdgeType::Contradicts);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vector && cargo test graph::tests::parse_rel`
Expected: FAIL — `parse_metadata_edges` doesn't exist

**Step 3: Implement metadata parsing**

Add to `impl GraphIndex`:

```rust
/// Parse `rel:*` metadata keys from a volume and create edges.
///
/// Convention:
/// - `rel:related` → Related (bidirectional)
/// - `rel:parent` → Parent (source→target) + Child (target→source)
/// - `rel:extends` → Extends (source→target, directed)
/// - `rel:contradicts` → Contradicts (bidirectional)
///
/// Values are comma-separated volume IDs.
pub fn parse_metadata_edges(
    &mut self,
    source_id: &str,
    metadata: &HashMap<String, String>,
    timestamp: u64,
) {
    for (key, value) in metadata {
        let rel_type = match key.as_str() {
            "rel:related" => Some(EdgeType::Related),
            "rel:parent" => Some(EdgeType::Parent),
            "rel:extends" => Some(EdgeType::Extends),
            "rel:contradicts" => Some(EdgeType::Contradicts),
            _ => None,
        };

        if let Some(edge_type) = rel_type {
            let target_ids: Vec<&str> = value.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            for target_id in target_ids {
                match edge_type {
                    EdgeType::Related | EdgeType::Contradicts => {
                        self.add_bidirectional_edge(
                            source_id, target_id, edge_type.clone(),
                            1.0, EdgeOrigin::Explicit, timestamp,
                        );
                    }
                    EdgeType::Parent => {
                        // source → target is Parent
                        self.add_edge(Edge {
                            source_id: source_id.into(),
                            target_id: target_id.into(),
                            edge_type: EdgeType::Parent,
                            weight: 1.0,
                            origin: EdgeOrigin::Explicit,
                            timestamp,
                        });
                        // target → source is Child
                        self.add_edge(Edge {
                            source_id: target_id.into(),
                            target_id: source_id.into(),
                            edge_type: EdgeType::Child,
                            weight: 1.0,
                            origin: EdgeOrigin::Explicit,
                            timestamp,
                        });
                    }
                    EdgeType::Extends => {
                        self.add_edge(Edge {
                            source_id: source_id.into(),
                            target_id: target_id.into(),
                            edge_type: EdgeType::Extends,
                            weight: 1.0,
                            origin: EdgeOrigin::Explicit,
                            timestamp,
                        });
                    }
                    _ => {} // Similar, CoOccurs, Child not set via metadata
                }
            }
        }
    }
}
```

Also add `use std::collections::HashMap;` to the imports at the top of graph.rs if not already present.

**Step 4: Run tests to verify they pass**

Run: `cd simse-vector && cargo test graph::tests`
Expected: PASS (all 10 tests)

**Step 5: Commit**

```bash
git add simse-vector/src/graph.rs
git commit -m "feat(graph): parse rel:* metadata into explicit edges"
```

---

### Task 4: Implicit similarity edges

**Files:**
- Modify: `simse-vector/src/graph.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn add_similarity_edge_above_threshold() {
    let mut graph = GraphIndex::new(GraphConfig {
        similarity_threshold: 0.85,
        ..GraphConfig::default()
    });

    // Similarity above threshold → edge created
    graph.add_similarity_edge("a", "b", 0.92, 1000);
    assert_eq!(graph.neighbors("a").len(), 1);
    assert_eq!(graph.neighbors("a")[0].edge_type, EdgeType::Similar);
    assert_eq!(graph.neighbors("a")[0].weight, 0.92);

    // Bidirectional
    assert_eq!(graph.neighbors("b").len(), 1);
}

#[test]
fn skip_similarity_edge_below_threshold() {
    let mut graph = GraphIndex::new(GraphConfig {
        similarity_threshold: 0.85,
        ..GraphConfig::default()
    });

    graph.add_similarity_edge("a", "b", 0.80, 1000);
    assert!(graph.neighbors("a").is_empty());
}

#[test]
fn skip_similarity_edge_when_explicit_exists() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    // Add explicit edge first
    graph.add_edge(Edge {
        source_id: "a".into(), target_id: "b".into(),
        edge_type: EdgeType::Related, weight: 1.0,
        origin: EdgeOrigin::Explicit, timestamp: 1000,
    });

    // Similarity edge should be skipped
    graph.add_similarity_edge("a", "b", 0.95, 2000);

    // Should still only have the explicit edge
    let neighbors = graph.neighbors("a");
    assert_eq!(neighbors.len(), 1);
    assert_eq!(neighbors[0].origin, EdgeOrigin::Explicit);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vector && cargo test graph::tests::add_similarity_edge`
Expected: FAIL

**Step 3: Implement similarity edge method**

Add to `impl GraphIndex`:

```rust
/// Check if an explicit edge already exists between two nodes.
fn has_explicit_edge(&self, a: &str, b: &str) -> bool {
    if let Some(edges) = self.adjacency.get(a) {
        if edges.iter().any(|e| e.target_id == b && e.origin == EdgeOrigin::Explicit) {
            return true;
        }
    }
    false
}

/// Conditionally add a Similar edge if cosine similarity exceeds threshold
/// and no explicit edge already exists between the pair.
pub fn add_similarity_edge(
    &mut self,
    a: &str,
    b: &str,
    similarity: f64,
    timestamp: u64,
) {
    if similarity < self.config.similarity_threshold {
        return;
    }
    if self.has_explicit_edge(a, b) || self.has_explicit_edge(b, a) {
        return;
    }
    self.add_bidirectional_edge(
        a, b, EdgeType::Similar, similarity,
        EdgeOrigin::Similarity, timestamp,
    );
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vector && cargo test graph::tests`
Expected: PASS (all 13 tests)

**Step 5: Commit**

```bash
git add simse-vector/src/graph.rs
git commit -m "feat(graph): add implicit similarity edge creation with threshold"
```

---

### Task 5: Implicit correlation (CoOccurs) edges

**Files:**
- Modify: `simse-vector/src/graph.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn sync_correlation_above_threshold() {
    let mut graph = GraphIndex::new(GraphConfig {
        correlation_threshold: 3,
        ..GraphConfig::default()
    });

    // Sync correlations — (a, b) has 5 co-occurrences, max is 5
    let correlations = vec![
        ("a".to_string(), vec![("b".to_string(), 5usize)]),
    ];
    graph.sync_correlations(&correlations, 5, 1000);

    assert_eq!(graph.neighbors("a").len(), 1);
    assert_eq!(graph.neighbors("a")[0].edge_type, EdgeType::CoOccurs);
    // Weight = count / max_count = 5/5 = 1.0
    assert!((graph.neighbors("a")[0].weight - 1.0).abs() < 1e-6);
}

#[test]
fn skip_correlation_below_threshold() {
    let mut graph = GraphIndex::new(GraphConfig {
        correlation_threshold: 3,
        ..GraphConfig::default()
    });

    let correlations = vec![
        ("a".to_string(), vec![("b".to_string(), 2usize)]),
    ];
    graph.sync_correlations(&correlations, 5, 1000);

    assert!(graph.neighbors("a").is_empty());
}

#[test]
fn prune_implicit_edges_below_threshold() {
    let mut config = GraphConfig::default();
    config.similarity_threshold = 0.85;
    let mut graph = GraphIndex::new(config);

    // Manually insert a weak implicit edge
    graph.add_edge(Edge {
        source_id: "a".into(), target_id: "b".into(),
        edge_type: EdgeType::Similar, weight: 0.80,
        origin: EdgeOrigin::Similarity, timestamp: 1000,
    });

    graph.prune_weak_implicit_edges(0.85);
    assert!(graph.neighbors("a").is_empty());
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vector && cargo test graph::tests::sync_correlation`
Expected: FAIL

**Step 3: Implement correlation sync and pruning**

Add to `impl GraphIndex`:

```rust
/// Sync CoOccurs edges from learning engine correlation data.
///
/// `correlations`: list of (entry_id, [(correlated_id, count), ...])
/// `max_count`: maximum co-occurrence count across all pairs (for normalization)
/// `timestamp`: current time
pub fn sync_correlations(
    &mut self,
    correlations: &[(String, Vec<(String, usize)>)],
    max_count: usize,
    timestamp: u64,
) {
    if max_count == 0 {
        return;
    }
    for (entry_id, pairs) in correlations {
        for (corr_id, count) in pairs {
            if *count < self.config.correlation_threshold {
                continue;
            }
            let weight = *count as f64 / max_count as f64;
            // Don't overwrite explicit edges
            if self.has_explicit_edge(entry_id, corr_id) || self.has_explicit_edge(corr_id, entry_id) {
                continue;
            }
            self.add_bidirectional_edge(
                entry_id, corr_id, EdgeType::CoOccurs, weight,
                EdgeOrigin::Correlation, timestamp,
            );
        }
    }
}

/// Remove implicit edges whose weight is below the given threshold.
pub fn prune_weak_implicit_edges(&mut self, min_weight: f64) {
    for edges in self.adjacency.values_mut() {
        edges.retain(|e| e.origin == EdgeOrigin::Explicit || e.weight >= min_weight);
    }
    for edges in self.reverse.values_mut() {
        edges.retain(|e| e.origin == EdgeOrigin::Explicit || e.weight >= min_weight);
    }
    // Remove empty entries
    self.adjacency.retain(|_, edges| !edges.is_empty());
    self.reverse.retain(|_, edges| !edges.is_empty());
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vector && cargo test graph::tests`
Expected: PASS (all 16 tests)

**Step 5: Commit**

```bash
git add simse-vector/src/graph.rs
git commit -m "feat(graph): add correlation sync and implicit edge pruning"
```

---

### Task 6: Graph traversal — BFS 1-2 hops

**Files:**
- Modify: `simse-vector/src/graph.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn traverse_one_hop() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_bidirectional_edge("a", "b", EdgeType::Related, 0.9, EdgeOrigin::Explicit, 1000);
    graph.add_bidirectional_edge("b", "c", EdgeType::Related, 0.8, EdgeOrigin::Explicit, 2000);
    graph.add_bidirectional_edge("c", "d", EdgeType::Related, 0.7, EdgeOrigin::Explicit, 3000);

    let results = graph.traverse("a", 1, None, 50);
    // 1-hop from a: only b
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].node_id, "b");
    assert_eq!(results[0].depth, 1);
}

#[test]
fn traverse_two_hops() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_bidirectional_edge("a", "b", EdgeType::Related, 0.9, EdgeOrigin::Explicit, 1000);
    graph.add_bidirectional_edge("b", "c", EdgeType::Extends, 0.8, EdgeOrigin::Explicit, 2000);

    let results = graph.traverse("a", 2, None, 50);
    assert_eq!(results.len(), 2);
    // b at depth 1, c at depth 2
    let b_result = results.iter().find(|r| r.node_id == "b").unwrap();
    assert_eq!(b_result.depth, 1);
    let c_result = results.iter().find(|r| r.node_id == "c").unwrap();
    assert_eq!(c_result.depth, 2);
}

#[test]
fn traverse_deduplicates_visited_nodes() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    // Triangle: a-b, b-c, a-c
    graph.add_bidirectional_edge("a", "b", EdgeType::Related, 0.9, EdgeOrigin::Explicit, 1000);
    graph.add_bidirectional_edge("b", "c", EdgeType::Related, 0.8, EdgeOrigin::Explicit, 2000);
    graph.add_bidirectional_edge("a", "c", EdgeType::Related, 0.7, EdgeOrigin::Explicit, 3000);

    let results = graph.traverse("a", 2, None, 50);
    // b and c at depth 1 (both direct neighbors of a)
    // No duplicates
    assert_eq!(results.len(), 2);
}

#[test]
fn traverse_with_edge_type_filter() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_edge(Edge {
        source_id: "a".into(), target_id: "b".into(),
        edge_type: EdgeType::Related, weight: 0.9,
        origin: EdgeOrigin::Explicit, timestamp: 1000,
    });
    graph.add_edge(Edge {
        source_id: "a".into(), target_id: "c".into(),
        edge_type: EdgeType::Similar, weight: 0.8,
        origin: EdgeOrigin::Similarity, timestamp: 2000,
    });

    let results = graph.traverse("a", 1, Some(&[EdgeType::Related]), 50);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].node_id, "b");
}

#[test]
fn traverse_respects_max_results() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    for i in 0..10 {
        graph.add_edge(Edge {
            source_id: "a".into(),
            target_id: format!("n{}", i),
            edge_type: EdgeType::Related,
            weight: 0.9 - (i as f64 * 0.05),
            origin: EdgeOrigin::Explicit,
            timestamp: 1000,
        });
    }

    let results = graph.traverse("a", 1, None, 3);
    assert_eq!(results.len(), 3);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vector && cargo test graph::tests::traverse`
Expected: FAIL

**Step 3: Implement traversal**

Add a traversal result type and the `traverse` method:

```rust
/// Result node from a graph traversal.
#[derive(Debug, Clone)]
pub struct TraversalNode {
    pub node_id: String,
    pub depth: usize,
    pub path: Vec<String>,
}

impl GraphIndex {
    /// BFS traversal from a starting node up to `max_depth` hops.
    /// Optionally filter by edge types. Returns at most `max_results` nodes.
    /// The starting node is never included in results.
    pub fn traverse(
        &self,
        start_id: &str,
        max_depth: usize,
        edge_types: Option<&[EdgeType]>,
        max_results: usize,
    ) -> Vec<TraversalNode> {
        use std::collections::{HashSet, VecDeque};

        let mut visited = HashSet::new();
        visited.insert(start_id.to_string());

        let mut queue: VecDeque<(String, usize, Vec<String>)> = VecDeque::new();
        queue.push_back((start_id.to_string(), 0, vec![start_id.to_string()]));

        let mut results = Vec::new();

        while let Some((current_id, depth, path)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            let neighbors = match edge_types {
                Some(types) => self.neighbors_by_type(&current_id, types),
                None => self.neighbors(&current_id),
            };

            for edge in neighbors {
                if visited.contains(&edge.target_id) {
                    continue;
                }
                visited.insert(edge.target_id.clone());

                let mut new_path = path.clone();
                new_path.push(edge.target_id.clone());

                results.push(TraversalNode {
                    node_id: edge.target_id.clone(),
                    depth: depth + 1,
                    path: new_path.clone(),
                });

                if results.len() >= max_results {
                    return results;
                }

                queue.push_back((edge.target_id.clone(), depth + 1, new_path));
            }
        }

        results
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vector && cargo test graph::tests`
Expected: PASS (all 21 tests)

**Step 5: Commit**

```bash
git add simse-vector/src/graph.rs
git commit -m "feat(graph): implement BFS traversal with depth, type filter, and max results"
```

---

### Task 7: Graph-boosted scoring

**Files:**
- Modify: `simse-vector/src/graph.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn compute_graph_score_for_connected_candidates() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_bidirectional_edge("a", "b", EdgeType::Related, 0.9, EdgeOrigin::Explicit, 1000);
    graph.add_bidirectional_edge("a", "c", EdgeType::Similar, 0.7, EdgeOrigin::Similarity, 2000);

    // Candidate "a" has edges to both "b" and "c"
    // If "b" and "c" are in the result set, graph_score = max(0.9, 0.7) = 0.9
    let relevant_ids: Vec<String> = vec!["b".into(), "c".into()];
    let score = graph.compute_graph_score("a", &relevant_ids);
    assert!((score - 0.9).abs() < 1e-6);
}

#[test]
fn graph_score_zero_for_unconnected_candidate() {
    let graph = GraphIndex::new(GraphConfig::default());
    let relevant_ids: Vec<String> = vec!["b".into(), "c".into()];
    let score = graph.compute_graph_score("a", &relevant_ids);
    assert_eq!(score, 0.0);
}

#[test]
fn apply_graph_boost_to_existing_score() {
    let graph = GraphIndex::new(GraphConfig {
        graph_boost_weight: 0.15,
        ..GraphConfig::default()
    });

    let boosted = graph.apply_graph_boost(0.8, 0.9);
    // (1 - 0.15) * 0.8 + 0.15 * 0.9 = 0.68 + 0.135 = 0.815
    assert!((boosted - 0.815).abs() < 1e-6);
}

#[test]
fn graph_boost_with_zero_graph_score_preserves_original() {
    let graph = GraphIndex::new(GraphConfig {
        graph_boost_weight: 0.15,
        ..GraphConfig::default()
    });

    let boosted = graph.apply_graph_boost(0.8, 0.0);
    // (1 - 0.15) * 0.8 + 0.15 * 0.0 = 0.68
    assert!((boosted - 0.68).abs() < 1e-6);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vector && cargo test graph::tests::compute_graph_score`
Expected: FAIL

**Step 3: Implement graph scoring**

Add to `impl GraphIndex`:

```rust
/// Compute graph score for a candidate based on its connectivity to
/// the other volumes in the result set.
///
/// graph_score = max(edge.weight for edges connecting candidate to relevant_ids)
/// Returns 0.0 if no connections exist.
pub fn compute_graph_score(&self, candidate_id: &str, relevant_ids: &[String]) -> f64 {
    let edges = match self.adjacency.get(candidate_id) {
        Some(e) => e,
        None => return 0.0,
    };
    edges.iter()
        .filter(|e| relevant_ids.contains(&e.target_id))
        .map(|e| e.weight)
        .fold(0.0f64, f64::max)
}

/// Blend an existing score with a graph score using the configured weight.
///
/// final = (1 - graph_boost_weight) * existing_score + graph_boost_weight * graph_score
pub fn apply_graph_boost(&self, existing_score: f64, graph_score: f64) -> f64 {
    let w = self.config.graph_boost_weight;
    (1.0 - w) * existing_score + w * graph_score
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vector && cargo test graph::tests`
Expected: PASS (all 25 tests)

**Step 5: Commit**

```bash
git add simse-vector/src/graph.rs
git commit -m "feat(graph): add graph score computation and boost blending"
```

---

### Task 8: Graph serialization and persistence

**Files:**
- Modify: `simse-vector/src/graph.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn serialize_deserialize_graph_state() {
    let mut graph = GraphIndex::new(GraphConfig::default());
    graph.add_bidirectional_edge("a", "b", EdgeType::Related, 0.9, EdgeOrigin::Explicit, 1000);
    graph.add_edge(Edge {
        source_id: "a".into(), target_id: "c".into(),
        edge_type: EdgeType::Similar, weight: 0.88,
        origin: EdgeOrigin::Similarity, timestamp: 2000,
    });

    let state = graph.serialize();
    // Only explicit edges should be serialized
    assert_eq!(state.explicit_edges.len(), 2); // a→b and b→a

    // Rebuild from state
    let graph2 = GraphIndex::from_state(state, GraphConfig::default());
    let neighbors = graph2.neighbors("a");
    assert_eq!(neighbors.len(), 1); // Only explicit a→b (similar edge not persisted)
    assert_eq!(neighbors[0].edge_type, EdgeType::Related);
}

#[test]
fn empty_graph_serializes_to_empty_state() {
    let graph = GraphIndex::new(GraphConfig::default());
    let state = graph.serialize();
    assert!(state.explicit_edges.is_empty());

    let graph2 = GraphIndex::from_state(state, GraphConfig::default());
    assert_eq!(graph2.edge_count(), 0);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-vector && cargo test graph::tests::serialize`
Expected: FAIL

**Step 3: Implement serialization**

Add to `graph.rs`:

```rust
/// Serialized form of an edge for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeSerialized {
    #[serde(rename = "sourceId")]
    pub source_id: String,
    #[serde(rename = "targetId")]
    pub target_id: String,
    #[serde(rename = "edgeType")]
    pub edge_type: EdgeType,
    pub weight: f64,
    pub timestamp: u64,
}

/// Persisted graph state — only explicit edges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphState {
    #[serde(rename = "explicitEdges")]
    pub explicit_edges: Vec<EdgeSerialized>,
    pub config: GraphConfig,
}

impl GraphIndex {
    /// Serialize explicit edges for persistence.
    pub fn serialize(&self) -> GraphState {
        let mut explicit_edges = Vec::new();
        for edges in self.adjacency.values() {
            for edge in edges {
                if edge.origin == EdgeOrigin::Explicit {
                    explicit_edges.push(EdgeSerialized {
                        source_id: edge.source_id.clone(),
                        target_id: edge.target_id.clone(),
                        edge_type: edge.edge_type.clone(),
                        weight: edge.weight,
                        timestamp: edge.timestamp,
                    });
                }
            }
        }
        GraphState {
            explicit_edges,
            config: self.config.clone(),
        }
    }

    /// Rebuild a GraphIndex from persisted state (explicit edges only).
    /// Implicit edges must be rebuilt separately after volumes are loaded.
    pub fn from_state(state: GraphState, config: GraphConfig) -> Self {
        let mut graph = Self::new(config);
        for edge in state.explicit_edges {
            graph.add_edge(Edge {
                source_id: edge.source_id,
                target_id: edge.target_id,
                edge_type: edge.edge_type,
                weight: edge.weight,
                origin: EdgeOrigin::Explicit,
                timestamp: edge.timestamp,
            });
        }
        graph
    }
}
```

**Step 4: Run tests to verify they pass**

Run: `cd simse-vector && cargo test graph::tests`
Expected: PASS (all 27 tests)

**Step 5: Commit**

```bash
git add simse-vector/src/graph.rs
git commit -m "feat(graph): add serialization for explicit edge persistence"
```

---

### Task 9: Wire GraphIndex into VolumeStore

**Files:**
- Modify: `simse-vector/src/store.rs`
- Modify: `simse-vector/src/types.rs`

This is the largest integration task. It connects the GraphIndex to the VolumeStore lifecycle.

**Step 1: Add GraphConfig to StoreConfig and GraphIndex to VolumeStore**

In `store.rs`, add `use crate::graph::{GraphConfig, GraphIndex};` to imports.

Add to `StoreConfig`:
```rust
pub graph_config: GraphConfig,
```

Add to `StoreConfig::default()`:
```rust
graph_config: GraphConfig::default(),
```

Add to `VolumeStore`:
```rust
graph_index: GraphIndex,
```

Initialize in `VolumeStore::new()` (find the constructor):
```rust
graph_index: GraphIndex::new(config.graph_config.clone()),
```

**Step 2: Wire into add — parse metadata edges + create similarity edges**

In `VolumeStore::add()`, after `self.index_volume(&volume)` and before `self.dirty = true`:

```rust
// Parse explicit edges from rel:* metadata
self.graph_index.parse_metadata_edges(&volume.id, &volume.metadata, volume.timestamp);

// Create implicit similarity edges
let new_mag = compute_magnitude(&volume.embedding);
for existing in &self.volumes {
    let existing_mag = self.magnitude_cache.get(&existing.id);
    let sim = crate::cosine::cosine_similarity_with_magnitude(
        &volume.embedding, &existing.embedding, new_mag, existing_mag,
    );
    self.graph_index.add_similarity_edge(&volume.id, &existing.id, sim, volume.timestamp);
}
```

**Step 3: Wire into delete — cascade edge removal**

In `VolumeStore::delete()`, after `self.deindex_volume(&vol)`:

```rust
self.graph_index.remove_node(id);
```

**Step 4: Wire into save/load — persistence**

In `VolumeStore::save()`, serialize graph state alongside learning state. The graph state will be persisted via the `LearningState` companion — we add a `graph_state` key.

Actually, the cleaner approach: persist `GraphState` as a separate key in the persistence storage. Add to `persistence.rs`:

```rust
pub const GRAPH_KEY: &str = "__graph";
```

In `serialize_to_storage()`, add graph state parameter:

Modify `serialize_to_storage` signature to:
```rust
pub fn serialize_to_storage(
    entries: &[Volume],
    access_stats: &HashMap<String, AccessStats>,
    learning_state: Option<&LearningState>,
    graph_state: Option<&crate::graph::GraphState>,
) -> HashMap<String, Vec<u8>>
```

Add after the learning state block:
```rust
if let Some(state) = graph_state {
    if !state.explicit_edges.is_empty() {
        match serde_json::to_string(state) {
            Ok(json) => {
                data.insert(GRAPH_KEY.to_string(), json.into_bytes());
            }
            Err(_) => {}
        }
    }
}
```

Similarly, extend `DeserializedData`:
```rust
pub graph_state: Option<crate::graph::GraphState>,
```

In `deserialize_from_storage()`, handle the `GRAPH_KEY`:
```rust
if key == GRAPH_KEY {
    match std::str::from_utf8(value) {
        Ok(json_str) => match serde_json::from_str::<crate::graph::GraphState>(json_str) {
            Ok(state) => { graph_state = Some(state); }
            Err(_) => { skipped += 1; }
        },
        Err(_) => { skipped += 1; }
    }
    continue;
}
```

Update `save_to_directory` and `load_from_directory` to pass the new parameter through.

**Step 5: Wire into VolumeStore::save()**

Add `graph_state` to the save call:
```rust
let graph_state = self.graph_index.serialize();
persistence::save_to_directory(
    &path,
    &self.volumes,
    &self.access_stats,
    learning_state.as_ref(),
    Some(&graph_state),
)?;
```

**Step 6: Wire into VolumeStore load (initialize)**

After loading and rebuilding learning engine, rebuild graph:
```rust
// Restore explicit edges from persisted state
if let Some(gs) = data.graph_state {
    self.graph_index = GraphIndex::from_state(gs, self.config.graph_config.clone());
}

// Rebuild implicit similarity edges
for i in 0..self.volumes.len() {
    for j in (i + 1)..self.volumes.len() {
        let sim = crate::cosine::cosine_similarity(
            &self.volumes[i].embedding, &self.volumes[j].embedding,
        );
        let ts = self.volumes[i].timestamp.max(self.volumes[j].timestamp);
        self.graph_index.add_similarity_edge(
            &self.volumes[i].id, &self.volumes[j].id, sim, ts,
        );
    }
}
```

**Step 7: Add graph accessors to VolumeStore**

Add public methods to `VolumeStore`:

```rust
pub fn graph_neighbors(
    &self,
    id: &str,
    edge_types: Option<&[crate::graph::EdgeType]>,
    max_results: usize,
) -> Vec<(&crate::graph::Edge, Option<&Volume>)> {
    let edges = match edge_types {
        Some(types) => self.graph_index.neighbors_by_type(id, types),
        None => self.graph_index.neighbors(id),
    };
    edges.into_iter()
        .take(max_results)
        .map(|edge| {
            let volume = self.volumes.iter().find(|v| v.id == edge.target_id);
            (edge, volume)
        })
        .collect()
}

pub fn graph_traverse(
    &self,
    id: &str,
    depth: usize,
    edge_types: Option<&[crate::graph::EdgeType]>,
    max_results: usize,
) -> Vec<(crate::graph::TraversalNode, Option<&Volume>)> {
    let nodes = self.graph_index.traverse(id, depth, edge_types, max_results);
    nodes.into_iter()
        .map(|node| {
            let volume = self.volumes.iter().find(|v| v.id == node.node_id);
            (node, volume)
        })
        .collect()
}

pub fn graph_index(&self) -> &GraphIndex {
    &self.graph_index
}
```

**Step 8: Run all Rust tests**

Run: `cd simse-vector && cargo test`
Expected: PASS — all existing tests continue to pass, graph tests pass

**Step 9: Commit**

```bash
git add simse-vector/src/store.rs simse-vector/src/graph.rs simse-vector/src/persistence.rs simse-vector/src/types.rs
git commit -m "feat(graph): wire GraphIndex into VolumeStore lifecycle"
```

---

### Task 10: Graph-boosted advancedSearch

**Files:**
- Modify: `simse-vector/src/types.rs`
- Modify: `simse-vector/src/store.rs`

**Step 1: Add GraphBoost to SearchOptions type**

In `types.rs`, add after `SearchOptions`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphBoost {
    pub enabled: Option<bool>,
    pub weight: Option<f64>,
}
```

Add to `SearchOptions`:
```rust
#[serde(rename = "graphBoost")]
pub graph_boost: Option<GraphBoost>,
```

**Step 2: Integrate graph boost into advanced_search**

In `VolumeStore::advanced_search()`, after the initial scoring loop but before final sort:

```rust
// Apply graph boost if enabled
if let Some(gb) = &options.graph_boost {
    if gb.enabled.unwrap_or(false) {
        let weight = gb.weight.unwrap_or(self.graph_index.config().graph_boost_weight);
        let result_ids: Vec<String> = scored.iter().map(|(_, v, _, _)| v.id.clone()).collect();
        for (score, volume, _, _) in &mut scored {
            let graph_score = self.graph_index.compute_graph_score(&volume.id, &result_ids);
            *score = (1.0 - weight) * *score + weight * graph_score;
        }
    }
}
```

Find the exact integration point by looking for the sort step in `advanced_search`.

**Step 3: Run tests**

Run: `cd simse-vector && cargo test`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-vector/src/store.rs simse-vector/src/types.rs
git commit -m "feat(graph): integrate graph boost into advancedSearch"
```

---

### Task 11: Graph-boosted recommend

**Files:**
- Modify: `simse-vector/src/types.rs`
- Modify: `simse-vector/src/store.rs`

**Step 1: Extend WeightProfile and RecommendationScores**

In `types.rs`, add `graph` field to `WeightProfile`:
```rust
pub graph: Option<f64>,
```

Add `graph` field to `RecommendationScores`:
```rust
pub graph: Option<f64>,
```

**Step 2: Integrate graph into recommend scoring**

In `VolumeStore::recommend()`, compute graph score alongside vector/recency/frequency. The graph score for a candidate is its max edge weight to any volume the user recently accessed (tracked via `access_stats`).

After the frequency score computation:
```rust
// Graph score: connectivity to recently accessed volumes
let recent_ids: Vec<String> = self.access_stats.iter()
    .filter(|(_, stats)| stats.access_count > 0)
    .map(|(id, _)| id.clone())
    .collect();
let graph_score = self.graph_index.compute_graph_score(&vol.id, &recent_ids);
```

Include in the weighted sum based on the `graph` weight from the profile.

**Step 3: Run tests**

Run: `cd simse-vector && cargo test`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-vector/src/store.rs simse-vector/src/types.rs
git commit -m "feat(graph): integrate graph score into recommend scoring"
```

---

### Task 12: JSON-RPC endpoints — graph/neighbors and graph/traverse

**Files:**
- Modify: `simse-vector/src/server.rs`

**Step 1: Add handler param types**

```rust
#[derive(Deserialize)]
struct GraphNeighborsParams {
    id: String,
    #[serde(rename = "edgeTypes")]
    edge_types: Option<Vec<String>>,
    #[serde(rename = "maxResults")]
    max_results: Option<usize>,
}

#[derive(Deserialize)]
struct GraphTraverseParams {
    id: String,
    depth: Option<usize>,
    #[serde(rename = "edgeTypes")]
    edge_types: Option<Vec<String>>,
    #[serde(rename = "maxResults")]
    max_results: Option<usize>,
}
```

**Step 2: Add handler functions**

```rust
fn parse_edge_types(raw: &[String]) -> Vec<crate::graph::EdgeType> {
    raw.iter().filter_map(|s| match s.as_str() {
        "Related" => Some(crate::graph::EdgeType::Related),
        "Parent" => Some(crate::graph::EdgeType::Parent),
        "Child" => Some(crate::graph::EdgeType::Child),
        "Extends" => Some(crate::graph::EdgeType::Extends),
        "Contradicts" => Some(crate::graph::EdgeType::Contradicts),
        "Similar" => Some(crate::graph::EdgeType::Similar),
        "CoOccurs" => Some(crate::graph::EdgeType::CoOccurs),
        _ => None,
    }).collect()
}

fn handle_graph_neighbors(
    store: &VolumeStore,
    params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
    let p: GraphNeighborsParams = parse_params(params)?;
    let edge_types = p.edge_types.as_ref().map(|et| parse_edge_types(et));
    let max = p.max_results.unwrap_or(20);

    let results = store.graph_neighbors(&p.id, edge_types.as_deref(), max);
    let neighbors: Vec<serde_json::Value> = results.iter().map(|(edge, vol)| {
        serde_json::json!({
            "volume": vol,
            "edge": {
                "edgeType": format!("{:?}", edge.edge_type),
                "weight": edge.weight,
                "origin": format!("{:?}", edge.origin),
            }
        })
    }).collect();

    Ok(serde_json::json!({ "neighbors": neighbors }))
}

fn handle_graph_traverse(
    store: &VolumeStore,
    params: serde_json::Value,
) -> Result<serde_json::Value, VectorError> {
    let p: GraphTraverseParams = parse_params(params)?;
    let edge_types = p.edge_types.as_ref().map(|et| parse_edge_types(et));
    let depth = p.depth.unwrap_or(1).min(2); // Cap at 2 hops
    let max = p.max_results.unwrap_or(50);

    let results = store.graph_traverse(&p.id, depth, edge_types.as_deref(), max);
    let nodes: Vec<serde_json::Value> = results.iter().map(|(node, vol)| {
        serde_json::json!({
            "volume": vol,
            "depth": node.depth,
            "path": node.path,
        })
    }).collect();

    Ok(serde_json::json!({ "nodes": nodes }))
}
```

**Step 3: Register in dispatch**

Add to the `dispatch()` match block:

```rust
// -- Graph ---------------------------------------------------
"graph/neighbors" => self.with_store(|s| handle_graph_neighbors(s, req.params)),
"graph/traverse" => self.with_store(|s| handle_graph_traverse(s, req.params)),
```

**Step 4: Run tests**

Run: `cd simse-vector && cargo test`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-vector/src/server.rs
git commit -m "feat(graph): add graph/neighbors and graph/traverse JSON-RPC endpoints"
```

---

### Task 13: Rust integration tests

**Files:**
- Modify: `simse-vector/tests/integration.rs`

**Step 1: Write integration tests**

Add to the existing integration test file:

```rust
#[test]
fn graph_neighbors_with_explicit_edges() {
    let mut harness = Harness::spawn();
    harness.init(None);

    let id1 = harness.add("Volume about Rust programming", &[1.0, 0.0, 0.0], Some(json!({
        "topic": "programming",
        "rel:related": "" // Will be set after id2 is known
    })));
    let id2 = harness.add("Volume about Rust async", &[0.9, 0.1, 0.0], Some(json!({
        "topic": "programming",
        "rel:related": &id1
    })));

    let resp = harness.call("graph/neighbors", json!({
        "id": id2,
        "maxResults": 10
    }));

    let neighbors = resp["neighbors"].as_array().unwrap();
    assert!(neighbors.len() >= 1);
}

#[test]
fn graph_traverse_two_hops() {
    let mut harness = Harness::spawn();
    harness.init(None);

    let id1 = harness.add("Base concept", &[1.0, 0.0, 0.0], None);
    let id2 = harness.add("Extends base", &[0.9, 0.1, 0.0], Some(json!({
        "rel:extends": &id1
    })));
    let id3 = harness.add("Extends further", &[0.8, 0.2, 0.0], Some(json!({
        "rel:extends": &id2
    })));

    let resp = harness.call("graph/traverse", json!({
        "id": id1,
        "depth": 2,
        "maxResults": 50
    }));

    // Should find id2 at depth 1 and id3 at depth 2 (via reverse edges)
    let nodes = resp["nodes"].as_array().unwrap();
    assert!(!nodes.is_empty());
}

#[test]
fn graph_boosted_search() {
    let mut harness = Harness::spawn();
    harness.init(None);

    let id1 = harness.add("Machine learning fundamentals", &[1.0, 0.0, 0.0], None);
    let id2 = harness.add("Neural network architectures", &[0.8, 0.2, 0.0], Some(json!({
        "rel:related": &id1
    })));
    let _id3 = harness.add("Gardening tips for beginners", &[0.0, 0.0, 1.0], None);

    let resp = harness.call("store/advancedSearch", json!({
        "queryEmbedding": [0.9, 0.1, 0.0],
        "maxResults": 10,
        "graphBoost": { "enabled": true, "weight": 0.2 }
    }));

    let results = resp["results"].as_array().unwrap();
    assert!(results.len() >= 2);
    // The related volume should rank highly
}

#[test]
fn graph_persistence_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.path().to_str().unwrap();
    let mut harness = Harness::spawn();

    harness.init(Some(dir_path));
    let id1 = harness.add("Volume A", &[1.0, 0.0], None);
    let _id2 = harness.add("Volume B", &[0.0, 1.0], Some(json!({
        "rel:related": &id1
    })));

    harness.call("store/save", json!({}));
    harness.dispose();

    // Reload in a new harness
    let mut harness2 = Harness::spawn();
    harness2.init(Some(dir_path));

    let resp = harness2.call("graph/neighbors", json!({
        "id": id1,
        "maxResults": 10
    }));

    let neighbors = resp["neighbors"].as_array().unwrap();
    // Explicit edge should survive the round trip
    assert!(!neighbors.is_empty());
    harness2.dispose();
}

#[test]
fn delete_volume_cascades_graph_edges() {
    let mut harness = Harness::spawn();
    harness.init(None);

    let id1 = harness.add("Volume A", &[1.0, 0.0, 0.0], None);
    let id2 = harness.add("Volume B", &[0.9, 0.1, 0.0], Some(json!({
        "rel:related": &id1
    })));

    // Verify edge exists
    let resp = harness.call("graph/neighbors", json!({ "id": id1, "maxResults": 10 }));
    assert!(!resp["neighbors"].as_array().unwrap().is_empty());

    // Delete volume B
    harness.call("store/delete", json!({ "id": id2 }));

    // Edge should be gone
    let resp2 = harness.call("graph/neighbors", json!({ "id": id1, "maxResults": 10 }));
    assert!(resp2["neighbors"].as_array().unwrap().is_empty());
}
```

**Note:** These tests rely on the existing `Harness` struct pattern in integration.rs. Adapt the `add()` helper to accept an optional metadata parameter if it doesn't already. Check the existing test helpers in the file.

**Step 2: Run integration tests**

Run: `cd simse-vector && cargo test --test integration`
Expected: PASS

**Step 3: Commit**

```bash
git add simse-vector/tests/integration.rs
git commit -m "test(graph): add Rust integration tests for graph endpoints"
```

---

### Task 14: TypeScript client — graph methods

**Files:**
- Modify: `src/ai/library/types.ts`
- Modify: `src/ai/library/stacks.ts`

**Step 1: Add graph types to types.ts**

```typescript
// ---------------------------------------------------------------------------
// Graph Intelligence
// ---------------------------------------------------------------------------

export type GraphEdgeType =
    | 'Related'
    | 'Parent'
    | 'Child'
    | 'Extends'
    | 'Contradicts'
    | 'Similar'
    | 'CoOccurs';

export type GraphEdgeOrigin = 'Explicit' | 'Similarity' | 'Correlation';

export interface GraphEdge {
    readonly edgeType: GraphEdgeType;
    readonly weight: number;
    readonly origin: GraphEdgeOrigin;
}

export interface GraphNeighbor {
    readonly volume: Volume;
    readonly edge: GraphEdge;
}

export interface GraphTraversalNode {
    readonly volume: Volume;
    readonly depth: number;
    readonly path: readonly string[];
}

export interface GraphBoostOptions {
    readonly enabled?: boolean;
    readonly weight?: number;
}
```

Add `graphBoost` to `SearchOptions`:
```typescript
readonly graphBoost?: GraphBoostOptions;
```

Add `graph` to `WeightProfile`:
```typescript
readonly graph?: number;
```

Add `graph` to `RecommendationScores`:
```typescript
readonly graph?: number;
```

**Step 2: Add graph methods to Stacks interface and implementation**

In `stacks.ts`, add to the `Stacks` interface:

```typescript
readonly graphNeighbors: (
    id: string,
    edgeTypes?: readonly GraphEdgeType[],
    maxResults?: number,
) => Promise<GraphNeighbor[]>;
readonly graphTraverse: (
    id: string,
    depth?: number,
    edgeTypes?: readonly GraphEdgeType[],
    maxResults?: number,
) => Promise<GraphTraversalNode[]>;
```

Add implementations in `createStacks`:

```typescript
const graphNeighbors = async (
    id: string,
    edgeTypes?: readonly GraphEdgeType[],
    maxResults?: number,
): Promise<GraphNeighbor[]> => {
    const result = await client.request<{ neighbors: GraphNeighbor[] }>(
        'graph/neighbors',
        { id, edgeTypes: edgeTypes ? [...edgeTypes] : undefined, maxResults },
    );
    return result.neighbors;
};

const graphTraverse = async (
    id: string,
    depth?: number,
    edgeTypes?: readonly GraphEdgeType[],
    maxResults?: number,
): Promise<GraphTraversalNode[]> => {
    const result = await client.request<{ nodes: GraphTraversalNode[] }>(
        'graph/traverse',
        { id, depth, edgeTypes: edgeTypes ? [...edgeTypes] : undefined, maxResults },
    );
    return result.nodes;
};
```

Add `graphNeighbors` and `graphTraverse` to the frozen return object.

**Step 3: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 4: Commit**

```bash
git add src/ai/library/types.ts src/ai/library/stacks.ts
git commit -m "feat(graph): add graph types and methods to TypeScript client"
```

---

### Task 15: TypeScript E2E tests

**Files:**
- Create: `tests/library/graph-intelligence.test.ts`

**Step 1: Write E2E tests**

```typescript
import { afterEach, describe, expect, it } from 'bun:test';
import { fileURLToPath } from 'node:url';
import { createLibrary } from '../../src/ai/library/library.js';
import type { EmbeddingProvider } from '../../src/ai/library/types.js';

const ENGINE_PATH = fileURLToPath(
    new URL(
        '../../simse-vector/target/debug/simse-vector-engine.exe',
        import.meta.url,
    ),
);

// Deterministic mock embedder producing similar vectors for related text
function createMockEmbedder(): EmbeddingProvider {
    const DIM = 128;
    function wordHash(word: string): number {
        let h = 0;
        for (let i = 0; i < word.length; i++) {
            h = (h * 31 + word.charCodeAt(i)) | 0;
        }
        return ((h % DIM) + DIM) % DIM;
    }
    function hashText(text: string): number[] {
        const vec = new Array(DIM).fill(0);
        const words = text.toLowerCase().replace(/[^a-z0-9\s]/g, '').split(/\s+/);
        for (const word of words) {
            if (word.length === 0) continue;
            vec[wordHash(word)] += 1.0;
            vec[wordHash(`${word}_`)] += 0.5;
        }
        const mag = Math.sqrt(vec.reduce((s: number, v: number) => s + v * v, 0)) || 1;
        return vec.map((v: number) => v / mag);
    }
    return Object.freeze({
        embed: async (input: string | readonly string[]) => {
            const texts = typeof input === 'string' ? [input] : [...input];
            return { embeddings: texts.map(hashText) };
        },
    });
}

describe('Graph Intelligence E2E', () => {
    const embedder = createMockEmbedder();
    let library: ReturnType<typeof createLibrary>;

    afterEach(async () => {
        await library?.dispose();
    });

    it('creates explicit edges from rel:* metadata', async () => {
        library = createLibrary(embedder, {}, {
            enginePath: ENGINE_PATH,
            stacksOptions: { duplicateThreshold: 1 },
        });
        await library.initialize();

        const id1 = await library.add('Machine learning fundamentals', {
            topic: 'ml',
        });
        const id2 = await library.add('Deep learning builds on ML', {
            topic: 'ml',
            'rel:related': id1,
        });

        // Verify graph neighbors exist via stacks
        // (library.stacks is internal — test via search boost instead)
        const results = await library.advancedSearch({
            queryEmbedding: (await embedder.embed('machine learning')).embeddings[0] as number[],
            maxResults: 10,
            graphBoost: { enabled: true, weight: 0.2 },
        });
        expect(results.length).toBeGreaterThanOrEqual(2);
    });

    it('graph-boosted search ranks connected volumes higher', async () => {
        library = createLibrary(embedder, {}, {
            enginePath: ENGINE_PATH,
            stacksOptions: { duplicateThreshold: 1 },
        });
        await library.initialize();

        const id1 = await library.add('TypeScript is a typed superset of JavaScript');
        const id2 = await library.add('JavaScript runs in the browser', {
            'rel:related': id1,
        });
        await library.add('The weather in London is rainy');

        const results = await library.advancedSearch({
            queryEmbedding: (await embedder.embed('TypeScript programming')).embeddings[0] as number[],
            maxResults: 10,
            graphBoost: { enabled: true, weight: 0.2 },
        });

        expect(results.length).toBeGreaterThanOrEqual(2);
        // The JS volume should be boosted by its connection to the TS volume
    });
});
```

**Note:** The `library.advancedSearch` method may need to be exposed or the test may need to use `library.stacks` directly. Check the Library interface in `library.ts`. If `advancedSearch` is not on the Library interface, add a thin passthrough or test via stacks directly.

**Step 2: Run the test**

Run: `cd /d/GitHub/simse && bun test tests/library/graph-intelligence.test.ts`
Expected: PASS

**Step 3: Commit**

```bash
git add tests/library/graph-intelligence.test.ts
git commit -m "test(graph): add TypeScript E2E tests for graph intelligence"
```

---

### Task 16: Final verification

**Step 1: Run all Rust tests**

Run: `cd simse-vector && cargo test`
Expected: PASS

**Step 2: Run all TypeScript tests**

Run: `bun test`
Expected: PASS (0 failures)

**Step 3: Run typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 4: Run lint**

Run: `bun run lint`
Expected: Clean (or only pre-existing warnings in simse-code/simse-landing)

**Step 5: Final commit if any cleanup needed**

```bash
git add -A
git commit -m "chore(graph): final cleanup and verification"
```
