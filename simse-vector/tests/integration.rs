// ---------------------------------------------------------------------------
// Integration tests for simse-vector-engine JSON-RPC 2.0 / NDJSON protocol
// ---------------------------------------------------------------------------
//
// Each test spawns a fresh simse-vector-engine binary and communicates via
// stdin/stdout using newline-delimited JSON-RPC 2.0 messages.
// ---------------------------------------------------------------------------

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

struct VectorProcess {
	child: Child,
	reader: BufReader<std::process::ChildStdout>,
	next_id: AtomicU64,
}

impl VectorProcess {
	fn spawn() -> Self {
		let bin = env!("CARGO_BIN_EXE_simse-vector-engine");
		let mut child = Command::new(bin)
			.stdin(Stdio::piped())
			.stdout(Stdio::piped())
			.stderr(Stdio::null())
			.spawn()
			.expect("failed to spawn simse-vector-engine");

		let stdout = child.stdout.take().expect("no stdout");
		let reader = BufReader::new(stdout);

		Self {
			child,
			reader,
			next_id: AtomicU64::new(1),
		}
	}

	fn send(&mut self, method: &str, params: Value) -> RpcResponse {
		let id = self.next_id.fetch_add(1, Ordering::SeqCst);
		let request = json!({
			"jsonrpc": "2.0",
			"id": id,
			"method": method,
			"params": params,
		});

		let stdin = self.child.stdin.as_mut().expect("no stdin");
		let mut line = serde_json::to_string(&request).unwrap();
		line.push('\n');
		stdin.write_all(line.as_bytes()).unwrap();
		stdin.flush().unwrap();

		loop {
			let mut buf = String::new();
			let bytes_read = self
				.reader
				.read_line(&mut buf)
				.expect("failed to read from stdout");
			if bytes_read == 0 {
				panic!("unexpected EOF while waiting for response to id={}", id);
			}
			let buf = buf.trim();
			if buf.is_empty() {
				continue;
			}
			let parsed: Value = serde_json::from_str(buf)
				.unwrap_or_else(|e| panic!("invalid JSON from engine: {e}\nline: {buf}"));
			// Skip notifications (no id field)
			if parsed.get("id").is_none() {
				continue;
			}
			let resp_id = parsed["id"].as_u64().expect("response id is not u64");
			assert_eq!(resp_id, id, "response id mismatch");
			if let Some(error) = parsed.get("error") {
				return RpcResponse::Error(error.clone());
			}
			return RpcResponse::Ok(parsed.get("result").cloned().unwrap_or(Value::Null));
		}
	}

	fn call(&mut self, method: &str, params: Value) -> Value {
		match self.send(method, params) {
			RpcResponse::Ok(v) => v,
			RpcResponse::Error(e) => panic!("expected success, got error: {e}"),
		}
	}

	fn call_err(&mut self, method: &str, params: Value) -> Value {
		match self.send(method, params) {
			RpcResponse::Error(e) => e,
			RpcResponse::Ok(v) => panic!("expected error, got success: {v}"),
		}
	}

	/// Initialize store with defaults (no persistence).
	fn initialize(&mut self) -> Value {
		self.call("store/initialize", json!({}))
	}

	/// Initialize with a storage path for persistence.
	fn initialize_with_path(&mut self, path: &str) -> Value {
		self.call("store/initialize", json!({ "storagePath": path }))
	}
}

impl Drop for VectorProcess {
	fn drop(&mut self) {
		drop(self.child.stdin.take());
		let _ = self.child.wait();
	}
}

#[derive(Debug)]
enum RpcResponse {
	Ok(Value),
	Error(Value),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn initialize_and_add() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	// Add a volume
	let result = proc.call(
		"store/add",
		json!({
			"text": "hello world",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": { "topic": "test" }
		}),
	);
	let id = result["id"].as_str().expect("add should return id");
	assert!(!id.is_empty(), "id should be non-empty");

