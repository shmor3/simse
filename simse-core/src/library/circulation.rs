//! Circulation Desk — async job queue for background library maintenance.
//!
//! Ports `src/ai/library/circulation-desk.ts` (~272 lines) to Rust.
//!
//! - Job types: extraction, compendium, reorganization, optimization
//! - `enqueue_*` methods push jobs onto a `tokio::sync::mpsc` channel
//! - `drain()` receives and processes jobs sequentially
//! - Escalation checking (topic/global volume thresholds trigger optimization)
//! - Failed jobs are swallowed (fire-and-forget)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use crate::error::SimseError;

use super::librarian::{Librarian, TurnContext, Volume};
use super::librarian_reg::LibrarianRegistry;

// ---------------------------------------------------------------------------
// Topic catalog trait
// ---------------------------------------------------------------------------

/// Trait for topic catalog operations used during circulation.
pub trait TopicCatalog: Send + Sync {
	fn resolve(&self, proposed_topic: &str) -> String;
	fn relocate(&self, volume_id: &str, new_topic: &str);
	fn merge(&self, source_topic: &str, target_topic: &str);
}

// ---------------------------------------------------------------------------
// Thresholds
// ---------------------------------------------------------------------------

/// Thresholds controlling when different maintenance jobs are triggered.
#[derive(Debug, Clone)]
pub struct CirculationThresholds {
	/// Minimum entries in a topic before compendium is triggered.
	pub compendium_min_entries: usize,
	/// Max volumes per topic before reorganization is triggered.
	pub reorganization_max_volumes_per_topic: usize,
	/// Topic volume threshold for optimization escalation.
	pub optimization_topic_threshold: usize,
	/// Global volume threshold for optimization escalation.
	pub optimization_global_threshold: usize,
	/// Model ID for optimization (required if optimization is enabled).
	pub optimization_model_id: Option<String>,
	/// Complexity threshold for specialist spawning.
	pub spawning_complexity_threshold: Option<usize>,
}

impl Default for CirculationThresholds {
	fn default() -> Self {
		Self {
			compendium_min_entries: 10,
			reorganization_max_volumes_per_topic: 30,
			optimization_topic_threshold: 50,
			optimization_global_threshold: 500,
			optimization_model_id: None,
			spawning_complexity_threshold: None,
		}
	}
}

// ---------------------------------------------------------------------------
// Library operations trait
// ---------------------------------------------------------------------------

/// Operations the circulation desk needs from the library.
#[async_trait]
pub trait CirculationLibraryOps: Send + Sync {
	async fn add_volume(
		&self,
		text: &str,
		metadata: std::collections::HashMap<String, String>,
	) -> Result<String, SimseError>;

	async fn check_duplicate(&self, text: &str) -> Result<DuplicateCheckResult, SimseError>;

	async fn get_volumes_for_topic(&self, topic: &str) -> Result<Vec<Volume>, SimseError>;

	async fn delete_volume(&self, id: &str) -> Result<(), SimseError>;

	async fn get_total_volume_count(&self) -> Result<usize, SimseError>;

	async fn get_all_topics(&self) -> Result<Vec<String>, SimseError>;
}

/// Minimal duplicate check result.
#[derive(Debug, Clone)]
pub struct DuplicateCheckResult {
	pub is_duplicate: bool,
}

// ---------------------------------------------------------------------------
// Job types
// ---------------------------------------------------------------------------

/// A background job for the circulation desk.
#[derive(Debug, Clone)]
pub enum Job {
	Extraction(TurnContext),
	Compendium { topic: String },
	Reorganization { topic: String },
	Optimization { topic: String },
}

// ---------------------------------------------------------------------------
// CirculationDesk
// ---------------------------------------------------------------------------

/// Async background queue for library maintenance operations.
///
/// Jobs are enqueued via `enqueue_*` methods and processed sequentially
/// by calling `drain()`. Failed jobs are silently dropped.
pub struct CirculationDesk {
	sender: mpsc::UnboundedSender<Job>,
	receiver: Mutex<mpsc::UnboundedReceiver<Job>>,
	librarian: Option<Arc<Librarian>>,
	registry: Option<Arc<LibrarianRegistry>>,
	library_ops: Arc<dyn CirculationLibraryOps>,
	thresholds: CirculationThresholds,
	catalog: Option<Arc<dyn TopicCatalog>>,
	processing: AtomicBool,
	disposed: AtomicBool,
	pending_count: std::sync::atomic::AtomicUsize,
}

/// Options for creating a `CirculationDesk`.
pub struct CirculationDeskOptions {
	pub librarian: Option<Arc<Librarian>>,
	pub registry: Option<Arc<LibrarianRegistry>>,
	pub library_ops: Arc<dyn CirculationLibraryOps>,
	pub thresholds: CirculationThresholds,
	pub catalog: Option<Arc<dyn TopicCatalog>>,
}

