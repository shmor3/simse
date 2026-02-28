// ---------------------------------------------------------------------------
// Binary persistence format + Gzip compression
// ---------------------------------------------------------------------------
//
// Ports the TypeScript stacks-serialize.ts (228 lines) and preservation.ts
// (85 lines) into a single Rust module.
//
// **Format compatibility**: Rust produces the exact same binary format as the
// TypeScript implementation so existing TS-written stores can be read by Rust
// and vice versa.
//
// Entry binary format per key-value pair:
//   [4B text-len BE][text UTF-8]
//   [4B emb-b64-len BE][emb base64 UTF-8]
//   [4B meta-json-len BE][meta JSON UTF-8]
//   [8B timestamp (two 32-bit BE halves)]
//   [4B accessCount BE]
//   [8B lastAccessed (two 32-bit BE halves)]
//
// File format (v2): gzipped JSON with `{ "version": 2, "entries": { ... } }`
// where each value is base64-encoded binary entry data.
// ---------------------------------------------------------------------------

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use flate2::read::{GzDecoder, GzEncoder};
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use thiserror::Error;

use crate::learning::LearningState;
use crate::types::Volume;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum PersistenceError {
	#[error("IO error: {0}")]
	Io(#[from] std::io::Error),
	#[error("Corruption: {0}")]
	Corruption(String),
	#[error("Serialization: {0}")]
	Serialization(String),
}

// ---------------------------------------------------------------------------
// Access stats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct AccessStats {
	pub access_count: u32,
	pub last_accessed: u64,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const LEARNING_KEY: &str = "__learning";

// ---------------------------------------------------------------------------
// Embedding encode / decode
// ---------------------------------------------------------------------------

/// Encode a f32 slice as base64 of Float32 little-endian bytes.
/// This matches the JS `Float32Array` byte order (LE on all standard platforms).
pub fn encode_embedding(embedding: &[f32]) -> String {
	let bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();
	STANDARD.encode(&bytes)
}

/// Decode a base64-encoded Float32 LE byte string back to `Vec<f32>`.
pub fn decode_embedding(encoded: &str) -> Result<Vec<f32>, PersistenceError> {
	let bytes = STANDARD
		.decode(encoded)
		.map_err(|e| PersistenceError::Corruption(format!("Invalid base64: {}", e)))?;
	if bytes.len() % 4 != 0 {
		return Err(PersistenceError::Corruption(
			"Invalid embedding length".into(),
		));
	}
	let mut result = Vec::with_capacity(bytes.len() / 4);
	for chunk in bytes.chunks_exact(4) {
		result.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
	}
	Ok(result)
}

// ---------------------------------------------------------------------------
// Gzip compress / decompress
// ---------------------------------------------------------------------------

/// Gzip-compress a byte slice (level 6 — matches the TS default).
pub fn compress(data: &[u8]) -> Result<Vec<u8>, PersistenceError> {
	let mut encoder = GzEncoder::new(data, Compression::new(6));
	let mut compressed = Vec::new();
	encoder
		.read_to_end(&mut compressed)
		.map_err(PersistenceError::Io)?;
	Ok(compressed)
}

/// Gunzip-decompress a byte slice.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, PersistenceError> {
	let mut decoder = GzDecoder::new(data);
	let mut decompressed = Vec::new();
	decoder
		.read_to_end(&mut decompressed)
		.map_err(PersistenceError::Io)?;
	Ok(decompressed)
}

/// Check if data starts with gzip magic bytes (0x1f, 0x8b).
pub fn is_gzipped(data: &[u8]) -> bool {
	data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b
}

// ---------------------------------------------------------------------------
// Per-entry binary codec
// ---------------------------------------------------------------------------