	// Get by ID and verify
	let result = proc.call("store/getById", json!({ "id": id }));
	let volume = &result["volume"];
	assert_eq!(volume["text"].as_str().unwrap(), "hello world");
	assert_eq!(volume["metadata"]["topic"].as_str().unwrap(), "test");
	assert_eq!(volume["id"].as_str().unwrap(), id);
}

#[test]
fn search_by_embedding() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	// Add 3 volumes with different embeddings
	proc.call(
		"store/add",
		json!({
			"text": "rust programming",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "python coding",
			"embedding": [0.9, 0.1, 0.0],
			"metadata": {}
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "cooking recipes",
			"embedding": [0.0, 0.0, 1.0],
			"metadata": {}
		}),
	);

	// Search with embedding close to "rust programming"
	let result = proc.call(
		"store/search",
		json!({
			"queryEmbedding": [1.0, 0.0, 0.0],
			"maxResults": 2,
			"threshold": 0.0
		}),
	);

	let results = result["results"].as_array().expect("results should be array");
	assert_eq!(results.len(), 2, "should return exactly 2 results");

	// First result should be "rust programming" (exact match, score = 1.0)
	assert_eq!(
		results[0]["volume"]["text"].as_str().unwrap(),
		"rust programming"
	);
}

#[test]
fn text_search_fuzzy() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	proc.call(
		"store/add",
		json!({
			"text": "machine learning algorithms",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "deep learning neural networks",
			"embedding": [0.0, 1.0, 0.0],
			"metadata": {}
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "cooking with herbs",
			"embedding": [0.0, 0.0, 1.0],
			"metadata": {}
		}),
	);

	// Text search with fuzzy mode
	let result = proc.call(
		"store/textSearch",
		json!({
			"query": "learning",
			"mode": "fuzzy"
		}),
	);

	let results = result["results"].as_array().expect("results should be array");
	assert!(
		results.len() >= 2,
		"should return at least the 2 learning-related volumes, got {}",
		results.len()
	);

	// Verify the learning-related volumes are in the results
	let texts: Vec<&str> = results
		.iter()
		.map(|r| r["volume"]["text"].as_str().unwrap())
		.collect();
	assert!(
		texts.contains(&"machine learning algorithms"),
		"should contain 'machine learning algorithms'"
	);
	assert!(
		texts.contains(&"deep learning neural networks"),
		"should contain 'deep learning neural networks'"
	);
}

#[test]
fn text_search_bm25() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	proc.call(
		"store/add",
		json!({
			"text": "machine learning algorithms for data science",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "deep learning neural networks and AI",
			"embedding": [0.0, 1.0, 0.0],
			"metadata": {}
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "cooking with herbs and spices",
			"embedding": [0.0, 0.0, 1.0],
			"metadata": {}
		}),
	);

	// Text search with bm25 mode
	let result = proc.call(
		"store/textSearch",
		json!({
			"query": "learning",
			"mode": "bm25"
		}),
	);

	let results = result["results"].as_array().expect("results should be array");
	assert!(
		!results.is_empty(),
		"bm25 search for 'learning' should return results"
	);

	// All results should contain "learning" in the text
	for r in results {
		let text = r["volume"]["text"].as_str().unwrap();
		assert!(
			text.contains("learning"),
			"bm25 result text should contain 'learning', got: {text}"
		);
	}
}

#[test]
fn metadata_filtering() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	proc.call(
		"store/add",
		json!({
			"text": "rust systems programming",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": { "language": "rust" }
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "python data science",
			"embedding": [0.0, 1.0, 0.0],
			"metadata": { "language": "python" }
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "web development tutorial",
			"embedding": [0.0, 0.0, 1.0],
			"metadata": { "type": "tutorial" }
		}),
	);

	// Filter by metadata: language = "rust"
	let result = proc.call(
		"store/filterByMetadata",
		json!({
			"filters": [{ "key": "language", "value": "rust" }]
		}),
	);

	let volumes = result["volumes"].as_array().expect("volumes should be array");
	assert_eq!(volumes.len(), 1, "should return exactly 1 volume");
	assert_eq!(
		volumes[0]["metadata"]["language"].as_str().unwrap(),
		"rust"
	);
	assert_eq!(
		volumes[0]["text"].as_str().unwrap(),
		"rust systems programming"
	);
}

