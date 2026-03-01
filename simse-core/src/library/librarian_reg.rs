//! Librarian Registry — manages multiple librarians with bidding, arbitration,
//! and specialist spawning.
//!
//! Ports `src/ai/library/librarian-registry.ts` (~469 lines) to Rust.
//!
//! - `LibrarianRegistry` — multi-librarian management
//! - `initialize()` — loads definitions from disk, creates default librarian
//! - `register()` / `unregister()` — with disk persistence
//! - `resolve_librarian()` — topic-glob filtering, bid collection, self-resolution gap, LLM arbitration
//! - `spawn_specialist()` — LLM-driven specialist generation

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::error::SimseError;
use crate::library::TextGenerationProvider;

use super::librarian::{
	ArbitrationResult, Librarian, LibrarianBid, LibrarianLibraryAccess, LibrarianOptions,
};
use super::librarian_def::{
	default_definition, load_all_definitions, matches_topic, save_definition,
	validate_definition, LibrarianDefinition,
};

// ---------------------------------------------------------------------------
// Disposable connection
// ---------------------------------------------------------------------------

/// Disposable connection handle for librarians with external ACP connections.
#[async_trait]
pub trait DisposableConnection: Send + Sync {
	async fn close(&self) -> Result<(), SimseError>;
}

// ---------------------------------------------------------------------------
// ManagedLibrarian
// ---------------------------------------------------------------------------

/// A librarian bundled with its definition, provider, and optional connection.
pub struct ManagedLibrarian {
	pub definition: LibrarianDefinition,
	pub librarian: Arc<Librarian>,
	pub provider: Arc<dyn TextGenerationProvider>,
	pub connection: Option<Arc<dyn DisposableConnection>>,
}

// ---------------------------------------------------------------------------
// Connection factory
// ---------------------------------------------------------------------------

/// Factory for creating connections to external librarian ACP servers.
#[async_trait]
pub trait ConnectionFactory: Send + Sync {
	async fn create_connection(
		&self,
		definition: &LibrarianDefinition,
	) -> Result<(Arc<dyn DisposableConnection>, Arc<dyn TextGenerationProvider>), SimseError>;
}

// ---------------------------------------------------------------------------
// Registry options
// ---------------------------------------------------------------------------

/// Options for creating a `LibrarianRegistry`.
pub struct LibrarianRegistryOptions {
	pub librarians_dir: PathBuf,
	pub library: Arc<dyn LibrarianLibraryAccess>,
	pub default_provider: Arc<dyn TextGenerationProvider>,
	pub self_resolution_gap: f64,
	pub connection_factory: Option<Arc<dyn ConnectionFactory>>,
}

impl LibrarianRegistryOptions {
	pub fn new(
		librarians_dir: PathBuf,
		library: Arc<dyn LibrarianLibraryAccess>,
		default_provider: Arc<dyn TextGenerationProvider>,
	) -> Self {
		Self {
			librarians_dir,
			library,
			default_provider,
			self_resolution_gap: 0.3,
			connection_factory: None,
		}
	}
}

// ---------------------------------------------------------------------------
// Internal JSON helpers for LLM responses
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ArbitrationJson {
	winner: Option<String>,
	reason: Option<String>,
}

#[derive(Deserialize)]
struct SpawnAssessmentJson {
	#[serde(rename = "shouldSpawn")]
	should_spawn: Option<bool>,
	reason: Option<String>,
}

// ---------------------------------------------------------------------------
// LibrarianRegistry
// ---------------------------------------------------------------------------

/// Multi-librarian manager with bidding, arbitration, and specialist spawning.
pub struct LibrarianRegistry {
	librarians_dir: PathBuf,
	library: Arc<dyn LibrarianLibraryAccess>,
	default_provider: Arc<dyn TextGenerationProvider>,
	self_resolution_gap: f64,
	connection_factory: Option<Arc<dyn ConnectionFactory>>,
	librarians: Mutex<HashMap<String, Arc<ManagedLibrarian>>>,
	initialized: Mutex<bool>,
}

impl LibrarianRegistry {
	/// Create a new registry (not yet initialized).
	pub fn new(options: LibrarianRegistryOptions) -> Self {
		Self {
			librarians_dir: options.librarians_dir,
			library: options.library,
			default_provider: options.default_provider,
			self_resolution_gap: options.self_resolution_gap,
			connection_factory: options.connection_factory,
			librarians: Mutex::new(HashMap::new()),
			initialized: Mutex::new(false),
		}
	}