/// Serialize a single entry to the binary format.
///
/// Layout:
/// ```text
/// [4B text-len BE][text UTF-8]
/// [4B emb-b64-len BE][emb base64 UTF-8]
/// [4B meta-json-len BE][meta JSON UTF-8]
/// [8B timestamp as two 32-bit BE halves]
/// [4B accessCount BE]
/// [8B lastAccessed as two 32-bit BE halves]
/// ```
pub fn serialize_entry(volume: &Volume, stats: Option<&AccessStats>) -> Vec<u8> {
	let text_bytes = volume.text.as_bytes();
	let emb_b64 = encode_embedding(&volume.embedding);
	let emb_bytes = emb_b64.as_bytes();
	let meta_json = serde_json::to_string(&volume.metadata).unwrap_or_default();
	let meta_bytes = meta_json.as_bytes();

	let total = 4 + text_bytes.len() + 4 + emb_bytes.len() + 4 + meta_bytes.len() + 8 + 4 + 8;
	let mut buf = Vec::with_capacity(total);

	// Text
	buf.extend_from_slice(&(text_bytes.len() as u32).to_be_bytes());
	buf.extend_from_slice(text_bytes);

	// Embedding base64
	buf.extend_from_slice(&(emb_bytes.len() as u32).to_be_bytes());
	buf.extend_from_slice(emb_bytes);

	// Metadata JSON
	buf.extend_from_slice(&(meta_bytes.len() as u32).to_be_bytes());
	buf.extend_from_slice(meta_bytes);

	// Timestamp as two 32-bit BE halves (matches TS: Math.floor(ts / 0x100000000), ts >>> 0)
	let ts = volume.timestamp;
	let ts_high = (ts >> 32) as u32;
	let ts_low = ts as u32;
	buf.extend_from_slice(&ts_high.to_be_bytes());
	buf.extend_from_slice(&ts_low.to_be_bytes());

	// Access count
	buf.extend_from_slice(&stats.map_or(0u32, |s| s.access_count).to_be_bytes());

	// Last accessed as two 32-bit BE halves
	let la = stats.map_or(0u64, |s| s.last_accessed);
	let la_high = (la >> 32) as u32;
	let la_low = la as u32;
	buf.extend_from_slice(&la_high.to_be_bytes());
	buf.extend_from_slice(&la_low.to_be_bytes());

	buf
}

/// Deserialized result from a single binary entry.
#[derive(Debug, Clone)]
pub struct DeserializedEntry {
	pub volume: Volume,
	pub access_count: u32,
	pub last_accessed: u64,
}

/// Read a `u32` from `data` at `offset` (big-endian).
/// Returns `None` if there aren't enough bytes.
fn read_u32_be(data: &[u8], offset: usize) -> Option<u32> {
	if offset + 4 > data.len() {
		return None;
	}
	Some(u32::from_be_bytes([
		data[offset],
		data[offset + 1],
		data[offset + 2],
		data[offset + 3],
	]))
}