#[test]
fn date_range_filtering() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	proc.call(
		"store/add",
		json!({
			"text": "volume one",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "volume two",
			"embedding": [0.0, 1.0, 0.0],
			"metadata": {}
		}),
	);

	// Filter with wide date range (should include everything)
	let result = proc.call(
		"store/filterByDateRange",
		json!({
			"after": 0,
			"before": 9999999999999_u64
		}),
	);
	let volumes = result["volumes"].as_array().expect("volumes should be array");
	assert_eq!(
		volumes.len(),
		2,
		"wide date range should return all volumes"
	);

	// Filter with before = 0 (nothing should match since timestamps are current)
	let result = proc.call("store/filterByDateRange", json!({ "before": 0 }));
	let volumes = result["volumes"].as_array().expect("volumes should be array");
	assert_eq!(
		volumes.len(),
		0,
		"before=0 should return no volumes since timestamps are current"
	);
}

#[test]
fn duplicate_detection() {
	let mut proc = VectorProcess::spawn();

	// Initialize with strict duplicate threshold and skip behavior
	proc.call(
		"store/initialize",
		json!({
			"duplicateThreshold": 0.99,
			"duplicateBehavior": "skip"
		}),
	);

	// Add first volume
	proc.call(
		"store/add",
		json!({
			"text": "unique text one",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);

	// Add duplicate with same embedding
	proc.call(
		"store/add",
		json!({
			"text": "duplicate text",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);

	// Check store size — should still be 1 because duplicate was skipped
	let result = proc.call("store/size", json!({}));
	let count = result["count"].as_u64().expect("count should be u64");
	assert_eq!(count, 1, "duplicate should have been skipped, size should be 1");
}

#[test]
fn delete_and_batch() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	// Add 3 volumes
	let r1 = proc.call(
		"store/add",
		json!({
			"text": "first volume",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);
	let id1 = r1["id"].as_str().unwrap().to_string();

	let r2 = proc.call(
		"store/add",
		json!({
			"text": "second volume",
			"embedding": [0.0, 1.0, 0.0],
			"metadata": {}
		}),
	);
	let id2 = r2["id"].as_str().unwrap().to_string();

	let r3 = proc.call(
		"store/add",
		json!({
			"text": "third volume",
			"embedding": [0.0, 0.0, 1.0],
			"metadata": {}
		}),
	);
	let id3 = r3["id"].as_str().unwrap().to_string();

	// Verify size = 3
	let result = proc.call("store/size", json!({}));
	assert_eq!(result["count"].as_u64().unwrap(), 3);

	// Delete first by ID
	let result = proc.call("store/delete", json!({ "id": id1 }));
	assert!(result["deleted"].as_bool().unwrap(), "delete should return true");

	// Verify size = 2
	let result = proc.call("store/size", json!({}));
	assert_eq!(result["count"].as_u64().unwrap(), 2);

	// Delete batch remaining 2
	let result = proc.call("store/deleteBatch", json!({ "ids": [id2, id3] }));
	assert_eq!(
		result["count"].as_u64().unwrap(),
		2,
		"deleteBatch should return count of 2"
	);

	// Verify size = 0
	let result = proc.call("store/size", json!({}));
	assert_eq!(result["count"].as_u64().unwrap(), 0);
}

#[test]
fn advanced_search() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	proc.call(
		"store/add",
		json!({
			"text": "rust systems programming language",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": { "language": "rust", "topic": "code" }
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "python data science library",
			"embedding": [0.0, 1.0, 0.0],
			"metadata": { "language": "python", "topic": "code" }
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "cooking healthy recipes",
			"embedding": [0.0, 0.0, 1.0],
			"metadata": { "type": "recipe" }
		}),
	);

	// Advanced search: embedding + text + metadata filter
	let result = proc.call(
		"store/advancedSearch",
		json!({
			"queryEmbedding": [1.0, 0.0, 0.0],
			"text": {
				"query": "programming",
				"mode": "fuzzy"
			},
			"metadata": [{ "key": "language", "value": "rust" }],
			"maxResults": 5
		}),
	);

	let results = result["results"].as_array().expect("results should be array");
	assert!(
		!results.is_empty(),
		"advanced search should return at least one result"
	);

	// First result should be the rust volume (matches all criteria)
	let first = &results[0];
	assert_eq!(first["volume"]["text"].as_str().unwrap(), "rust systems programming language");
	// Should have score breakdown
	assert!(first.get("scores").is_some(), "should include score breakdown");
}

#[test]
fn topic_operations() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	// Add volume with a topic
	let r = proc.call(
		"store/add",
		json!({
			"text": "rust ownership and borrowing",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": { "topic": "code/rust" }
		}),
	);
	let volume_id = r["id"].as_str().unwrap().to_string();

	// Get topics
	let result = proc.call("store/getTopics", json!({}));
	let topics = result["topics"].as_array().expect("topics should be array");
	let topic_names: Vec<&str> = topics
		.iter()
		.map(|t| t["topic"].as_str().unwrap())
		.collect();
	assert!(
		topic_names.contains(&"code/rust"),
		"topics should include 'code/rust', got: {:?}",
		topic_names
	);

	// Filter by topic
	let result = proc.call(
		"store/filterByTopic",
		json!({ "topics": ["code/rust"] }),
	);
	let volumes = result["volumes"].as_array().expect("volumes should be array");
	assert_eq!(volumes.len(), 1);
	assert_eq!(volumes[0]["id"].as_str().unwrap(), volume_id);

	// Catalog resolve
	let result = proc.call("catalog/resolve", json!({ "topic": "code/rust" }));
	assert!(
		result.get("resolved").is_some(),
		"catalog/resolve should return resolved field"
	);

	// Catalog sections
	let result = proc.call("catalog/sections", json!({}));
	assert!(
		result.get("sections").is_some(),
		"catalog/sections should return sections field"
	);
	let sections = result["sections"].as_array().expect("sections should be array");
	assert!(
		!sections.is_empty(),
		"should have at least one section for 'code/rust'"
	);
}

#[test]
fn recommendation() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	// Add multiple volumes
	proc.call(
		"store/add",
		json!({
			"text": "rust programming guide",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "python tutorial",
			"embedding": [0.0, 1.0, 0.0],
			"metadata": {}
		}),
	);
	proc.call(
		"store/add",
		json!({
			"text": "cooking basics",
			"embedding": [0.0, 0.0, 1.0],
			"metadata": {}
		}),
	);

	// Do a search to create access stats
	proc.call(
		"store/search",
		json!({
			"queryEmbedding": [1.0, 0.0, 0.0],
			"maxResults": 3,
			"threshold": 0.0
		}),
	);

	// Get recommendations
	let result = proc.call(
		"store/recommend",
		json!({
			"queryEmbedding": [1.0, 0.0, 0.0],
			"maxResults": 3
		}),
	);

	let results = result["results"].as_array().expect("results should be array");
	assert!(
		!results.is_empty(),
		"recommendation should return results"
	);

	// Each result should have a score
	for r in results {
		assert!(
			r.get("score").is_some(),
			"recommendation result should have a score"
		);
		let score = r["score"].as_f64().unwrap();
		assert!(score >= 0.0, "recommendation score should be >= 0");
	}
}

#[test]
fn learning_profile() {
	let mut proc = VectorProcess::spawn();

	// Initialize with learning enabled
	proc.call(
		"store/initialize",
		json!({ "learningEnabled": true }),
	);

	// Add volumes
	let r1 = proc.call(
		"store/add",
		json!({
			"text": "rust programming",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);
	let id1 = r1["id"].as_str().unwrap().to_string();

	proc.call(
		"store/add",
		json!({
			"text": "python scripting",
			"embedding": [0.0, 1.0, 0.0],
			"metadata": {}
		}),
	);

	// Record some queries to build the learning profile
	proc.call(
		"learning/recordQuery",
		json!({
			"embedding": [1.0, 0.0, 0.0],
			"selectedIds": [id1]
		}),
	);
	proc.call(
		"learning/recordQuery",
		json!({
			"embedding": [0.9, 0.1, 0.0],
			"selectedIds": [id1]
		}),
	);

	// Get profile
	let result = proc.call("learning/profile", json!({}));
	let profile = &result["profile"];
	assert!(
		profile.get("totalQueries").is_some(),
		"profile should have totalQueries"
	);
	let total = profile["totalQueries"].as_u64().unwrap();
	assert!(
		total >= 2,
		"totalQueries should be >= 2, got {}",
		total
	);
}

#[test]
fn persistence_round_trip() {
	let tmp = tempfile::tempdir().expect("failed to create tempdir");
	let storage_path = tmp.path().to_str().expect("tempdir path is not valid UTF-8");

	let (id1, id2);

	// --- First process: add volumes and save ---
	{
		let mut proc = VectorProcess::spawn();
		proc.initialize_with_path(storage_path);

		let r1 = proc.call(
			"store/add",
			json!({
				"text": "persisted volume one",
				"embedding": [1.0, 0.0, 0.0],
				"metadata": { "source": "test" }
			}),
		);
		id1 = r1["id"].as_str().unwrap().to_string();

		let r2 = proc.call(
			"store/add",
			json!({
				"text": "persisted volume two",
				"embedding": [0.0, 1.0, 0.0],
				"metadata": { "source": "test" }
			}),
		);
		id2 = r2["id"].as_str().unwrap().to_string();

		// Save to disk
		proc.call("store/save", json!({}));

		// Dispose (also saves if dirty, but we already saved)
		proc.call("store/dispose", json!({}));

		// Drop proc — stdin closes, child exits
	}

	// --- Second process: load from disk and verify ---
	{
		let mut proc = VectorProcess::spawn();
		proc.initialize_with_path(storage_path);

		// Verify size
		let result = proc.call("store/size", json!({}));
		assert_eq!(
			result["count"].as_u64().unwrap(),
			2,
			"should have 2 volumes after reload"
		);

		// Verify first volume
		let result = proc.call("store/getById", json!({ "id": id1 }));
		let vol = &result["volume"];
		assert_eq!(vol["text"].as_str().unwrap(), "persisted volume one");
		assert_eq!(vol["metadata"]["source"].as_str().unwrap(), "test");

		// Verify second volume
		let result = proc.call("store/getById", json!({ "id": id2 }));
		let vol = &result["volume"];
		assert_eq!(vol["text"].as_str().unwrap(), "persisted volume two");
		assert_eq!(vol["metadata"]["source"].as_str().unwrap(), "test");
	}
}

#[test]
fn error_before_init() {
	let mut proc = VectorProcess::spawn();

	// Do NOT call initialize — go straight to add
	let err = proc.call_err(
		"store/add",
		json!({
			"text": "should fail",
			"embedding": [1.0, 0.0, 0.0],
			"metadata": {}
		}),
	);

	// The error should indicate the store is not initialized
	let has_not_init_message = err
		.get("message")
		.and_then(|m| m.as_str())
		.map(|m| m.to_lowercase().contains("not initialized"))
		.unwrap_or(false);

	let has_not_init_code = err
		.get("data")
		.and_then(|d| d.get("vectorCode"))
		.and_then(|c| c.as_str())
		.map(|c| c == "STACKS_NOT_LOADED")
		.unwrap_or(false);

	assert!(
		has_not_init_message || has_not_init_code,
		"error should indicate store not initialized, got: {err}"
	);
}

#[test]
fn unknown_method() {
	let mut proc = VectorProcess::spawn();
	proc.initialize();

	// Call a nonexistent method
	let err = proc.call_err("nonexistent/method", json!({}));

	// Should get METHOD_NOT_FOUND error code (-32601)
	let code = err["code"].as_i64().expect("error should have code");
	assert_eq!(
		code, -32601,
		"unknown method should return METHOD_NOT_FOUND (-32601), got: {code}"
	);
}