	// -----------------------------------------------------------------------
	// Helpers
	// -----------------------------------------------------------------------

	async fn build_managed(
		&self,
		definition: LibrarianDefinition,
	) -> Result<ManagedLibrarian, SimseError> {
		if definition.acp.is_some() {
			if let Some(ref factory) = self.connection_factory {
				let (connection, provider) = factory.create_connection(&definition).await?;
				let librarian = Librarian::new(
					Arc::clone(&provider),
					Some(LibrarianOptions {
						name: Some(definition.name.clone()),
						purpose: Some(definition.purpose.clone()),
					}),
				);
				return Ok(ManagedLibrarian {
					definition,
					librarian: Arc::new(librarian),
					provider,
					connection: Some(connection),
				});
			}
		}

		let librarian = Librarian::new(
			Arc::clone(&self.default_provider),
			Some(LibrarianOptions {
				name: Some(definition.name.clone()),
				purpose: Some(definition.purpose.clone()),
			}),
		);
		Ok(ManagedLibrarian {
			definition,
			librarian: Arc::new(librarian),
			provider: Arc::clone(&self.default_provider),
			connection: None,
		})
	}

	// -----------------------------------------------------------------------
	// Lifecycle
	// -----------------------------------------------------------------------

	/// Initialize the registry: create the default librarian and load
	/// definitions from disk.
	pub async fn initialize(&self) -> Result<(), SimseError> {
		{
			let init = self.initialized.lock().await;
			if *init {
				return Ok(());
			}
		}

		// Create default librarian
		let default_managed = self.build_managed(default_definition()).await?;
		{
			let mut libs = self.librarians.lock().await;
			libs.insert("default".to_string(), Arc::new(default_managed));
		}

		// Load definitions from disk
		let definitions = load_all_definitions(&self.librarians_dir).await;
		for def in definitions {
			if def.name == "default" {
				continue;
			}
			match self.build_managed(def.clone()).await {
				Ok(managed) => {
					let mut libs = self.librarians.lock().await;
					libs.insert(def.name.clone(), Arc::new(managed));
				}
				Err(_) => {
					// Skip failed librarians
				}
			}
		}

		{
			let mut init = self.initialized.lock().await;
			*init = true;
		}

		Ok(())
	}

	/// Dispose all librarians and close connections.
	pub async fn dispose(&self) {
		let librarians = {
			let mut libs = self.librarians.lock().await;
			let drained: HashMap<String, Arc<ManagedLibrarian>> = libs.drain().collect();
			drained
		};

		for (_, managed) in librarians {
			if let Some(ref conn) = managed.connection {
				let _ = conn.close().await;
			}
		}

		{
			let mut init = self.initialized.lock().await;
			*init = false;
		}
	}

	// -----------------------------------------------------------------------
	// Register / unregister
	// -----------------------------------------------------------------------

	/// Register a new librarian from a definition.
	///
	/// Validates the definition, builds a managed librarian, and persists
	/// the definition to disk.
	pub async fn register(
		&self,
		definition: LibrarianDefinition,
	) -> Result<Arc<ManagedLibrarian>, SimseError> {
		let result = validate_definition(&definition);
		if !result.valid {
			return Err(SimseError::library(
				crate::error::LibraryErrorCode::InvalidInput,
				format!(
					"Invalid librarian definition: {}",
					result.errors.join(", ")
				),
			));
		}

		{
			let libs = self.librarians.lock().await;
			if libs.contains_key(&definition.name) {
				return Err(SimseError::library(
					crate::error::LibraryErrorCode::InvalidInput,
					format!(
						"Librarian \"{}\" is already registered",
						definition.name
					),
				));
			}
		}

		let managed = Arc::new(self.build_managed(definition.clone()).await?);

		{
			let mut libs = self.librarians.lock().await;
			libs.insert(definition.name.clone(), Arc::clone(&managed));
		}

		// Persist to disk (ignore errors)
		let _ = save_definition(&self.librarians_dir, &definition).await;

		Ok(managed)
	}