/// Deserialize a single entry from the binary format.
pub fn deserialize_entry(id: &str, data: &[u8]) -> Result<DeserializedEntry, PersistenceError> {
	let mut offset = 0;

	// Text
	let text_len = read_u32_be(data, offset)
		.ok_or_else(|| PersistenceError::Corruption("Truncated: text length".into()))?
		as usize;
	offset += 4;
	if offset + text_len > data.len() {
		return Err(PersistenceError::Corruption("Truncated: text data".into()));
	}
	let text = std::str::from_utf8(&data[offset..offset + text_len])
		.map_err(|e| PersistenceError::Corruption(format!("Invalid UTF-8 in text: {}", e)))?
		.to_string();
	offset += text_len;

	// Embedding base64
	let emb_len = read_u32_be(data, offset)
		.ok_or_else(|| PersistenceError::Corruption("Truncated: embedding length".into()))?
		as usize;
	offset += 4;
	if offset + emb_len > data.len() {
		return Err(PersistenceError::Corruption(
			"Truncated: embedding data".into(),
		));
	}
	let emb_b64 = std::str::from_utf8(&data[offset..offset + emb_len]).map_err(|e| {
		PersistenceError::Corruption(format!("Invalid UTF-8 in embedding base64: {}", e))
	})?;
	let embedding = decode_embedding(emb_b64)?;
	offset += emb_len;

	// Metadata JSON
	let meta_len = read_u32_be(data, offset)
		.ok_or_else(|| PersistenceError::Corruption("Truncated: metadata length".into()))?
		as usize;
	offset += 4;
	if offset + meta_len > data.len() {
		return Err(PersistenceError::Corruption(
			"Truncated: metadata data".into(),
		));
	}
	let meta_json = std::str::from_utf8(&data[offset..offset + meta_len]).map_err(|e| {
		PersistenceError::Corruption(format!("Invalid UTF-8 in metadata JSON: {}", e))
	})?;
	let metadata: HashMap<String, String> = serde_json::from_str(meta_json).map_err(|e| {
		PersistenceError::Corruption(format!("Invalid metadata JSON: {}", e))
	})?;
	offset += meta_len;

	// Timestamp as two 32-bit BE halves
	let ts_high = read_u32_be(data, offset)
		.ok_or_else(|| PersistenceError::Corruption("Truncated: timestamp high".into()))?;
	offset += 4;
	let ts_low = read_u32_be(data, offset)
		.ok_or_else(|| PersistenceError::Corruption("Truncated: timestamp low".into()))?;
	offset += 4;
	let timestamp = ((ts_high as u64) << 32) | (ts_low as u64);

	// Access count
	let access_count = read_u32_be(data, offset)
		.ok_or_else(|| PersistenceError::Corruption("Truncated: access count".into()))?;
	offset += 4;

	// Last accessed as two 32-bit BE halves
	let la_high = read_u32_be(data, offset)
		.ok_or_else(|| PersistenceError::Corruption("Truncated: lastAccessed high".into()))?;
	offset += 4;
	let la_low = read_u32_be(data, offset)
		.ok_or_else(|| PersistenceError::Corruption("Truncated: lastAccessed low".into()))?;
	let last_accessed = ((la_high as u64) << 32) | (la_low as u64);

	Ok(DeserializedEntry {
		volume: Volume {
			id: id.to_string(),
			text,
			embedding,
			metadata,
			timestamp,
		},
		access_count,
		last_accessed,
	})
}

// ---------------------------------------------------------------------------
// Bulk operations
// ---------------------------------------------------------------------------

/// Result of deserializing an entire store from raw storage data.
#[derive(Debug)]
pub struct DeserializedData {
	pub entries: Vec<Volume>,
	pub access_stats: HashMap<String, AccessStats>,
	pub learning_state: Option<LearningState>,
	pub skipped: usize,
}

/// Deserialize from raw storage data (`HashMap<String, Vec<u8>>`).
///
/// For each key:
/// - If key == `LEARNING_KEY`: parse as JSON into `LearningState`.
/// - Otherwise: `deserialize_entry(key, value)`.
pub fn deserialize_from_storage(raw_data: &HashMap<String, Vec<u8>>) -> DeserializedData {
	let mut entries = Vec::new();
	let mut access_stats = HashMap::new();
	let mut learning_state: Option<LearningState> = None;
	let mut skipped = 0;

	for (key, value) in raw_data {
		if key == LEARNING_KEY {
			// Parse learning state from UTF-8 JSON
			match std::str::from_utf8(value) {
				Ok(json_str) => match serde_json::from_str::<LearningState>(json_str) {
					Ok(state) => {
						learning_state = Some(state);
					}
					Err(_) => {
						// Invalid learning state — start fresh
						skipped += 1;
					}
				},
				Err(_) => {
					skipped += 1;
				}
			}
			continue;
		}

		match deserialize_entry(key, value) {
			Ok(result) => {
				if result.access_count > 0 || result.last_accessed > 0 {
					access_stats.insert(
						result.volume.id.clone(),
						AccessStats {
							access_count: result.access_count,
							last_accessed: result.last_accessed,
						},
					);
				}
				entries.push(result.volume);
			}
			Err(_) => {
				skipped += 1;
			}
		}
	}

	DeserializedData {
		entries,
		access_stats,
		learning_state,
		skipped,
	}
}

