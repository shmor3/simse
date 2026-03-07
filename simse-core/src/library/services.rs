//! Library Services — middleware hooking the library into the agentic loop.
//!
//! Ports `src/ai/library/library-services.ts` (~150 lines) to Rust.
//!
//! - `LibraryServices` — enriches system prompts with relevant context from
//!   the library and stores responses back after each turn.
//! - `enrich_system_prompt()` — searches library, formats memory block, prepends to system prompt.
//! - `after_response()` — enqueues extraction via CirculationDesk or directly adds Q&A pair.

use std::collections::HashMap;
use std::sync::Arc;

use crate::adaptive::context_format::{format_context, ContextFormatOptions};

use crate::error::SimseError;
use crate::library::Library;

use super::circulation::CirculationDesk;
use super::librarian::TurnContext;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Context for enriching a system prompt.
#[derive(Debug, Clone)]
pub struct LibraryContext {
	pub user_input: String,
	pub current_system_prompt: String,
	pub conversation_history: String,
	pub turn: usize,
}

/// Options for creating `LibraryServices`.
pub struct LibraryServicesOptions {
	/// Maximum results to retrieve per turn. Defaults to 5.
	pub max_results: usize,
	/// Minimum relevance score for inclusion.
	pub min_score: Option<f64>,
	/// Output format: "structured" (XML tags) or "natural". Defaults to "structured".
	pub format: Option<String>,
	/// XML tag name for the outer wrapper. Defaults to "memory-context".
	pub tag: Option<String>,
	/// Maximum total characters in the output. Defaults to 4000.
	pub max_chars: Option<usize>,
	/// Topic to tag stored Q&A pairs with. Defaults to "conversation".
	pub store_topic: String,
	/// Whether to store Q&A pairs in library. Defaults to true.
	pub store_responses: bool,
	/// Optional CirculationDesk for async background extraction.
	pub circulation_desk: Option<Arc<CirculationDesk>>,
}

impl Default for LibraryServicesOptions {
	fn default() -> Self {
		Self {
			max_results: 5,
			min_score: None,
			format: None,
			tag: None,
			max_chars: None,
			store_topic: "conversation".to_string(),
			store_responses: true,
			circulation_desk: None,
		}
	}
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if a response looks like an error message.
fn is_error_response(response: &str) -> bool {
	let lower = response.to_lowercase();
	lower.starts_with("error")
		|| lower.contains("error communicating")
		|| lower.contains("failed to")
}

// ---------------------------------------------------------------------------
// LibraryServices
// ---------------------------------------------------------------------------

/// Middleware that enriches agentic loop turns with library context
/// and stores responses back into the library.
pub struct LibraryServices {
	library: Arc<Library>,
	max_results: usize,
	min_score: Option<f64>,
	format: Option<String>,
	tag: Option<String>,
	max_chars: Option<usize>,
	store_topic: String,
	store_responses: bool,
	circulation_desk: Option<Arc<CirculationDesk>>,
}

impl LibraryServices {
	/// Create new library services wrapping the given library.
	pub fn new(library: Arc<Library>, options: Option<LibraryServicesOptions>) -> Self {
		let opts = options.unwrap_or_default();
		Self {
			library,
			max_results: opts.max_results,
			min_score: opts.min_score,
			format: opts.format,
			tag: opts.tag,
			max_chars: opts.max_chars,
			store_topic: opts.store_topic,
			store_responses: opts.store_responses,
			circulation_desk: opts.circulation_desk,
		}
	}

	/// Search the library for relevant context and prepend it to the system prompt.
	///
	/// Returns the original system prompt unchanged if:
	/// - The library is not initialized
	/// - The library is empty
	/// - No relevant results are found
	/// - The search fails
	pub async fn enrich_system_prompt(&self, context: &LibraryContext) -> String {
		if !self.library.is_initialized() || self.library.size() == 0 {
			return context.current_system_prompt.clone();
		}

		let results = match self
			.library
			.search(
				&context.user_input,
				Some(self.max_results),
				self.min_score,
			)
			.await
		{
			Ok(r) => r,
			Err(_) => return context.current_system_prompt.clone(),
		};

		if results.is_empty() {
			return context.current_system_prompt.clone();
		}

		let format_opts = ContextFormatOptions {
			max_results: Some(self.max_results),
			min_score: self.min_score,
			format: self.format.clone(),
			tag: self.tag.clone(),
			max_chars: self.max_chars,
		};

		let now = std::time::SystemTime::now()
			.duration_since(std::time::UNIX_EPOCH)
			.map(|d| d.as_millis() as u64)
			.unwrap_or(0);

		let memory_block = format_context(&results, &format_opts, now);

		if memory_block.is_empty() {
			return context.current_system_prompt.clone();
		}

		format!(
			"{}\n\n{}",
			context.current_system_prompt, memory_block
		)
	}

	/// Process a response after it has been generated.
	///
	/// If a CirculationDesk is configured, enqueues an extraction job.
	/// Otherwise, directly adds the Q&A pair to the library.
	///
	/// Skips:
	/// - Empty or whitespace-only responses
	/// - Error-looking responses
	/// - If store_responses is false
	/// - If the library is not initialized
	pub async fn after_response(
		&self,
		user_input: &str,
		response: &str,
	) -> Result<(), SimseError> {
		if !self.store_responses {
			return Ok(());
		}
		if response.trim().is_empty() {
			return Ok(());
		}
		if is_error_response(response) {
			return Ok(());
		}
		if !self.library.is_initialized() {
			return Ok(());
		}

		if let Some(ref desk) = self.circulation_desk {
			desk.enqueue_extraction(TurnContext {
				user_input: user_input.to_string(),
				response: response.to_string(),
			});
		} else {
			let text = format!("Q: {}\nA: {}", user_input, response);
			let mut metadata = HashMap::new();
			metadata.insert("topic".to_string(), self.store_topic.clone());
			self.library.add(&text, metadata).await?;
		}

		Ok(())
	}
}