impl CirculationDesk {
	/// Create a new circulation desk.
	///
	/// Requires either a `librarian` or a `registry` (or both).
	pub fn new(options: CirculationDeskOptions) -> Result<Self, SimseError> {
		if options.librarian.is_none() && options.registry.is_none() {
			return Err(SimseError::library(
				crate::error::LibraryErrorCode::InvalidInput,
				"CirculationDesk requires either librarian or registry",
			));
		}

		let (sender, receiver) = mpsc::unbounded_channel();

		Ok(Self {
			sender,
			receiver: Mutex::new(receiver),
			librarian: options.librarian,
			registry: options.registry,
			library_ops: options.library_ops,
			thresholds: options.thresholds,
			catalog: options.catalog,
			processing: AtomicBool::new(false),
			disposed: AtomicBool::new(false),
			pending_count: std::sync::atomic::AtomicUsize::new(0),
		})
	}

	// -----------------------------------------------------------------------
	// Default librarian accessor
	// -----------------------------------------------------------------------

	fn default_librarian(&self) -> Option<&Librarian> {
		self.librarian.as_deref()
	}

	// -----------------------------------------------------------------------
	// Enqueue methods
	// -----------------------------------------------------------------------

	/// Enqueue a memory extraction job.
	pub fn enqueue_extraction(&self, turn: TurnContext) {
		if self.disposed.load(Ordering::Relaxed) {
			return;
		}
		let _ = self.sender.send(Job::Extraction(turn));
		self.pending_count.fetch_add(1, Ordering::Relaxed);
	}

	/// Enqueue a compendium (summarization) job.
	pub fn enqueue_compendium(&self, topic: String) {
		if self.disposed.load(Ordering::Relaxed) {
			return;
		}
		let _ = self.sender.send(Job::Compendium { topic });
		self.pending_count.fetch_add(1, Ordering::Relaxed);
	}

	/// Enqueue a reorganization job.
	pub fn enqueue_reorganization(&self, topic: String) {
		if self.disposed.load(Ordering::Relaxed) {
			return;
		}
		let _ = self.sender.send(Job::Reorganization { topic });
		self.pending_count.fetch_add(1, Ordering::Relaxed);
	}

	/// Enqueue an optimization job.
	pub fn enqueue_optimization(&self, topic: String) {
		if self.disposed.load(Ordering::Relaxed) {
			return;
		}
		let _ = self.sender.send(Job::Optimization { topic });
		self.pending_count.fetch_add(1, Ordering::Relaxed);
	}

	// -----------------------------------------------------------------------
	// Drain
	// -----------------------------------------------------------------------

	/// Process all pending jobs sequentially.
	///
	/// Returns immediately if already processing or disposed.
	/// Failed jobs are silently dropped.
	pub async fn drain(&self) {
		if self.processing.load(Ordering::Relaxed) || self.disposed.load(Ordering::Relaxed) {
			return;
		}
		self.processing.store(true, Ordering::Relaxed);

		let mut receiver = self.receiver.lock().await;
		while let Ok(job) = receiver.try_recv() {
			self.pending_count.fetch_sub(1, Ordering::Relaxed);
			let _ = self.process_job(job).await;
		}

		self.processing.store(false, Ordering::Relaxed);
	}

	// -----------------------------------------------------------------------
	// Flush & Dispose
	// -----------------------------------------------------------------------

	/// Discard all pending jobs without processing.
	pub async fn flush(&self) {
		let mut receiver = self.receiver.lock().await;
		while receiver.try_recv().is_ok() {
			self.pending_count.fetch_sub(1, Ordering::Relaxed);
		}
	}

	/// Dispose the circulation desk: mark as disposed and discard jobs.
	pub async fn dispose(&self) {
		self.disposed.store(true, Ordering::Relaxed);
		self.flush().await;
	}

	// -----------------------------------------------------------------------
	// Status
	// -----------------------------------------------------------------------

	/// Number of pending (unprocessed) jobs.
	pub fn pending(&self) -> usize {
		self.pending_count.load(Ordering::Relaxed)
	}

	/// Whether the desk is currently processing jobs.
	pub fn is_processing(&self) -> bool {
		self.processing.load(Ordering::Relaxed)
	}

	// -----------------------------------------------------------------------
	// Internal: resolve librarian
	// -----------------------------------------------------------------------

	async fn resolve_librarian_for_job(
		&self,
		topic: &str,
		content: &str,
	) -> (Arc<Librarian>, String) {
		// When a registry is available, resolve through it (bidding/arbitration).
		if let Some(ref registry) = self.registry {
			let result = registry.resolve_librarian(topic, content).await;
			if let Some(managed) = registry.get(&result.winner).await {
				return (Arc::clone(&managed.librarian), result.winner);
			}
		}
		// Fall back to the explicit librarian.
		if let Some(ref lib) = self.librarian {
			return (Arc::clone(lib), "default".to_string());
		}
		// Constructor guarantees at least one is set; this is unreachable.
		unreachable!("CirculationDesk requires either librarian or registry")
	}

	// -----------------------------------------------------------------------
	// Internal: escalation checking
	// -----------------------------------------------------------------------