	/// Unregister a librarian by name.
	///
	/// Cannot unregister the default librarian.
	pub async fn unregister(&self, name: &str) -> Result<(), SimseError> {
		if name == "default" {
			return Err(SimseError::library(
				crate::error::LibraryErrorCode::InvalidInput,
				"Cannot unregister the default librarian",
			));
		}

		let managed = {
			let mut libs = self.librarians.lock().await;
			libs.remove(name)
		};

		if let Some(managed) = managed {
			if let Some(ref conn) = managed.connection {
				let _ = conn.close().await;
			}
		}

		// Delete file from disk (ignore errors)
		let file_path = self.librarians_dir.join(format!("{}.json", name));
		let _ = tokio::fs::remove_file(file_path).await;

		Ok(())
	}

	// -----------------------------------------------------------------------
	// Accessors
	// -----------------------------------------------------------------------

	/// Get a managed librarian by name.
	pub async fn get(&self, name: &str) -> Option<Arc<ManagedLibrarian>> {
		let libs = self.librarians.lock().await;
		libs.get(name).cloned()
	}

	/// List all managed librarians.
	pub async fn list(&self) -> Vec<Arc<ManagedLibrarian>> {
		let libs = self.librarians.lock().await;
		libs.values().cloned().collect()
	}

	/// Get the default librarian.
	///
	/// Panics if the registry has not been initialized.
	pub async fn default_librarian(&self) -> Arc<ManagedLibrarian> {
		let libs = self.librarians.lock().await;
		libs.get("default")
			.cloned()
			.expect("Registry not initialized: default librarian not available")
	}

	// -----------------------------------------------------------------------
	// Resolve
	// -----------------------------------------------------------------------

	/// Resolve which librarian should manage content for a given topic.
	///
	/// Resolution strategy:
	/// 1. Find candidates whose topic globs match.
	/// 2. If 0 matches, use default.
	/// 3. If 1 match, use it directly.
	/// 4. If multiple matches, collect bids and check self-resolution gap.
	/// 5. If gap is insufficient, use LLM arbitration via the default provider.
	/// 6. Falls back to highest bidder.
	pub async fn resolve_librarian(
		&self,
		topic: &str,
		content: &str,
	) -> ArbitrationResult {
		let candidates: Vec<Arc<ManagedLibrarian>> = {
			let libs = self.librarians.lock().await;
			libs.values()
				.filter(|m| matches_topic(&m.definition.topics, topic))
				.cloned()
				.collect()
		};

		// 0 matches -> default
		if candidates.is_empty() {
			return ArbitrationResult {
				winner: "default".to_string(),
				reason: "No specialist matched the topic; using default librarian.".to_string(),
				bids: Vec::new(),
			};
		}

		// 1 match -> return directly
		if candidates.len() == 1 {
			return ArbitrationResult {
				winner: candidates[0].definition.name.clone(),
				reason: "Only one librarian matched the topic.".to_string(),
				bids: Vec::new(),
			};
		}

		// Multiple matches -> collect bids
		let mut bids: Vec<LibrarianBid> = Vec::new();
		for candidate in &candidates {
			let bid = candidate
				.librarian
				.bid(content, topic, self.library.as_ref())
				.await;
			bids.push(bid);
		}

		// Sort by confidence descending
		bids.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

		// Self-resolution: if gap between top two is large enough
		if bids.len() >= 2 {
			let gap = bids[0].confidence - bids[1].confidence;
			if gap > self.self_resolution_gap {
				return ArbitrationResult {
					winner: bids[0].librarian_name.clone(),
					reason: format!(
						"Self-resolved: confidence gap {:.2} exceeds threshold {}.",
						gap, self.self_resolution_gap
					),
					bids,
				};
			}
		}

		// Arbitration by default provider
		let candidate_names: Vec<String> = bids.iter().map(|b| b.librarian_name.clone()).collect();
		let bids_description: String = bids
			.iter()
			.map(|b| {
				format!(
					"- {} (confidence: {:.2}): {}",
					b.librarian_name, b.confidence, b.argument
				)
			})
			.collect::<Vec<_>>()
			.join("\n");

		let preview = if content.len() > 500 {
			&content[..500]
		} else {
			content
		};

		let prompt = format!(
			r#"You are arbitrating between librarians to decide who should manage new content.

Topic: {}
Content preview: {}

Bids:
{}

Choose the best librarian. Return ONLY valid JSON:
{{"winner": "librarian-name", "reason": "brief explanation"}}"#,
			topic, preview, bids_description
		);

		if let Ok(response) = self.default_provider.generate(&prompt, None).await {
			if let Ok(parsed) = serde_json::from_str::<ArbitrationJson>(&response) {
				if let Some(ref winner) = parsed.winner {
					if candidate_names.contains(winner) {
						return ArbitrationResult {
							winner: winner.clone(),
							reason: parsed
								.reason
								.unwrap_or_else(|| "Arbitration by default librarian.".to_string()),
							bids,
						};
					}
				}
			}
		}

		// Fallback: highest bidder wins
		ArbitrationResult {
			winner: bids[0].librarian_name.clone(),
			reason: "Highest bidder wins (arbitration fallback).".to_string(),
			bids,
		}
	}