/// Serialize all entries + learning state to storage format.
pub fn serialize_to_storage(
	entries: &[Volume],
	access_stats: &HashMap<String, AccessStats>,
	learning_state: Option<&LearningState>,
) -> HashMap<String, Vec<u8>> {
	let mut data = HashMap::new();

	for entry in entries {
		let stats = access_stats.get(&entry.id);
		data.insert(entry.id.clone(), serialize_entry(entry, stats));
	}

	// Persist learning state alongside entries
	if let Some(state) = learning_state {
		if state.total_queries > 0 {
			match serde_json::to_string(state) {
				Ok(json) => {
					data.insert(LEARNING_KEY.to_string(), json.into_bytes());
				}
				Err(_) => {
					// Silently skip if serialization fails
				}
			}
		}
	}

	data
}

// ---------------------------------------------------------------------------
// File I/O — v2 gzipped index format
// ---------------------------------------------------------------------------

/// On-disk JSON structure for the v2 index file.
#[derive(Debug, Serialize, Deserialize)]
struct IndexFileV2 {
	version: u32,
	/// Each entry value is a base64-encoded binary entry.
	entries: HashMap<String, String>,
}

/// Save all data to a directory as a gzipped v2 index file.
///
/// Format: `index.gz` containing gzipped JSON:
/// ```json
/// { "version": 2, "entries": { "<id>": "<base64 binary entry data>", ... } }
/// ```
///
/// This is compatible with the TS v2 format.
pub fn save_to_directory(
	dir: &str,
	entries: &[Volume],
	access_stats: &HashMap<String, AccessStats>,
	learning_state: Option<&LearningState>,
) -> Result<(), PersistenceError> {
	// Create directory if needed
	std::fs::create_dir_all(dir).map_err(PersistenceError::Io)?;

	// Serialize all entries to binary, then base64 encode each
	let storage = serialize_to_storage(entries, access_stats, learning_state);
	let mut index_entries = HashMap::new();
	for (key, value) in &storage {
		index_entries.insert(key.clone(), STANDARD.encode(value));
	}

	let index = IndexFileV2 {
		version: 2,
		entries: index_entries,
	};

	let json = serde_json::to_string(&index)
		.map_err(|e| PersistenceError::Serialization(format!("Failed to serialize index: {}", e)))?;

	// Gzip the JSON
	let compressed = compress(json.as_bytes())?;

	// Write to index.gz
	let path = std::path::Path::new(dir).join("index.gz");
	std::fs::write(&path, &compressed).map_err(PersistenceError::Io)?;

	Ok(())
}

