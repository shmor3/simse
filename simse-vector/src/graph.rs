// ---------------------------------------------------------------------------
// Graph Intelligence — relationship edges between vector entries
// ---------------------------------------------------------------------------
//
// Maintains a directed graph of typed, weighted edges between volumes.
// Supports:
//
// 1. Explicit edges — declared via metadata (`rel:*` keys) or API calls.
// 2. Similarity edges — auto-created when cosine similarity exceeds a
//    threshold and no explicit edge already exists.
// 3. Correlation edges — derived from co-occurrence counts in the
//    learning engine.
//
// The graph augments search & recommendation with relationship-aware
// scoring and BFS traversal.
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

// ---------------------------------------------------------------------------
// Edge types
// ---------------------------------------------------------------------------

/// The semantic kind of relationship between two volumes.
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

/// How the edge was created.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EdgeOrigin {
	Explicit,
	Similarity,
	Correlation,
}

// ---------------------------------------------------------------------------
// Edge
// ---------------------------------------------------------------------------

/// A single directed, weighted edge from `source_id` to `target_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
	pub source_id: String,
	pub target_id: String,
	pub edge_type: EdgeType,
	pub weight: f64,
	pub origin: EdgeOrigin,
	pub timestamp: u64,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Tuning knobs for the graph index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
	/// Minimum cosine similarity to auto-create a Similar edge.
	#[serde(rename = "similarityThreshold")]
	pub similarity_threshold: f64,
	/// Minimum co-occurrence count to create a CoOccurs edge.
	#[serde(rename = "correlationThreshold")]
	pub correlation_threshold: usize,
	/// Hard cap on outgoing edges per node (weakest evicted first).
	#[serde(rename = "maxEdgesPerNode")]
	pub max_edges_per_node: usize,
	/// Blending weight when boosting search scores with graph info.
	#[serde(rename = "graphBoostWeight")]
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

// ---------------------------------------------------------------------------
// GraphIndex
// ---------------------------------------------------------------------------

/// In-memory graph of edges between volumes.
///
/// `adjacency` maps source_id → outgoing edges.
/// `reverse` maps target_id → incoming edges.
pub struct GraphIndex {
	adjacency: HashMap<String, Vec<Edge>>,
	reverse: HashMap<String, Vec<Edge>>,
	config: GraphConfig,
}

impl GraphIndex {
	// -- Constructor / accessors -------------------------------------------

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

	// -- Edge CRUD --------------------------------------------------------

	/// Add a single directed edge.
	///
	/// If an edge with the same (source, target, type) already exists,
	/// it is updated only when the new weight is stronger. After insertion
	/// the source node's edge list is capped at `max_edges_per_node` by
	/// evicting the weakest edge.
	pub fn add_edge(&mut self, edge: Edge) {
		// Try to update existing edge with same (source, target, type)
		let edges = self
			.adjacency
			.entry(edge.source_id.clone())
			.or_default();

		let mut found = false;
		for existing in edges.iter_mut() {
			if existing.target_id == edge.target_id && existing.edge_type == edge.edge_type {
				if edge.weight > existing.weight {
					existing.weight = edge.weight;
					existing.timestamp = edge.timestamp;
					existing.origin = edge.origin.clone();
				}
				found = true;
				break;
			}
		}

		if !found {
			edges.push(edge.clone());
		}

		// Enforce max_edges_per_node by evicting weakest
		let edges = self.adjacency.get_mut(&edge.source_id).unwrap();
		if edges.len() > self.config.max_edges_per_node {
			// Sort ascending by weight so we can pop the weakest
			edges.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap_or(std::cmp::Ordering::Equal));
			let evicted = edges.remove(0);
			// Also remove from reverse index
			if let Some(rev) = self.reverse.get_mut(&evicted.target_id) {
				rev.retain(|e| {
					!(e.source_id == evicted.source_id && e.edge_type == evicted.edge_type)
				});
			}
		}

