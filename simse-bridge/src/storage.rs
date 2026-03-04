//! File-based storage backend with binary format and gzip compression.
//!
//! Stores key-value data (`String` → `Vec<u8>`) in a compact binary file.
//! The on-disk format is gzip-compressed with an inner binary layout:
//!
//! ```text
//! MAGIC ("SIMK", 4 bytes) | version (u16 BE) | count (u32 BE) | entries…
//! ```
//!
//! Each entry: `key_len (u32 BE) | key (UTF-8) | val_len (u32 BE) | val`.
//!
//! Writes are atomic by default (write to `.tmp`, then rename).

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use tokio::fs;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAGIC: &[u8; 4] = b"SIMK";
const FORMAT_VERSION: u16 = 1;

/// Minimum valid file size: 4 (magic) + 2 (version) + 4 (count) = 10 bytes.
const HEADER_SIZE: usize = 4 + 2 + 4;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Configuration for [`FileStorageBackend`].
#[derive(Debug, Clone)]
pub struct StorageOptions {
    /// If `true`, writes go to a `.tmp` file first, then are renamed into
    /// place. Defaults to `true`.
    pub atomic_write: bool,
    /// Gzip compression level (0-9). Defaults to `6`.
    pub compression_level: u32,
}

impl Default for StorageOptions {
    fn default() -> Self {
        Self {
            atomic_write: true,
            compression_level: 6,
        }
    }
}

// ---------------------------------------------------------------------------
// FileStorageBackend
// ---------------------------------------------------------------------------

/// A file-based storage backend that persists key-value data in a single
/// gzipped binary file.
pub struct FileStorageBackend {
    path: PathBuf,
    options: StorageOptions,
}

impl FileStorageBackend {
    /// Create a new backend targeting `path` with the given options.
    pub fn new(path: PathBuf, options: StorageOptions) -> Self {
        Self { path, options }
    }

    /// Load all entries from disk.
    ///
    /// Returns an empty map if the file does not exist or is empty.
    /// Automatically detects gzip compression (magic bytes `0x1f 0x8b`).
    pub async fn load(&self) -> io::Result<HashMap<String, Vec<u8>>> {
        // File missing → empty map
        if !self.path.exists() {
            return Ok(HashMap::new());
        }

        let mut raw = fs::read(&self.path).await?;

        if raw.is_empty() {
            return Ok(HashMap::new());
        }

        // Auto-detect gzip (magic 0x1f 0x8b)
        if raw.len() >= 2 && raw[0] == 0x1f && raw[1] == 0x8b {
            let mut decoder = GzDecoder::new(&raw[..]);
            let mut decompressed = Vec::new();
            decoder.read_to_end(&mut decompressed)?;
            raw = decompressed;
        }

        deserialize(&raw)
    }

    /// Persist all entries to disk.
    ///
    /// Creates parent directories as needed. Data is serialized into the
    /// binary format, gzip-compressed, and written atomically (if enabled).
    pub async fn save(&self, data: &HashMap<String, Vec<u8>>) -> io::Result<()> {
        // Ensure parent dirs exist
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let raw = serialize(data);

        // Gzip compress
        let mut encoder =
            GzEncoder::new(Vec::new(), Compression::new(self.options.compression_level));
        encoder.write_all(&raw)?;
        let compressed = encoder.finish()?;

        if self.options.atomic_write {
            let tmp_path = self.tmp_path();
            fs::write(&tmp_path, &compressed).await?;
            fs::rename(&tmp_path, &self.path).await?;
        } else {
            fs::write(&self.path, &compressed).await?;
        }

        Ok(())
    }

    /// Clean up any leftover temporary file.
    pub async fn close(&self) -> io::Result<()> {
        let tmp = self.tmp_path();
        if tmp.exists() {
            fs::remove_file(&tmp).await?;
        }
        Ok(())
    }