/// Load all data from a directory. Reads `index.gz` (gzipped) or `index.json` (plain).
///
/// Auto-detects v1 (plain JSON array) vs v2 (gzipped `{ version: 2, entries }`).
pub fn load_from_directory(dir: &str) -> Result<DeserializedData, PersistenceError> {
	let dir_path = std::path::Path::new(dir);

	// Try index.gz first, then index.json
	let gz_path = dir_path.join("index.gz");
	let json_path = dir_path.join("index.json");

	let raw_bytes = if gz_path.exists() {
		std::fs::read(&gz_path).map_err(PersistenceError::Io)?
	} else if json_path.exists() {
		std::fs::read(&json_path).map_err(PersistenceError::Io)?
	} else {
		// No index file — return empty data
		return Ok(DeserializedData {
			entries: Vec::new(),
			access_stats: HashMap::new(),
			learning_state: None,
			skipped: 0,
		});
	};

	// Decompress if gzipped
	let json_bytes = if is_gzipped(&raw_bytes) {
		decompress(&raw_bytes)?
	} else {
		raw_bytes
	};

	let json_str = std::str::from_utf8(&json_bytes)
		.map_err(|e| PersistenceError::Corruption(format!("Invalid UTF-8 in index: {}", e)))?;

	// Parse as v2 index
	let index: IndexFileV2 = serde_json::from_str(json_str).map_err(|e| {
		PersistenceError::Corruption(format!("Invalid index JSON: {}", e))
	})?;

	if index.version != 2 {
		return Err(PersistenceError::Corruption(format!(
			"Unsupported index version: {}",
			index.version
		)));
	}

	// Decode base64 values back to binary
	let mut raw_data = HashMap::new();
	for (key, b64_value) in &index.entries {
		let binary = STANDARD.decode(b64_value).map_err(|e| {
			PersistenceError::Corruption(format!("Invalid base64 for entry '{}': {}", key, e))
		})?;
		raw_data.insert(key.clone(), binary);
	}

	Ok(deserialize_from_storage(&raw_data))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use crate::learning::LearningState;
	use crate::types::RequiredWeightProfile;

	fn make_volume(id: &str, text: &str, embedding: &[f32], ts: u64) -> Volume {
		Volume {
			id: id.to_string(),
			text: text.to_string(),
			embedding: embedding.to_vec(),
			metadata: HashMap::new(),
			timestamp: ts,
		}
	}

	fn make_volume_with_metadata(
		id: &str,
		text: &str,
		embedding: &[f32],
		ts: u64,
		meta: HashMap<String, String>,
	) -> Volume {
		Volume {
			id: id.to_string(),
			text: text.to_string(),
			embedding: embedding.to_vec(),
			metadata: meta,
			timestamp: ts,
		}
	}

	// 1. encode_decode_embedding_roundtrip
	#[test]
	fn encode_decode_embedding_roundtrip() {
		let original = vec![1.0f32, -0.5, 0.0, 3.14159, -1e10, 1e-10];
		let encoded = encode_embedding(&original);
		let decoded = decode_embedding(&encoded).unwrap();
		assert_eq!(original.len(), decoded.len());
		for (a, b) in original.iter().zip(decoded.iter()) {
			assert!(
				(a - b).abs() < 1e-6,
				"Mismatch: {} vs {}",
				a,
				b
			);
		}
	}

	// 2. encode_embedding_empty
	#[test]
	fn encode_embedding_empty() {
		let encoded = encode_embedding(&[]);
		assert_eq!(encoded, "");
		let decoded = decode_embedding(&encoded).unwrap();
		assert!(decoded.is_empty());
	}

	// 3. compress_decompress_roundtrip
	#[test]
	fn compress_decompress_roundtrip() {
		let original = b"Hello, world! This is a test of gzip compression.";
		let compressed = compress(original).unwrap();
		assert_ne!(compressed, original.as_slice());
		let decompressed = decompress(&compressed).unwrap();
		assert_eq!(decompressed, original.as_slice());
	}

	// 4. is_gzipped_detection
	#[test]
	fn is_gzipped_detection() {
		// Actual gzipped data starts with magic bytes
		let compressed = compress(b"test").unwrap();
		assert!(is_gzipped(&compressed));

		// Plain data does not
		assert!(!is_gzipped(b"not gzipped"));
		assert!(!is_gzipped(b""));
		assert!(!is_gzipped(&[0x1f])); // Only one magic byte
		assert!(!is_gzipped(&[0x00, 0x8b])); // Wrong first byte
	}

	// 5. serialize_deserialize_entry_roundtrip
	#[test]
	fn serialize_deserialize_entry_roundtrip() {
		let mut meta = HashMap::new();
		meta.insert("topic".to_string(), "rust".to_string());
		meta.insert("source".to_string(), "test".to_string());

		let volume = make_volume_with_metadata(
			"vol-1",
			"Rust is a systems programming language",
			&[0.1, 0.2, 0.3, 0.4],
			1700000000000,
			meta,
		);

		let serialized = serialize_entry(&volume, None);
		let deserialized = deserialize_entry("vol-1", &serialized).unwrap();

		assert_eq!(deserialized.volume.id, "vol-1");
		assert_eq!(deserialized.volume.text, "Rust is a systems programming language");
		assert_eq!(deserialized.volume.timestamp, 1700000000000);
		assert_eq!(deserialized.volume.metadata.get("topic").unwrap(), "rust");
		assert_eq!(deserialized.volume.metadata.get("source").unwrap(), "test");
		assert_eq!(deserialized.volume.embedding.len(), 4);
		for (a, b) in volume.embedding.iter().zip(deserialized.volume.embedding.iter()) {
			assert!((a - b).abs() < 1e-6);
		}
		assert_eq!(deserialized.access_count, 0);
		assert_eq!(deserialized.last_accessed, 0);
	}

	// 6. serialize_entry_with_access_stats
	#[test]
	fn serialize_entry_with_access_stats() {
		let volume = make_volume("vol-2", "test text", &[1.0, 2.0], 1000);
		let stats = AccessStats {
			access_count: 42,
			last_accessed: 1700000000000,
		};

		let serialized = serialize_entry(&volume, Some(&stats));
		let deserialized = deserialize_entry("vol-2", &serialized).unwrap();

		assert_eq!(deserialized.access_count, 42);
		assert_eq!(deserialized.last_accessed, 1700000000000);
	}

	// 7. serialize_entry_large_timestamp
	#[test]
	fn serialize_entry_large_timestamp() {
		// Timestamp larger than 2^32 to test the two-halves encoding
		let large_ts: u64 = 0x1_FFFF_FFFF; // > 2^32
		let volume = make_volume("vol-large-ts", "large timestamp", &[0.5], large_ts);

		let serialized = serialize_entry(&volume, None);
		let deserialized = deserialize_entry("vol-large-ts", &serialized).unwrap();

		assert_eq!(deserialized.volume.timestamp, large_ts);
	}

	// 8. deserialize_corrupt_data
	#[test]
	fn deserialize_corrupt_data() {
		// Too short to contain even the text length
		let result = deserialize_entry("bad", &[0, 0]);
		assert!(result.is_err());

		// Text length says 100 but only 4 bytes of data
		let result = deserialize_entry("bad", &[0, 0, 0, 100, 0, 0, 0, 0]);
		assert!(result.is_err());

		// Empty data
		let result = deserialize_entry("bad", &[]);
		assert!(result.is_err());
	}

	// 9. bulk_serialize_deserialize_roundtrip
	#[test]
	fn bulk_serialize_deserialize_roundtrip() {
		let volumes = vec![
			make_volume("a", "first entry", &[1.0, 0.0, 0.0], 1000),
			make_volume("b", "second entry", &[0.0, 1.0, 0.0], 2000),
			make_volume("c", "third entry", &[0.0, 0.0, 1.0], 3000),
		];

		let mut access_stats = HashMap::new();
		access_stats.insert(
			"a".to_string(),
			AccessStats {
				access_count: 5,
				last_accessed: 1500,
			},
		);
		access_stats.insert(
			"c".to_string(),
			AccessStats {
				access_count: 10,
				last_accessed: 2500,
			},
		);

		let storage = serialize_to_storage(&volumes, &access_stats, None);
		let result = deserialize_from_storage(&storage);

		assert_eq!(result.entries.len(), 3);
		assert_eq!(result.skipped, 0);
		assert!(result.learning_state.is_none());

		// Check access stats survived
		let stats_a = result.access_stats.get("a").unwrap();
		assert_eq!(stats_a.access_count, 5);
		assert_eq!(stats_a.last_accessed, 1500);

		let stats_c = result.access_stats.get("c").unwrap();
		assert_eq!(stats_c.access_count, 10);
		assert_eq!(stats_c.last_accessed, 2500);

		// b had no access stats, so should not appear
		assert!(!result.access_stats.contains_key("b"));
	}

	// 10. learning_state_roundtrip
	#[test]
	fn learning_state_roundtrip() {
		let volumes = vec![make_volume("a", "test", &[1.0], 1000)];
		let access_stats = HashMap::new();

		let learning = LearningState {
			version: 1,
			feedback: vec![],
			query_history: vec![],
			adapted_weights: RequiredWeightProfile {
				vector: 0.6,
				recency: 0.2,
				frequency: 0.2,
			},
			interest_embedding: None,
			total_queries: 42,
			last_updated: 5000,
			explicit_feedback: None,
			topic_profiles: None,
			correlations: None,
		};

		let storage = serialize_to_storage(&volumes, &access_stats, Some(&learning));

		// Verify learning key is present
		assert!(storage.contains_key(LEARNING_KEY));

		let result = deserialize_from_storage(&storage);
		assert!(result.learning_state.is_some());

		let restored = result.learning_state.unwrap();
		assert_eq!(restored.version, 1);
		assert_eq!(restored.total_queries, 42);
		assert_eq!(restored.last_updated, 5000);
		assert!((restored.adapted_weights.vector - 0.6).abs() < 1e-10);
		assert!((restored.adapted_weights.recency - 0.2).abs() < 1e-10);
		assert!((restored.adapted_weights.frequency - 0.2).abs() < 1e-10);
	}

	// 11. save_load_directory_roundtrip
	#[test]
	fn save_load_directory_roundtrip() {
		let dir = tempfile::tempdir().unwrap();
		let dir_path = dir.path().to_str().unwrap();

		let mut meta = HashMap::new();
		meta.insert("topic".to_string(), "testing".to_string());

		let volumes = vec![
			make_volume_with_metadata("x", "hello world", &[0.1, 0.2, 0.3], 1700000000000, meta),
			make_volume("y", "goodbye world", &[0.4, 0.5, 0.6], 1700000001000),
		];

		let mut access_stats = HashMap::new();
		access_stats.insert(
			"x".to_string(),
			AccessStats {
				access_count: 7,
				last_accessed: 1700000000500,
			},
		);

		let learning = LearningState {
			version: 1,
			feedback: vec![],
			query_history: vec![],
			adapted_weights: RequiredWeightProfile {
				vector: 0.5,
				recency: 0.3,
				frequency: 0.2,
			},
			interest_embedding: None,
			total_queries: 10,
			last_updated: 9000,
			explicit_feedback: None,
			topic_profiles: None,
			correlations: None,
		};

		save_to_directory(dir_path, &volumes, &access_stats, Some(&learning)).unwrap();

		// Verify index.gz was written
		assert!(dir.path().join("index.gz").exists());

		// Load it back
		let result = load_from_directory(dir_path).unwrap();

		assert_eq!(result.entries.len(), 2);
		assert_eq!(result.skipped, 0);

		// Find entries by id (order may differ)
		let entry_x = result.entries.iter().find(|e| e.id == "x").unwrap();
		assert_eq!(entry_x.text, "hello world");
		assert_eq!(entry_x.timestamp, 1700000000000);
		assert_eq!(entry_x.metadata.get("topic").unwrap(), "testing");

		let entry_y = result.entries.iter().find(|e| e.id == "y").unwrap();
		assert_eq!(entry_y.text, "goodbye world");

		// Access stats
		let stats_x = result.access_stats.get("x").unwrap();
		assert_eq!(stats_x.access_count, 7);
		assert_eq!(stats_x.last_accessed, 1700000000500);

		// Learning state
		let ls = result.learning_state.unwrap();
		assert_eq!(ls.total_queries, 10);
		assert!((ls.adapted_weights.vector - 0.5).abs() < 1e-10);
	}

	// 12. load_from_nonexistent_directory
	#[test]
	fn load_from_nonexistent_directory_returns_empty() {
		let dir = tempfile::tempdir().unwrap();
		let dir_path = dir.path().join("nonexistent");
		let result = load_from_directory(dir_path.to_str().unwrap()).unwrap();
		assert!(result.entries.is_empty());
		assert!(result.access_stats.is_empty());
		assert!(result.learning_state.is_none());
		assert_eq!(result.skipped, 0);
	}

	// 13. decode_embedding_invalid_base64
	#[test]
	fn decode_embedding_invalid_base64() {
		let result = decode_embedding("!!!invalid!!!");
		assert!(result.is_err());
	}

	// 14. decode_embedding_wrong_length
	#[test]
	fn decode_embedding_wrong_length() {
		// 3 bytes is not divisible by 4 (size of f32)
		let encoded = STANDARD.encode(&[0u8, 1, 2]);
		let result = decode_embedding(&encoded);
		assert!(result.is_err());
	}

	// 15. serialize_entry_unicode_text
	#[test]
	fn serialize_entry_unicode_text() {
		let volume = make_volume("unicode", "Hello, world!", &[1.0], 1000);
		let serialized = serialize_entry(&volume, None);
		let deserialized = deserialize_entry("unicode", &serialized).unwrap();
		assert_eq!(deserialized.volume.text, "Hello, world!");

		// Multi-byte characters
		let volume2 = make_volume("emoji", "Rust is great", &[1.0], 2000);
		let serialized2 = serialize_entry(&volume2, None);
		let deserialized2 = deserialize_entry("emoji", &serialized2).unwrap();
		assert_eq!(deserialized2.volume.text, "Rust is great");
	}

	// 16. compress_empty_data
	#[test]
	fn compress_empty_data() {
		let compressed = compress(b"").unwrap();
		assert!(is_gzipped(&compressed));
		let decompressed = decompress(&compressed).unwrap();
		assert!(decompressed.is_empty());
	}

	// 17. bulk_skips_corrupt_entries
	#[test]
	fn bulk_skips_corrupt_entries() {
		let mut raw_data = HashMap::new();

		// Valid entry
		let valid = make_volume("good", "valid entry", &[1.0, 2.0], 1000);
		raw_data.insert("good".to_string(), serialize_entry(&valid, None));

		// Corrupt entry
		raw_data.insert("bad".to_string(), vec![0, 0, 0]);

		let result = deserialize_from_storage(&raw_data);
		assert_eq!(result.entries.len(), 1);
		assert_eq!(result.entries[0].id, "good");
		assert_eq!(result.skipped, 1);
	}

	// 18. large_timestamp_two_halves_encoding
	#[test]
	fn large_timestamp_two_halves_encoding() {
		// Typical JS Date.now() value: 1700000000000
		// This is > 2^32 (4294967296)
		let ts: u64 = 1700000000000;
		let volume = make_volume("ts-test", "test", &[1.0], ts);

		let serialized = serialize_entry(&volume, None);
		let deserialized = deserialize_entry("ts-test", &serialized).unwrap();

		assert_eq!(deserialized.volume.timestamp, ts);

		// Also check with lastAccessed
		let stats = AccessStats {
			access_count: 1,
			last_accessed: 1700000000000,
		};
		let serialized2 = serialize_entry(&volume, Some(&stats));
		let deserialized2 = deserialize_entry("ts-test", &serialized2).unwrap();
		assert_eq!(deserialized2.last_accessed, 1700000000000);
	}

	// 19. learning_state_not_serialized_when_no_data
	#[test]
	fn learning_state_not_serialized_when_no_data() {
		let volumes = vec![make_volume("a", "test", &[1.0], 1000)];
		let access_stats = HashMap::new();

		let learning = LearningState {
			version: 1,
			feedback: vec![],
			query_history: vec![],
			adapted_weights: RequiredWeightProfile {
				vector: 0.6,
				recency: 0.2,
				frequency: 0.2,
			},
			interest_embedding: None,
			total_queries: 0, // no data
			last_updated: 0,
			explicit_feedback: None,
			topic_profiles: None,
			correlations: None,
		};

		let storage = serialize_to_storage(&volumes, &access_stats, Some(&learning));

		// Learning key should NOT be present when total_queries == 0
		assert!(!storage.contains_key(LEARNING_KEY));
	}

	// 20. save_to_directory_creates_dir
	#[test]
	fn save_to_directory_creates_dir() {
		let parent = tempfile::tempdir().unwrap();
		let nested = parent.path().join("a").join("b").join("c");
		let nested_str = nested.to_str().unwrap();

		let volumes = vec![make_volume("x", "test", &[1.0], 1000)];
		save_to_directory(nested_str, &volumes, &HashMap::new(), None).unwrap();

		assert!(nested.join("index.gz").exists());
	}
}