	// -----------------------------------------------------------------------
	// Spawn specialist
	// -----------------------------------------------------------------------

	/// Ask the LLM whether a specialist should be created for a topic,
	/// generate its definition, and register it.
	pub async fn spawn_specialist(
		&self,
		topic: &str,
		volumes: &[super::librarian::Volume],
	) -> Result<Arc<ManagedLibrarian>, SimseError> {
		// Step 1: Assess whether a specialist is needed
		let volume_samples: String = volumes
			.iter()
			.take(5)
			.map(|v| {
				let preview = if v.text.len() > 100 {
					&v.text[..100]
				} else {
					&v.text
				};
				format!("- {}", preview)
			})
			.collect::<Vec<_>>()
			.join("\n");

		let assess_prompt = format!(
			r#"Should a specialist librarian be created for the topic "{}"?
There are currently {} volumes in this topic.

Volume samples:
{}

Return ONLY valid JSON: {{"shouldSpawn": true/false, "reason": "brief explanation"}}"#,
			topic,
			volumes.len(),
			volume_samples
		);

		let assess_response = self
			.default_provider
			.generate(&assess_prompt, None)
			.await?;

		let assess_parsed: SpawnAssessmentJson =
			serde_json::from_str(&assess_response).map_err(|_| {
				SimseError::library(
					crate::error::LibraryErrorCode::InvalidInput,
					format!(
						"Failed to parse spawn assessment for topic \"{}\": invalid JSON response",
						topic
					),
				)
			})?;

		if !assess_parsed.should_spawn.unwrap_or(false) {
			return Err(SimseError::library(
				crate::error::LibraryErrorCode::InvalidInput,
				format!(
					"Specialist not needed for topic \"{}\": {}",
					topic,
					assess_parsed
						.reason
						.unwrap_or_else(|| "provider declined".to_string())
				),
			));
		}

		// Step 2: Generate a LibrarianDefinition
		let existing_names: Vec<String> = {
			let libs = self.librarians.lock().await;
			libs.keys().cloned().collect()
		};

		let generate_prompt = format!(
			r#"Generate a librarian definition JSON for a specialist that will manage the topic "{}".

The librarian should:
- Have a descriptive kebab-case name related to the topic
- Cover the topic and its subtopics
- Have appropriate permissions
- NOT use any of these existing names: {}

Return ONLY valid JSON matching this schema:
{{
  "name": "kebab-case-name",
  "description": "what this librarian does",
  "purpose": "detailed purpose statement",
  "topics": ["{}", "{}/**"],
  "permissions": {{ "add": true, "delete": true, "reorganize": true }},
  "thresholds": {{ "topicComplexity": 50, "escalateAt": 100 }}
}}"#,
			topic,
			existing_names.join(", "),
			topic,
			topic
		);

		let gen_response = self
			.default_provider
			.generate(&generate_prompt, None)
			.await?;

		let gen_parsed: LibrarianDefinition =
			serde_json::from_str(&gen_response).map_err(|_| {
				SimseError::library(
					crate::error::LibraryErrorCode::InvalidInput,
					format!(
						"Failed to parse generated definition for topic \"{}\": invalid JSON response",
						topic
					),
				)
			})?;

		// Validate the generated definition
		let validation = validate_definition(&gen_parsed);
		if !validation.valid {
			return Err(SimseError::library(
				crate::error::LibraryErrorCode::InvalidInput,
				format!(
					"Generated definition is invalid: {}",
					validation.errors.join(", ")
				),
			));
		}

		// Step 3: Register
		self.register(gen_parsed).await
	}

	/// Get the librarians directory path.
	pub fn librarians_dir(&self) -> &Path {
		&self.librarians_dir
	}
}
