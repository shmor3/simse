# Graph Intelligence for simse-vector

## Overview

Add a `GraphIndex` module to simse-vector that maintains weighted, typed edges between volumes. Supports both explicit edges (via `rel:*` metadata) and implicit edges (auto-discovered from embedding similarity and query co-occurrence). Graph connectivity integrates into existing search and recommendation scoring as an additional signal.

## Data Model

### Edge

```rust
pub struct Edge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: EdgeType,
    pub weight: f64,           // 0.0-1.0
    pub origin: EdgeOrigin,
    pub timestamp: u64,
}

pub enum EdgeType {
    Related,      // General relationship
    Parent,       // Hierarchical: source is parent of target
    Child,        // Inverse of Parent
    Extends,      // Target builds on source
    Contradicts,  // Source and target conflict
    Similar,      // High cosine similarity (implicit)
    CoOccurs,     // Frequently appear in same query results (implicit)
}

pub enum EdgeOrigin {
    Explicit,     // From rel:* metadata
    Similarity,   // Auto-discovered from embedding cosine
    Correlation,  // From learning engine co-occurrence
}
```

### GraphIndex

```rust
pub struct GraphIndex {
    adjacency: HashMap<String, Vec<Edge>>,  // source_id -> outgoing edges
    reverse: HashMap<String, Vec<Edge>>,    // target_id -> incoming edges
    config: GraphConfig,
}

pub struct GraphConfig {
    pub similarity_threshold: f64,   // Min cosine for Similar edge (default 0.85)
    pub correlation_threshold: usize, // Min co-occurrence count for CoOccurs edge (default 3)
    pub max_edges_per_node: usize,   // Cap to prevent hub explosion (default 50)
    pub graph_boost_weight: f64,     // Weight in advancedSearch (default 0.15)
}
```

### Metadata Convention

Explicit edges declared via metadata keys prefixed with `rel:`:

| Metadata Key | EdgeType | Direction |
|---|---|---|
| `rel:related` | Related | Bidirectional |
| `rel:parent` | Parent/Child | Source->Target + inverse |
| `rel:extends` | Extends | Source->Target |
| `rel:contradicts` | Contradicts | Bidirectional |

Values are comma-separated volume IDs: `rel:related: "id1,id2,id3"`

## Implicit Edge Discovery

### Source 1: Embedding Similarity

On `store/add` and `store/addBatch`, compute cosine similarity against existing volumes. Create `Similar` edges for pairs exceeding `similarity_threshold` (default 0.85).

- O(N) per add (same cost as existing dedup check)
- Skip if explicit edge already exists between the pair
- Keep only top `max_edges_per_node` strongest edges per volume

### Source 2: Learning Engine Correlations

On `learning/recordQuery`, after correlations update, promote pairs exceeding `correlation_threshold` (default 3 co-occurrences) to `CoOccurs` graph edges.

- Weight: normalized co-occurrence count (count / max_count_in_graph)
- Decay: scale weight by recency of the last query that produced the co-occurrence

### Edge Lifecycle

- **Created**: On volume add (similarity) or query recording (correlation)
- **Updated**: Weight refreshed when a stronger signal arrives
- **Removed**: Cascade when endpoint volume deleted. Implicit edges below threshold pruned during cleanup.

## Graph-Boosted Search

### Score Computation

For search candidates, compute:

```
graph_score(candidate) = max(edge.weight for edge in edges_to_relevant_volumes)
```

Where "relevant volumes" are the other top-scoring candidates in the same result set.

### advancedSearch Integration

```
final_score = (1 - graph_boost_weight) * existing_score + graph_boost_weight * graph_score
```

Default `graph_boost_weight: 0.15`. Activated via optional `graphBoost` field in SearchOptions:

```json
{ "graphBoost": { "enabled": true, "weight": 0.2 } }
```

When omitted or `enabled: false`, behavior unchanged.

### recommend Integration

Graph added as fourth weight dimension:

```
score = vector * w_v + recency * w_r + frequency * w_f + graph * w_g
```

Graph score: edge connectivity to volumes the user recently interacted with.

### Zero Regression

If GraphIndex has no edges for candidates, `graph_score = 0`. Existing scoring unaffected for stores without graph data.

## JSON-RPC API

### New Endpoints

**`graph/neighbors`** -- Direct neighbors (1 hop)

```json
// Request
{ "id": "volume-abc", "edgeTypes": ["Related", "Similar"], "maxResults": 20 }

// Response
{ "neighbors": [{ "volume": {...}, "edge": { "edgeType": "Similar", "weight": 0.92, "origin": "Similarity" } }] }
```

**`graph/traverse`** -- Shallow traversal (1-2 hops)

```json
// Request
{ "id": "volume-abc", "depth": 2, "edgeTypes": ["Related", "Extends"], "maxResults": 50 }

// Response
{ "nodes": [{ "volume": {...}, "depth": 1, "path": ["volume-abc", "volume-def"] }] }
```

### Modified Endpoints

**`store/advancedSearch`** -- Add optional `graphBoost` to SearchOptions
**`store/recommend`** -- Add optional `graph` weight to WeightProfile

## Persistence

### Persisted

Explicit edges saved in `graph_state` section of the store file:

```rust
pub struct GraphState {
    pub explicit_edges: Vec<EdgeSerialized>,
    pub config: GraphConfig,
}
```

### Rebuilt on Load

Implicit edges rebuilt on `store/initialize`:
1. Similar edges: recomputed from volume embeddings (O(N^2) at startup)
2. CoOccurs edges: synced from learning engine's persisted correlation graph

### Migration

Existing stores without `graph_state` load normally. GraphIndex initializes empty and builds implicit edges from existing data on first load.

## Testing

### Rust Unit Tests (graph.rs)

- Edge CRUD: add/remove, bidirectional creation for Related/Contradicts
- Metadata parsing: rel:* keys -> correct EdgeType and targets
- Neighbor lookup: sorted by weight, filtered by type
- Traversal: 1-hop and 2-hop BFS, depth limiting, deduplication
- Edge cap: max_edges_per_node enforced, weakest evicted
- Cascade delete: removing volume removes all its edges

### Rust Integration Tests (tests/integration.rs)

- Add volumes with rel:* metadata -> graph edges created
- Implicit similarity edges created when cosine > threshold
- Correlation sync: record queries -> CoOccurs edges appear
- Graph-boosted advancedSearch: connected volumes rank higher
- Graph-boosted recommend: graph weight affects scores
- Persistence round-trip: save -> load -> explicit edges preserved, implicit rebuilt
- Migration: load old format without graph_state -> no crash, empty graph

### TypeScript E2E Tests (tests/library/)

- graph/neighbors and graph/traverse JSON-RPC calls through TS client
- Graph boost in search results via advancedSearch with graphBoost option
- End-to-end: add related volumes -> search -> verify boosted ranking
