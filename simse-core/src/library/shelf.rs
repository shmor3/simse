//! Shelf — agent-scoped library partition.
//!
//! A `Shelf` is a thin wrapper around a [`Library`] that:
//! - Adds `metadata.shelf = name` to all `add` operations
//! - Filters search and `entries()` results to only include entries on this shelf
//! - Provides `search_global()` for unfiltered library-wide searches

use std::collections::HashMap;
use std::sync::Arc;

use simse_adaptive_engine::types::{Entry, Lookup};

use crate::error::SimseError;

use super::library::Library;

// ---------------------------------------------------------------------------
// Shelf
// ---------------------------------------------------------------------------

/// An agent-scoped partition within the library.
///
/// All `add` calls automatically tag entries with `metadata.shelf = name`.
/// Search and listing operations filter results to this shelf by default.
#[derive(Clone)]
pub struct Shelf {
	name: String,
	library: Arc<Library>,
}

impl Shelf {
	/// Create a new shelf wrapping the given library.
	pub fn new(name: String, library: Arc<Library>) -> Self {
		Self { name, library }
	}

	/// The shelf name.
	pub fn name(&self) -> &str {
		&self.name
	}

	/// Add text to the library with `metadata.shelf = name`.
	pub async fn add(
		&self,
		text: &str,
		metadata: HashMap<String, String>,
	) -> Result<String, SimseError> {
		let mut meta = metadata;
		meta.insert("shelf".to_string(), self.name.clone());
		self.library.add(text, meta).await
	}

	/// Search the library and filter results to this shelf.
	pub async fn search(
		&self,
		query: &str,
		max_results: Option<usize>,
		threshold: Option<f64>,
	) -> Result<Vec<Lookup>, SimseError> {
		let results = self.library.search(query, max_results, threshold).await?;
		Ok(results
			.into_iter()
			.filter(|r| r.entry.metadata.get("shelf").map(|s| s.as_str()) == Some(&self.name))
			.collect())
	}

	/// Search the library without shelf filtering.
	pub async fn search_global(
		&self,
		query: &str,
		max_results: Option<usize>,
		threshold: Option<f64>,
	) -> Result<Vec<Lookup>, SimseError> {
		self.library.search(query, max_results, threshold).await
	}

	/// Get all entries on this shelf.
	pub fn entries(&self) -> Result<Vec<Entry>, SimseError> {
		let all = self.library.get_all()?;
		Ok(all
			.into_iter()
			.filter(|v| v.metadata.get("shelf").map(|s| s.as_str()) == Some(&self.name))
			.collect())
	}
}