	async fn check_escalation(&self, topic: &str) -> Vec<Job> {
		let mut new_jobs = Vec::new();

		if self.thresholds.optimization_model_id.is_none() {
			return new_jobs;
		}

		if let Ok(volumes) = self.library_ops.get_volumes_for_topic(topic).await {
			if volumes.len() >= self.thresholds.optimization_topic_threshold {
				new_jobs.push(Job::Optimization {
					topic: topic.to_string(),
				});
				return new_jobs;
			}
		}

		if let Ok(total) = self.library_ops.get_total_volume_count().await {
			if total >= self.thresholds.optimization_global_threshold {
				if let Ok(topics) = self.library_ops.get_all_topics().await {
					for t in topics {
						new_jobs.push(Job::Optimization { topic: t });
					}
				}
			}
		}

		new_jobs
	}

	// -----------------------------------------------------------------------
	// Internal: spawning check
	// -----------------------------------------------------------------------

	async fn check_spawning(&self, topic: &str) {
		if let (Some(ref registry), Some(threshold)) =
			(&self.registry, self.thresholds.spawning_complexity_threshold)
		{
			if let Ok(volumes) = self.library_ops.get_volumes_for_topic(topic).await {
				if volumes.len() >= threshold {
					let _ = registry.spawn_specialist(topic, &volumes).await;
				}
			}
		}
	}

	// -----------------------------------------------------------------------
	// Internal: process a single job
	// -----------------------------------------------------------------------

	async fn process_job(&self, job: Job) -> Result<(), SimseError> {
		match job {
			Job::Extraction(turn) => {
				let librarian = match self.default_librarian() {
					Some(l) => l,
					None => return Ok(()),
				};

				let result = librarian.extract(&turn).await;
				let mut extracted_topics = std::collections::HashSet::new();

				for mem in result.memories {
					let dup = self.library_ops.check_duplicate(&mem.text).await?;
					if dup.is_duplicate {
						continue;
					}

					let topic = if let Some(ref catalog) = self.catalog {
						catalog.resolve(&mem.topic)
					} else {
						mem.topic.clone()
					};

					let mut metadata = std::collections::HashMap::new();
					metadata.insert("topic".to_string(), topic.clone());
					metadata.insert("tags".to_string(), mem.tags.join(","));
					metadata.insert("entryType".to_string(), mem.entry_type.to_string());

					self.library_ops.add_volume(&mem.text, metadata).await?;
					extracted_topics.insert(topic);
				}

				// Check escalation and spawning for each extracted topic
				let mut escalation_jobs = Vec::new();
				for topic in &extracted_topics {
					let new_jobs = self.check_escalation(topic).await;
					escalation_jobs.extend(new_jobs);
					self.check_spawning(topic).await;
				}

				// Enqueue escalation jobs
				for job in escalation_jobs {
					if let Job::Optimization { topic } = job {
						self.enqueue_optimization(topic);
					}
				}
			}

			Job::Compendium { topic } => {
				let (librarian, _name) = self.resolve_librarian_for_job(&topic, "").await;

				let volumes = self.library_ops.get_volumes_for_topic(&topic).await?;
				if volumes.len() >= self.thresholds.compendium_min_entries {
					let _ = librarian.summarize(&volumes, &topic).await;
				}
			}

			Job::Reorganization { topic } => {
				let (librarian, _name) = self.resolve_librarian_for_job(&topic, "").await;

				let volumes = self.library_ops.get_volumes_for_topic(&topic).await?;
				if volumes.len() >= self.thresholds.reorganization_max_volumes_per_topic {
					let plan = librarian.reorganize(&topic, &volumes).await;
					if let Some(ref catalog) = self.catalog {
						for mv in &plan.moves {
							catalog.relocate(&mv.volume_id, &mv.new_topic);
						}
						for merge in &plan.merges {
							catalog.merge(&merge.source, &merge.target);
						}
					}
				}
			}

			Job::Optimization { topic } => {
				let model_id = match &self.thresholds.optimization_model_id {
					Some(id) => id.clone(),
					None => return Ok(()),
				};

				let (librarian, _name) = self.resolve_librarian_for_job(&topic, "").await;

				let volumes = self.library_ops.get_volumes_for_topic(&topic).await?;
				if volumes.is_empty() {
					return Ok(());
				}

				let result = librarian.optimize(&volumes, &topic, &model_id).await;

				for id in &result.pruned {
					let _ = self.library_ops.delete_volume(id).await;
				}

				if !result.summary.is_empty() {
					let mut metadata = std::collections::HashMap::new();
					metadata.insert("topic".to_string(), topic.clone());
					metadata.insert("entryType".to_string(), "compendium".to_string());
					let _ = self.library_ops.add_volume(&result.summary, metadata).await;
				}

				if let Some(ref catalog) = self.catalog {
					for mv in &result.reorganization.moves {
						catalog.relocate(&mv.volume_id, &mv.new_topic);
					}
					for merge in &result.reorganization.merges {
						catalog.merge(&merge.source, &merge.target);
					}
				}
			}
		}

		Ok(())
	}
}