		// Update reverse index (only if we didn't just update in-place via found)
		if !found {
			// Check if the newly added edge survived eviction before updating reverse index
			let survived = self
				.adjacency
				.get(&edge.source_id)
				.map(|edges| {
					edges.iter().any(|e| {
						e.target_id == edge.target_id && e.edge_type == edge.edge_type
					})
				})
				.unwrap_or(false);

			if survived {
				let rev = self
					.reverse
					.entry(edge.target_id.clone())
					.or_default();
				rev.push(edge);
			}
		} else {
			// Update the weight in reverse index too
			if let Some(rev) = self.reverse.get_mut(&edge.target_id) {
				for existing in rev.iter_mut() {
					if existing.source_id == edge.source_id
						&& existing.edge_type == edge.edge_type
					{
						if edge.weight > existing.weight {
							existing.weight = edge.weight;
							existing.timestamp = edge.timestamp;
							existing.origin = edge.origin.clone();
						}
						break;
					}
				}
			}
		}
	}

	/// Create edges in both directions between `a` and `b`.
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
			source_id: a.to_string(),
			target_id: b.to_string(),
			edge_type: edge_type.clone(),
			weight,
			origin: origin.clone(),
			timestamp,
		});
		self.add_edge(Edge {
			source_id: b.to_string(),
			target_id: a.to_string(),
			edge_type,
			weight,
			origin,
			timestamp,
		});
	}

	/// Remove all edges that involve `id` (both as source and target).
	pub fn remove_node(&mut self, id: &str) {
		// Remove outgoing edges for this node, and clean up reverse entries
		if let Some(outgoing) = self.adjacency.remove(id) {
			for edge in &outgoing {
				if let Some(rev) = self.reverse.get_mut(&edge.target_id) {
					rev.retain(|e| e.source_id != id);
					if rev.is_empty() {
						self.reverse.remove(&edge.target_id);
					}
				}
			}
		}

		// Remove incoming edges for this node, and clean up adjacency entries
		if let Some(incoming) = self.reverse.remove(id) {
			for edge in &incoming {
				if let Some(adj) = self.adjacency.get_mut(&edge.source_id) {
					adj.retain(|e| e.target_id != id);
					if adj.is_empty() {
						self.adjacency.remove(&edge.source_id);
					}
				}
			}
		}
	}

	/// Outgoing edges from `id`, sorted by weight descending.
	pub fn neighbors(&self, id: &str) -> Vec<&Edge> {
		let mut result: Vec<&Edge> = match self.adjacency.get(id) {
			Some(edges) => edges.iter().collect(),
			None => return vec![],
		};
		result.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
		result
	}

	/// Outgoing edges from `id` filtered to the given types, sorted by
	/// weight descending.
	pub fn neighbors_by_type(&self, id: &str, types: &[EdgeType]) -> Vec<&Edge> {
		let mut result: Vec<&Edge> = match self.adjacency.get(id) {
			Some(edges) => edges
				.iter()
				.filter(|e| types.contains(&e.edge_type))
				.collect(),
			None => return vec![],
		};
		result.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
		result
	}

	// -- Metadata parsing -------------------------------------------------

	/// Parse `rel:*` keys from volume metadata and create explicit edges.
	///
	/// Supported keys:
	///   `rel:related`     → Related (bidirectional), weight 1.0
	///   `rel:parent`      → Parent (source→target) + Child (target→source), weight 1.0
	///   `rel:extends`     → Extends (directed source→target only), weight 1.0
	///   `rel:contradicts`  → Contradicts (bidirectional), weight 1.0
	///
	/// Values are comma-separated volume IDs.
	pub fn parse_metadata_edges(
		&mut self,
		source_id: &str,
		metadata: &HashMap<String, String>,
		timestamp: u64,
	) {
		for (key, value) in metadata {
			let targets: Vec<&str> = value.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

			match key.as_str() {
				"rel:related" => {
					for target in &targets {
						self.add_bidirectional_edge(
							source_id,
							target,
							EdgeType::Related,
							1.0,
							EdgeOrigin::Explicit,
							timestamp,
						);
					}
				}
				"rel:parent" => {
					for target in &targets {
						// source → target = Parent
						self.add_edge(Edge {
							source_id: source_id.to_string(),
							target_id: target.to_string(),
							edge_type: EdgeType::Parent,
							weight: 1.0,
							origin: EdgeOrigin::Explicit,
							timestamp,
						});
						// target → source = Child
						self.add_edge(Edge {
							source_id: target.to_string(),
							target_id: source_id.to_string(),
							edge_type: EdgeType::Child,
							weight: 1.0,
							origin: EdgeOrigin::Explicit,
							timestamp,
						});
					}
				}
				"rel:extends" => {
					for target in &targets {
						self.add_edge(Edge {
							source_id: source_id.to_string(),
							target_id: target.to_string(),
							edge_type: EdgeType::Extends,
							weight: 1.0,
							origin: EdgeOrigin::Explicit,
							timestamp,
						});
					}
				}
				"rel:contradicts" => {
					for target in &targets {
						self.add_bidirectional_edge(
							source_id,
							target,
							EdgeType::Contradicts,
							1.0,
							EdgeOrigin::Explicit,
							timestamp,
						);
					}
				}
				_ => {}
			}
		}
	}

	// -- Implicit similarity edges ----------------------------------------

	/// Whether an explicit edge exists between `a` and `b` (in either direction).
	pub fn has_explicit_edge(&self, a: &str, b: &str) -> bool {
		let check = |src: &str, tgt: &str| -> bool {
			if let Some(edges) = self.adjacency.get(src) {
				edges
					.iter()
					.any(|e| e.target_id == tgt && e.origin == EdgeOrigin::Explicit)
			} else {
				false
			}
		};
		check(a, b) || check(b, a)
	}

	/// Create a `Similar` edge (bidirectional, Similarity origin) when
	/// `similarity >= config.similarity_threshold` and no explicit edge
	/// exists between the pair.
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
		if self.has_explicit_edge(a, b) {
			return;
		}
		self.add_bidirectional_edge(
			a,
			b,
			EdgeType::Similar,
			similarity,
			EdgeOrigin::Similarity,
			timestamp,
		);
	}

	// -- Implicit correlation edges ---------------------------------------

	/// Sync correlation edges from the learning engine's co-occurrence data.
	///
	/// `correlations` maps entry_id → { peer_id → count }.
	/// `max_count` is the maximum co-occurrence count across all pairs
	/// (used to normalize weight to [0, 1]).
	///
	/// Creates `CoOccurs` edges for pairs whose count >= `correlation_threshold`.
	/// Skips pairs that already have an explicit edge.
	pub fn sync_correlations(
		&mut self,
		correlations: &HashMap<String, HashMap<String, usize>>,
		max_count: usize,
		timestamp: u64,
	) {
		if max_count == 0 {
			return;
		}

		// Track pairs already processed to avoid double-adding
		let mut processed: HashSet<(String, String)> = HashSet::new();

		for (entry_id, peers) in correlations {
			for (peer_id, &count) in peers {
				if count < self.config.correlation_threshold {
					continue;
				}
				// Normalize pair order so we only process each pair once
				let pair = if entry_id < peer_id {
					(entry_id.clone(), peer_id.clone())
				} else {
					(peer_id.clone(), entry_id.clone())
				};
				if processed.contains(&pair) {
					continue;
				}
				processed.insert(pair);

				if self.has_explicit_edge(entry_id, peer_id) {
					continue;
				}

				let weight = count as f64 / max_count as f64;
				self.add_bidirectional_edge(
					entry_id,
					peer_id,
					EdgeType::CoOccurs,
					weight,
					EdgeOrigin::Correlation,
					timestamp,
				);
			}
		}
	}

	/// Remove implicit (Similarity / Correlation) edges with weight below
	/// `min_weight`. Cleans up empty adjacency entries.
	pub fn prune_weak_implicit_edges(&mut self, min_weight: f64) {
		let sources: Vec<String> = self.adjacency.keys().cloned().collect();

		for source in &sources {
			if let Some(edges) = self.adjacency.get_mut(source) {
				let to_remove: Vec<(String, EdgeType)> = edges
					.iter()
					.filter(|e| {
						e.origin != EdgeOrigin::Explicit && e.weight < min_weight
					})
					.map(|e| (e.target_id.clone(), e.edge_type.clone()))
					.collect();

				for (target_id, edge_type) in &to_remove {
					// Remove from reverse index
					if let Some(rev) = self.reverse.get_mut(target_id) {
						rev.retain(|e| {
							!(e.source_id == *source && e.edge_type == *edge_type)
						});
					}
				}

				edges.retain(|e| {
					e.origin == EdgeOrigin::Explicit || e.weight >= min_weight
				});
			}
		}

		// Clean up empty entries
		self.adjacency.retain(|_, edges| !edges.is_empty());
		self.reverse.retain(|_, edges| !edges.is_empty());
	}

	// -- Traversal --------------------------------------------------------

	/// BFS traversal from `start_id` up to `max_depth` hops.
	///
	/// Optionally filters by `edge_types`. Deduplicates visited nodes.
	/// The start node is never included in the results.
	/// Results are capped at `max_results`.
	pub fn traverse(
		&self,
		start_id: &str,
		max_depth: usize,
		edge_types: Option<&[EdgeType]>,
		max_results: usize,
	) -> Vec<TraversalNode> {
		let mut visited: HashSet<String> = HashSet::new();
		visited.insert(start_id.to_string());

		let mut queue: VecDeque<(String, usize, Vec<String>)> = VecDeque::new();
		queue.push_back((start_id.to_string(), 0, vec![start_id.to_string()]));

		let mut results: Vec<TraversalNode> = Vec::new();

		while let Some((current_id, depth, path)) = queue.pop_front() {
			if depth >= max_depth {
				continue;
			}

			let edges = match edge_types {
				Some(types) => self.neighbors_by_type(&current_id, types),
				None => self.neighbors(&current_id),
			};

			for edge in edges {
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

	// -- Scoring ----------------------------------------------------------

	/// Compute the graph-based relevance score for `candidate_id` given a
	/// set of known-relevant IDs.
	///
	/// Returns the maximum edge weight from `candidate_id` to any entry
	/// in `relevant_ids`. Returns 0.0 if no connection exists.
	pub fn compute_graph_score(&self, candidate_id: &str, relevant_ids: &[String]) -> f64 {
		let edges = match self.adjacency.get(candidate_id) {
			Some(e) => e,
			None => return 0.0,
		};

		let mut max_weight: f64 = 0.0;
		for edge in edges {
			if relevant_ids.contains(&edge.target_id) && edge.weight > max_weight {
				max_weight = edge.weight;
			}
		}
		max_weight
	}

	/// Blend an existing search/recommendation score with the graph score.
	///
	/// `blended = (1 - w) * existing + w * graph` where
	/// `w = config.graph_boost_weight`.
	pub fn apply_graph_boost(&self, existing_score: f64, graph_score: f64) -> f64 {
		let w = self.config.graph_boost_weight;
		(1.0 - w) * existing_score + w * graph_score
	}

	// -- Serialization ----------------------------------------------------

	/// Serialize the graph to a persistable state.
	///
	/// Only explicit edges are persisted — implicit edges are rebuilt
	/// from similarity data and the learning engine on load.
	pub fn serialize(&self) -> GraphState {
		let mut explicit_edges: Vec<EdgeSerialized> = Vec::new();

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

	/// Rebuild a graph from persisted state.
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

// ---------------------------------------------------------------------------
// Traversal result
// ---------------------------------------------------------------------------

/// A node discovered during BFS traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraversalNode {
	pub node_id: String,
	pub depth: usize,
	pub path: Vec<String>,
}

// ---------------------------------------------------------------------------
// Serialization types
// ---------------------------------------------------------------------------

/// A single edge in serialized form (no origin — always Explicit).
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

/// Persistable snapshot of the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphState {
	#[serde(rename = "explicitEdges")]
	pub explicit_edges: Vec<EdgeSerialized>,
	pub config: GraphConfig,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashMap;

	fn default_graph() -> GraphIndex {
		GraphIndex::new(GraphConfig::default())
	}

	fn make_edge(
		source: &str,
		target: &str,
		edge_type: EdgeType,
		weight: f64,
		origin: EdgeOrigin,
	) -> Edge {
		Edge {
			source_id: source.to_string(),
			target_id: target.to_string(),
			edge_type,
			weight,
			origin,
			timestamp: 1000,
		}
	}

	// -- Task 1: Data model -----------------------------------------------

	#[test]
	fn create_graph_index_with_default_config() {
		let graph = default_graph();
		assert_eq!(graph.config().similarity_threshold, 0.85);
		assert_eq!(graph.config().correlation_threshold, 3);
		assert_eq!(graph.config().max_edges_per_node, 50);
		assert_eq!(graph.config().graph_boost_weight, 0.15);
		assert_eq!(graph.edge_count(), 0);
	}

	// -- Task 2: Edge CRUD ------------------------------------------------

	#[test]
	fn add_edge_and_retrieve_neighbors() {
		let mut graph = default_graph();
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 0.9, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "c", EdgeType::Related, 0.7, EdgeOrigin::Explicit));

		let neighbors = graph.neighbors("a");
		assert_eq!(neighbors.len(), 2);
		// Should be sorted by weight desc
		assert_eq!(neighbors[0].target_id, "b");
		assert_eq!(neighbors[1].target_id, "c");
		assert_eq!(graph.edge_count(), 2);
	}

	#[test]
	fn bidirectional_edge_creates_both_directions() {
		let mut graph = default_graph();
		graph.add_bidirectional_edge("a", "b", EdgeType::Related, 1.0, EdgeOrigin::Explicit, 1000);

		let from_a = graph.neighbors("a");
		assert_eq!(from_a.len(), 1);
		assert_eq!(from_a[0].target_id, "b");

		let from_b = graph.neighbors("b");
		assert_eq!(from_b.len(), 1);
		assert_eq!(from_b[0].target_id, "a");

		assert_eq!(graph.edge_count(), 2);
	}

	#[test]
	fn remove_edges_for_node_cascades() {
		let mut graph = default_graph();
		graph.add_bidirectional_edge("a", "b", EdgeType::Related, 1.0, EdgeOrigin::Explicit, 1000);
		graph.add_bidirectional_edge("a", "c", EdgeType::Related, 0.8, EdgeOrigin::Explicit, 1000);
		graph.add_bidirectional_edge("b", "c", EdgeType::Related, 0.6, EdgeOrigin::Explicit, 1000);
		assert_eq!(graph.edge_count(), 6);

		graph.remove_node("a");

		// Edges from/to "a" should be gone
		assert!(graph.neighbors("a").is_empty());
		// "b" should only have edge to "c"
		let from_b = graph.neighbors("b");
		assert_eq!(from_b.len(), 1);
		assert_eq!(from_b[0].target_id, "c");
		// "c" should only have edge to "b"
		let from_c = graph.neighbors("c");
		assert_eq!(from_c.len(), 1);
		assert_eq!(from_c[0].target_id, "b");

		assert_eq!(graph.edge_count(), 2);
	}

	#[test]
	fn max_edges_per_node_evicts_weakest() {
		let mut graph = GraphIndex::new(GraphConfig {
			max_edges_per_node: 3,
			..Default::default()
		});

		graph.add_edge(make_edge("a", "b", EdgeType::Related, 0.5, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "c", EdgeType::Related, 0.8, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "d", EdgeType::Related, 0.3, EdgeOrigin::Explicit));
		assert_eq!(graph.neighbors("a").len(), 3);

		// Adding a 4th edge should evict the weakest (d at 0.3)
		graph.add_edge(make_edge("a", "e", EdgeType::Related, 0.9, EdgeOrigin::Explicit));
		let neighbors = graph.neighbors("a");
		assert_eq!(neighbors.len(), 3);

		let ids: Vec<&str> = neighbors.iter().map(|e| e.target_id.as_str()).collect();
		assert!(ids.contains(&"e"));
		assert!(ids.contains(&"c"));
		assert!(ids.contains(&"b"));
		assert!(!ids.contains(&"d")); // weakest evicted
	}

	#[test]
	fn neighbors_filtered_by_edge_type() {
		let mut graph = default_graph();
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 0.9, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "c", EdgeType::Parent, 1.0, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "d", EdgeType::Similar, 0.85, EdgeOrigin::Similarity));

		let related = graph.neighbors_by_type("a", &[EdgeType::Related]);
		assert_eq!(related.len(), 1);
		assert_eq!(related[0].target_id, "b");

		let parent_and_similar =
			graph.neighbors_by_type("a", &[EdgeType::Parent, EdgeType::Similar]);
		assert_eq!(parent_and_similar.len(), 2);
	}

	#[test]
	fn update_edge_weight_when_stronger() {
		let mut graph = default_graph();
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 0.5, EdgeOrigin::Explicit));

		// Same source+target+type but stronger weight
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 0.9, EdgeOrigin::Explicit));

		let neighbors = graph.neighbors("a");
		assert_eq!(neighbors.len(), 1);
		assert!((neighbors[0].weight - 0.9).abs() < 1e-10);

		// Weaker weight should NOT update
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 0.3, EdgeOrigin::Explicit));
		let neighbors = graph.neighbors("a");
		assert!((neighbors[0].weight - 0.9).abs() < 1e-10);
	}

	// -- Task 3: Metadata parsing -----------------------------------------

	#[test]
	fn parse_rel_metadata_creates_related_edges() {
		let mut graph = default_graph();
		let mut metadata = HashMap::new();
		metadata.insert("rel:related".to_string(), "b,c".to_string());

		graph.parse_metadata_edges("a", &metadata, 1000);

		// Bidirectional: a→b, b→a, a→c, c→a
		assert_eq!(graph.edge_count(), 4);

		let from_a = graph.neighbors("a");
		assert_eq!(from_a.len(), 2);
		assert!(from_a.iter().all(|e| e.edge_type == EdgeType::Related));

		let from_b = graph.neighbors("b");
		assert_eq!(from_b.len(), 1);
		assert_eq!(from_b[0].target_id, "a");
	}

	#[test]
	fn parse_rel_parent_creates_parent_child_edges() {
		let mut graph = default_graph();
		let mut metadata = HashMap::new();
		metadata.insert("rel:parent".to_string(), "p1".to_string());

		graph.parse_metadata_edges("child1", &metadata, 1000);

		// child1 → p1 (Parent), p1 → child1 (Child)
		assert_eq!(graph.edge_count(), 2);

		let from_child = graph.neighbors("child1");
		assert_eq!(from_child.len(), 1);
		assert_eq!(from_child[0].edge_type, EdgeType::Parent);
		assert_eq!(from_child[0].target_id, "p1");

		let from_parent = graph.neighbors("p1");
		assert_eq!(from_parent.len(), 1);
		assert_eq!(from_parent[0].edge_type, EdgeType::Child);
		assert_eq!(from_parent[0].target_id, "child1");
	}

	#[test]
	fn parse_rel_extends_creates_directed_edge() {
		let mut graph = default_graph();
		let mut metadata = HashMap::new();
		metadata.insert("rel:extends".to_string(), "base".to_string());

		graph.parse_metadata_edges("derived", &metadata, 1000);

		// Only one direction: derived → base (Extends)
		assert_eq!(graph.edge_count(), 1);

		let from_derived = graph.neighbors("derived");
		assert_eq!(from_derived.len(), 1);
		assert_eq!(from_derived[0].edge_type, EdgeType::Extends);
		assert_eq!(from_derived[0].target_id, "base");

		// No reverse
		assert!(graph.neighbors("base").is_empty());
	}

	#[test]
	fn parse_rel_contradicts_creates_bidirectional_edges() {
		let mut graph = default_graph();
		let mut metadata = HashMap::new();
		metadata.insert("rel:contradicts".to_string(), "claim2".to_string());

		graph.parse_metadata_edges("claim1", &metadata, 1000);

		assert_eq!(graph.edge_count(), 2);

		let from_1 = graph.neighbors("claim1");
		assert_eq!(from_1.len(), 1);
		assert_eq!(from_1[0].edge_type, EdgeType::Contradicts);

		let from_2 = graph.neighbors("claim2");
		assert_eq!(from_2.len(), 1);
		assert_eq!(from_2[0].edge_type, EdgeType::Contradicts);
	}

	// -- Task 4: Implicit similarity edges --------------------------------

	#[test]
	fn add_similarity_edge_above_threshold() {
		let mut graph = default_graph(); // threshold = 0.85
		graph.add_similarity_edge("a", "b", 0.90, 1000);

		assert_eq!(graph.edge_count(), 2); // bidirectional
		let neighbors = graph.neighbors("a");
		assert_eq!(neighbors.len(), 1);
		assert_eq!(neighbors[0].edge_type, EdgeType::Similar);
		assert_eq!(neighbors[0].origin, EdgeOrigin::Similarity);
	}

	#[test]
	fn skip_similarity_edge_below_threshold() {
		let mut graph = default_graph(); // threshold = 0.85
		graph.add_similarity_edge("a", "b", 0.80, 1000);
		assert_eq!(graph.edge_count(), 0);
	}

	#[test]
	fn skip_similarity_edge_when_explicit_exists() {
		let mut graph = default_graph();
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 1.0, EdgeOrigin::Explicit));

		graph.add_similarity_edge("a", "b", 0.95, 2000);
		// Should still only have the original explicit edge
		let neighbors = graph.neighbors("a");
		assert_eq!(neighbors.len(), 1);
		assert_eq!(neighbors[0].origin, EdgeOrigin::Explicit);
	}

	// -- Task 5: Implicit correlation edges -------------------------------

	#[test]
	fn sync_correlation_above_threshold() {
		let mut graph = default_graph(); // correlation_threshold = 3

		let mut correlations: HashMap<String, HashMap<String, usize>> = HashMap::new();
		let mut peers = HashMap::new();
		peers.insert("b".to_string(), 5);
		correlations.insert("a".to_string(), peers);

		let mut peers_b = HashMap::new();
		peers_b.insert("a".to_string(), 5);
		correlations.insert("b".to_string(), peers_b);

		graph.sync_correlations(&correlations, 10, 1000);

		assert_eq!(graph.edge_count(), 2); // bidirectional
		let neighbors = graph.neighbors("a");
		assert_eq!(neighbors.len(), 1);
		assert_eq!(neighbors[0].edge_type, EdgeType::CoOccurs);
		assert_eq!(neighbors[0].origin, EdgeOrigin::Correlation);
		// weight = 5 / 10 = 0.5
		assert!((neighbors[0].weight - 0.5).abs() < 1e-10);
	}

	#[test]
	fn skip_correlation_below_threshold() {
		let mut graph = default_graph(); // correlation_threshold = 3

		let mut correlations: HashMap<String, HashMap<String, usize>> = HashMap::new();
		let mut peers = HashMap::new();
		peers.insert("b".to_string(), 2); // below threshold of 3
		correlations.insert("a".to_string(), peers);

		graph.sync_correlations(&correlations, 10, 1000);

		assert_eq!(graph.edge_count(), 0);
	}

	#[test]
	fn prune_implicit_edges_below_threshold() {
		let mut graph = default_graph();

		// Add an implicit similarity edge with low weight
		graph.add_bidirectional_edge(
			"a",
			"b",
			EdgeType::Similar,
			0.2,
			EdgeOrigin::Similarity,
			1000,
		);

		// Add an implicit correlation edge with higher weight
		graph.add_bidirectional_edge(
			"c",
			"d",
			EdgeType::CoOccurs,
			0.8,
			EdgeOrigin::Correlation,
			1000,
		);

		// Add an explicit edge with low weight (should NOT be pruned)
		graph.add_edge(make_edge("e", "f", EdgeType::Related, 0.1, EdgeOrigin::Explicit));

		assert_eq!(graph.edge_count(), 5); // 2 + 2 + 1

		graph.prune_weak_implicit_edges(0.5);

		// a↔b (0.2 similarity) pruned, c↔d (0.8 correlation) kept, e→f (explicit) kept
		assert_eq!(graph.edge_count(), 3);
		assert!(graph.neighbors("a").is_empty());
		assert!(!graph.neighbors("c").is_empty());
		assert!(!graph.neighbors("e").is_empty());
	}

	// -- Task 6: Traversal ------------------------------------------------

	#[test]
	fn traverse_one_hop() {
		let mut graph = default_graph();
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 1.0, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "c", EdgeType::Related, 0.8, EdgeOrigin::Explicit));

		let results = graph.traverse("a", 1, None, 100);
		assert_eq!(results.len(), 2);
		assert!(results.iter().all(|n| n.depth == 1));

		let ids: Vec<&str> = results.iter().map(|n| n.node_id.as_str()).collect();
		assert!(ids.contains(&"b"));
		assert!(ids.contains(&"c"));
	}

	#[test]
	fn traverse_two_hops() {
		let mut graph = default_graph();
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 1.0, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("b", "c", EdgeType::Related, 0.8, EdgeOrigin::Explicit));

		let results = graph.traverse("a", 2, None, 100);
		assert_eq!(results.len(), 2);

		// b at depth 1, c at depth 2
		let b_node = results.iter().find(|n| n.node_id == "b").unwrap();
		assert_eq!(b_node.depth, 1);
		assert_eq!(b_node.path, vec!["a", "b"]);

		let c_node = results.iter().find(|n| n.node_id == "c").unwrap();
		assert_eq!(c_node.depth, 2);
		assert_eq!(c_node.path, vec!["a", "b", "c"]);
	}

	#[test]
	fn traverse_deduplicates_visited_nodes() {
		let mut graph = default_graph();
		// a → b, a → c, b → c (c reachable via two paths)
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 1.0, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "c", EdgeType::Related, 0.8, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("b", "c", EdgeType::Related, 0.6, EdgeOrigin::Explicit));

		let results = graph.traverse("a", 2, None, 100);
		// c should only appear once (at depth 1 from a→c, since a's neighbors
		// are processed before b's)
		let c_count = results.iter().filter(|n| n.node_id == "c").count();
		assert_eq!(c_count, 1);
	}

	#[test]
	fn traverse_with_edge_type_filter() {
		let mut graph = default_graph();
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 1.0, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "c", EdgeType::Parent, 1.0, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "d", EdgeType::Similar, 0.9, EdgeOrigin::Similarity));

		let results = graph.traverse("a", 1, Some(&[EdgeType::Related]), 100);
		assert_eq!(results.len(), 1);
		assert_eq!(results[0].node_id, "b");
	}

	#[test]
	fn traverse_respects_max_results() {
		let mut graph = default_graph();
		graph.add_edge(make_edge("a", "b", EdgeType::Related, 1.0, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "c", EdgeType::Related, 0.9, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("a", "d", EdgeType::Related, 0.8, EdgeOrigin::Explicit));

		let results = graph.traverse("a", 1, None, 2);
		assert_eq!(results.len(), 2);
	}

	// -- Task 7: Scoring --------------------------------------------------

	#[test]
	fn compute_graph_score_for_connected_candidates() {
		let mut graph = default_graph();
		graph.add_edge(make_edge("x", "a", EdgeType::Related, 0.9, EdgeOrigin::Explicit));
		graph.add_edge(make_edge("x", "b", EdgeType::Similar, 0.7, EdgeOrigin::Similarity));

		let relevant = vec!["a".to_string(), "b".to_string()];
		let score = graph.compute_graph_score("x", &relevant);
		// max of 0.9 and 0.7
		assert!((score - 0.9).abs() < 1e-10);
	}

	#[test]
	fn graph_score_zero_for_unconnected_candidate() {
		let graph = default_graph();
		let relevant = vec!["a".to_string()];
		let score = graph.compute_graph_score("x", &relevant);
		assert_eq!(score, 0.0);
	}

	#[test]
	fn apply_graph_boost_to_existing_score() {
		let graph = default_graph(); // graph_boost_weight = 0.15
		let boosted = graph.apply_graph_boost(0.8, 1.0);
		// (1 - 0.15) * 0.8 + 0.15 * 1.0 = 0.68 + 0.15 = 0.83
		assert!((boosted - 0.83).abs() < 1e-10);
	}

	#[test]
	fn graph_boost_with_zero_graph_score_preserves_original() {
		let graph = default_graph();
		let boosted = graph.apply_graph_boost(0.8, 0.0);
		// (1 - 0.15) * 0.8 + 0.15 * 0.0 = 0.68
		assert!((boosted - 0.68).abs() < 1e-10);
	}

	// -- Task 8: Serialization --------------------------------------------

	#[test]
	fn serialize_deserialize_graph_state() {
		let mut graph = default_graph();
		graph.add_bidirectional_edge("a", "b", EdgeType::Related, 1.0, EdgeOrigin::Explicit, 1000);
		graph.add_similarity_edge("c", "d", 0.90, 2000); // implicit — should not be serialized

		let state = graph.serialize();
		assert_eq!(state.explicit_edges.len(), 2); // only a→b and b→a

		// Verify JSON round-trip
		let json = serde_json::to_string(&state).unwrap();
		let restored_state: GraphState = serde_json::from_str(&json).unwrap();

		let graph2 = GraphIndex::from_state(restored_state, GraphConfig::default());
		assert_eq!(graph2.edge_count(), 2);

		let from_a = graph2.neighbors("a");
		assert_eq!(from_a.len(), 1);
		assert_eq!(from_a[0].target_id, "b");
		assert_eq!(from_a[0].edge_type, EdgeType::Related);
	}

	#[test]
	fn empty_graph_serializes_to_empty_state() {
		let graph = default_graph();
		let state = graph.serialize();
		assert!(state.explicit_edges.is_empty());

		let json = serde_json::to_string(&state).unwrap();
		let restored_state: GraphState = serde_json::from_str(&json).unwrap();
		let graph2 = GraphIndex::from_state(restored_state, GraphConfig::default());
		assert_eq!(graph2.edge_count(), 0);
	}
}