    /// Path to the temporary file used during atomic writes.
    fn tmp_path(&self) -> PathBuf {
        let mut p = self.path.as_os_str().to_owned();
        p.push(".tmp");
        PathBuf::from(p)
    }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

/// Serialize a map into the SIMK binary format.
fn serialize(data: &HashMap<String, Vec<u8>>) -> Vec<u8> {
    // Calculate total size
    let mut total = HEADER_SIZE;
    for (key, val) in data {
        total += 4 + key.len() + 4 + val.len();
    }

    let mut buf = Vec::with_capacity(total);

    // Header
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
    buf.extend_from_slice(&(data.len() as u32).to_be_bytes());

    // Entries
    for (key, val) in data {
        let key_bytes = key.as_bytes();
        buf.extend_from_slice(&(key_bytes.len() as u32).to_be_bytes());
        buf.extend_from_slice(key_bytes);
        buf.extend_from_slice(&(val.len() as u32).to_be_bytes());
        buf.extend_from_slice(val);
    }

    buf
}

/// Deserialize the SIMK binary format into a map.
fn deserialize(bytes: &[u8]) -> io::Result<HashMap<String, Vec<u8>>> {
    if bytes.len() < HEADER_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid storage file — missing SIMK magic header",
        ));
    }

    // Validate magic
    if &bytes[..4] != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid storage file — missing SIMK magic header",
        ));
    }

    // Version
    let version = u16::from_be_bytes([bytes[4], bytes[5]]);
    if version != FORMAT_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Unsupported storage format version: {version} (expected 1)"),
        ));
    }

    // Entry count
    let count = u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;

    let mut offset = HEADER_SIZE;
    let mut result = HashMap::with_capacity(count);

    for i in 0..count {
        // Key length
        if offset + 4 > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Corrupt storage file at entry {i}"),
            ));
        }
        let key_len =
            u32::from_be_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
                as usize;
        offset += 4;

        // Key bytes
        if offset + key_len > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Corrupt storage file at entry {i}"),
            ));
        }
        let key = String::from_utf8(bytes[offset..offset + key_len].to_vec()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Corrupt storage file at entry {i}"),
            )
        })?;
        offset += key_len;

        // Value length
        if offset + 4 > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Corrupt storage file at entry {i}"),
            ));
        }
        let val_len =
            u32::from_be_bytes([bytes[offset], bytes[offset + 1], bytes[offset + 2], bytes[offset + 3]])
                as usize;
        offset += 4;

        // Value bytes
        if offset + val_len > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Corrupt storage file at entry {i}"),
            ));
        }
        let val = bytes[offset..offset + val_len].to_vec();
        offset += val_len;

        result.insert(key, val);
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn load_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.simk");
        let backend = FileStorageBackend::new(path, StorageOptions::default());

        let data = backend.load().await.unwrap();
        assert!(data.is_empty());
    }

    #[tokio::test]
    async fn roundtrip_save_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.simk");
        let backend = FileStorageBackend::new(path, StorageOptions::default());

        let mut data = HashMap::new();
        data.insert("key1".to_string(), b"value1".to_vec());
        data.insert("key2".to_string(), b"value2".to_vec());
        data.insert("empty".to_string(), Vec::new());

        backend.save(&data).await.unwrap();
        let loaded = backend.load().await.unwrap();

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.get("key1").unwrap(), b"value1");
        assert_eq!(loaded.get("key2").unwrap(), b"value2");
        assert_eq!(loaded.get("empty").unwrap(), &Vec::<u8>::new());
    }

    #[tokio::test]
    async fn atomic_write_no_tmp_remains() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("atomic.simk");
        let backend = FileStorageBackend::new(
            path.clone(),
            StorageOptions {
                atomic_write: true,
                ..Default::default()
            },
        );

        let mut data = HashMap::new();
        data.insert("k".to_string(), b"v".to_vec());
        backend.save(&data).await.unwrap();

        // The .tmp file should not remain after a successful save
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        assert!(!PathBuf::from(tmp).exists());
        // But the real file should exist
        assert!(path.exists());
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let mut data = HashMap::new();
        data.insert("alpha".to_string(), b"hello world".to_vec());
        data.insert("beta".to_string(), vec![0u8, 1, 2, 255]);
        data.insert("gamma".to_string(), Vec::new());

        let serialized = serialize(&data);
        let deserialized = deserialize(&serialized).unwrap();

        assert_eq!(deserialized.len(), data.len());
        for (k, v) in &data {
            assert_eq!(deserialized.get(k).unwrap(), v);
        }
    }

    #[test]
    fn deserialize_detects_bad_magic() {
        let bad = b"BADKxxxxxxxx";
        let err = deserialize(bad).unwrap_err();
        assert!(
            err.to_string()
                .contains("Invalid storage file — missing SIMK magic header"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn deserialize_detects_bad_version() {
        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&99u16.to_be_bytes()); // bad version
        buf.extend_from_slice(&0u32.to_be_bytes()); // count = 0

        let err = deserialize(&buf).unwrap_err();
        assert!(
            err.to_string()
                .contains("Unsupported storage format version: 99 (expected 1)"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn gzip_auto_detection() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("gzipped.simk");

        // Manually create gzipped binary data and write it
        let mut data = HashMap::new();
        data.insert("test_key".to_string(), b"test_value".to_vec());
        let raw = serialize(&data);

        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(6));
        encoder.write_all(&raw).unwrap();
        let compressed = encoder.finish().unwrap();

        // Verify it starts with gzip magic
        assert_eq!(compressed[0], 0x1f);
        assert_eq!(compressed[1], 0x8b);

        std::fs::write(&path, &compressed).unwrap();

        let backend = FileStorageBackend::new(path, StorageOptions::default());
        let loaded = backend.load().await.unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.get("test_key").unwrap(), b"test_value");
    }

    #[tokio::test]
    async fn non_atomic_write() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("direct.simk");
        let backend = FileStorageBackend::new(
            path.clone(),
            StorageOptions {
                atomic_write: false,
                ..Default::default()
            },
        );

        let mut data = HashMap::new();
        data.insert("k".to_string(), b"v".to_vec());
        backend.save(&data).await.unwrap();

        assert!(path.exists());

        // Verify data can be loaded back
        let loaded = backend.load().await.unwrap();
        assert_eq!(loaded.get("k").unwrap(), b"v");
    }

    #[tokio::test]
    async fn creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("a").join("b").join("c").join("deep.simk");
        let backend = FileStorageBackend::new(path.clone(), StorageOptions::default());

        let mut data = HashMap::new();
        data.insert("deep".to_string(), b"nested".to_vec());
        backend.save(&data).await.unwrap();

        assert!(path.exists());
        let loaded = backend.load().await.unwrap();
        assert_eq!(loaded.get("deep").unwrap(), b"nested");
    }

    #[test]
    fn deserialize_truncated_entry() {
        // Valid header claiming 1 entry, but no entry data
        let mut buf = Vec::new();
        buf.extend_from_slice(MAGIC);
        buf.extend_from_slice(&FORMAT_VERSION.to_be_bytes());
        buf.extend_from_slice(&1u32.to_be_bytes()); // count = 1 but no entry data

        let err = deserialize(&buf).unwrap_err();
        assert!(
            err.to_string().contains("Corrupt storage file at entry 0"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn close_removes_tmp_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("closable.simk");
        let backend = FileStorageBackend::new(path.clone(), StorageOptions::default());

        // Manually create a leftover .tmp file
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp_path = PathBuf::from(&tmp);
        std::fs::write(&tmp_path, b"leftover").unwrap();
        assert!(tmp_path.exists());

        backend.close().await.unwrap();
        assert!(!tmp_path.exists());
    }

    #[test]
    fn deserialize_too_small() {
        let buf = b"SIM"; // Only 3 bytes, too small for header
        let err = deserialize(buf).unwrap_err();
        assert!(
            err.to_string()
                .contains("Invalid storage file — missing SIMK magic header"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn serialize_empty_map() {
        let data = HashMap::new();
        let serialized = serialize(&data);

        // Should be exactly HEADER_SIZE bytes
        assert_eq!(serialized.len(), HEADER_SIZE);
        assert_eq!(&serialized[..4], MAGIC);

        let deserialized = deserialize(&serialized).unwrap();
        assert!(deserialized.is_empty());
    }

    #[test]
    fn binary_keys_with_unicode() {
        let mut data = HashMap::new();
        data.insert("cafe\u{0301}".to_string(), b"coffee".to_vec());
        data.insert("\u{1F600}".to_string(), b"emoji".to_vec());

        let serialized = serialize(&data);
        let deserialized = deserialize(&serialized).unwrap();

        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized.get("cafe\u{0301}").unwrap(), b"coffee");
        assert_eq!(deserialized.get("\u{1F600}").unwrap(), b"emoji");
    }
}
