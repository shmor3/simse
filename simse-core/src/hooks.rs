//! Typed async hook system for intercepting and transforming pipeline operations.
//!
//! Ports `src/hooks/hook-system.ts` + `types.ts` (~281 lines) to Rust.
//!
//! Provides 6 hook types:
//! - **before**: Intercept tool execution, modify requests or block them
//! - **after**: Transform tool results after execution
//! - **validate**: Collect validation messages for tool results
//! - **prompt_transform**: Chain transformations on system prompts
//! - **messages_transform**: Chain transformations on conversation messages
//! - **compacting**: Chain transformations on compaction summaries
//!
//! Each hook type has typed `register_xxx()` / `run_xxx()` method pairs.
//! Handlers are async (`Pin<Box<dyn Future>>`) and stored thread-safely
//! via `Arc<Mutex<Vec<...>>>`.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use crate::conversation::ConversationMessage;
use crate::tools::types::{ToolCallRequest, ToolCallResult};

// ---------------------------------------------------------------------------
// Handler ID
// ---------------------------------------------------------------------------

type HandlerId = u64;

// ---------------------------------------------------------------------------
// BlockedResult
// ---------------------------------------------------------------------------

/// Returned by a before-hook to block tool execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockedResult {
	/// Human-readable reason why execution was blocked.
	pub reason: String,
}

// ---------------------------------------------------------------------------
// BeforeHookResult
// ---------------------------------------------------------------------------

/// Result of a before-hook: either continue (possibly with a modified request)
/// or block execution entirely.
#[derive(Debug, Clone)]
pub enum BeforeHookResult {
	/// Continue execution with the (possibly modified) request.
	Continue(ToolCallRequest),
	/// Block execution with a reason.
	Blocked(BlockedResult),
}

// ---------------------------------------------------------------------------
// Handler type aliases
// ---------------------------------------------------------------------------

/// Before-hook handler: receives a tool call request, returns continue or blocked.
pub type BeforeHandler = Arc<
	dyn Fn(ToolCallRequest) -> Pin<Box<dyn Future<Output = BeforeHookResult> + Send>>
		+ Send
		+ Sync,
>;

/// After-hook context passed to each handler.
#[derive(Debug, Clone)]
pub struct AfterHookContext {
	pub request: ToolCallRequest,
	pub result: ToolCallResult,
}

/// After-hook handler: receives original request + current result, returns modified result.
pub type AfterHandler = Arc<
	dyn Fn(AfterHookContext) -> Pin<Box<dyn Future<Output = ToolCallResult> + Send>>
		+ Send
		+ Sync,
>;

/// Validate-hook context passed to each handler.
#[derive(Debug, Clone)]
pub struct ValidateHookContext {
	pub request: ToolCallRequest,
	pub result: ToolCallResult,
}

/// Validate-hook handler: receives request + result, returns validation messages.
pub type ValidateHandler = Arc<
	dyn Fn(ValidateHookContext) -> Pin<Box<dyn Future<Output = Vec<String>> + Send>>
		+ Send
		+ Sync,
>;

/// Prompt-transform handler: receives current prompt string, returns transformed prompt.
pub type PromptTransformHandler =
	Arc<dyn Fn(String) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync>;

/// Messages-transform handler: receives current messages, returns transformed messages.
pub type MessagesTransformHandler = Arc<
	dyn Fn(Vec<ConversationMessage>) -> Pin<Box<dyn Future<Output = Vec<ConversationMessage>> + Send>>
		+ Send
		+ Sync,
>;

/// Compacting-hook context passed to each handler.
#[derive(Debug, Clone)]
pub struct CompactingHookContext {
	pub messages: Vec<ConversationMessage>,
	pub summary: String,
}

/// Compacting handler: receives original messages + current summary, returns new summary.
pub type CompactingHandler = Arc<
	dyn Fn(CompactingHookContext) -> Pin<Box<dyn Future<Output = String> + Send>> + Send + Sync,
>;

// ---------------------------------------------------------------------------
// Internal handler wrappers (ID + callback)
// ---------------------------------------------------------------------------

struct BeforeEntry {
	id: HandlerId,
	handler: BeforeHandler,
}

struct AfterEntry {
	id: HandlerId,
	handler: AfterHandler,
}

struct ValidateEntry {
	id: HandlerId,
	handler: ValidateHandler,
}

struct PromptTransformEntry {
	id: HandlerId,
	handler: PromptTransformHandler,
}

struct MessagesTransformEntry {
	id: HandlerId,
	handler: MessagesTransformHandler,
}

struct CompactingEntry {
	id: HandlerId,
	handler: CompactingHandler,
}

// ---------------------------------------------------------------------------
// HookSystemInner
// ---------------------------------------------------------------------------

struct HookSystemInner {
	next_id: HandlerId,
	before_handlers: Vec<BeforeEntry>,
	after_handlers: Vec<AfterEntry>,
	validate_handlers: Vec<ValidateEntry>,
	prompt_transform_handlers: Vec<PromptTransformEntry>,
	messages_transform_handlers: Vec<MessagesTransformEntry>,
	compacting_handlers: Vec<CompactingEntry>,
}

impl HookSystemInner {
	fn new() -> Self {
		Self {
			next_id: 0,
			before_handlers: Vec::new(),
			after_handlers: Vec::new(),
			validate_handlers: Vec::new(),
			prompt_transform_handlers: Vec::new(),
			messages_transform_handlers: Vec::new(),
			compacting_handlers: Vec::new(),
		}
	}

	fn next_handler_id(&mut self) -> HandlerId {
		let id = self.next_id;
		self.next_id = self.next_id.wrapping_add(1);
		id
	}
}

// ---------------------------------------------------------------------------
// HookSystem
// ---------------------------------------------------------------------------

/// Typed async hook system for intercepting and transforming pipeline operations.
///
/// Provides six hook types with different chaining semantics:
///
/// | Hook type           | Chaining behaviour                                        |
/// |---------------------|-----------------------------------------------------------|
/// | `before`            | Chain requests; early-exit on `Blocked`                    |
/// | `after`             | Chain results (each gets original request + prev result)   |
/// | `validate`          | Concatenate all validation message arrays                  |
/// | `prompt_transform`  | Chain prompt strings                                      |
/// | `messages_transform`| Chain message arrays                                      |
/// | `compacting`        | Chain summaries (each gets original messages + prev summary)|
#[derive(Clone)]
pub struct HookSystem {
	inner: Arc<Mutex<HookSystemInner>>,
}

impl HookSystem {
	/// Create a new, empty hook system.
	pub fn new() -> Self {
		Self {
			inner: Arc::new(Mutex::new(HookSystemInner::new())),
		}
	}

	// -- Before hooks ------------------------------------------------------

	/// Register a before-hook that intercepts tool execution requests.
	///
	/// Handlers are called in registration order. If any handler returns
	/// `BeforeHookResult::Blocked`, the chain stops immediately.
	///
	/// Returns an unsubscribe closure.
	pub fn register_before(&self, handler: BeforeHandler) -> impl Fn() + 'static {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let id = inner.next_handler_id();
		inner.before_handlers.push(BeforeEntry {
			id,
			handler,
		});

		let state = Arc::clone(&self.inner);
		move || {
			let mut inner = state.lock().unwrap_or_else(|e| e.into_inner());
			inner.before_handlers.retain(|e| e.id != id);
		}
	}

	/// Run all before-hooks, chaining the request through each.
	///
	/// If any handler returns `Blocked`, the chain stops and the blocked
	/// result is returned immediately. Otherwise the final (possibly modified)
	/// request is returned as `Continue`.
	pub async fn run_before(&self, request: ToolCallRequest) -> BeforeHookResult {
		let handlers: Vec<BeforeHandler> = {
			let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
			inner
				.before_handlers
				.iter()
				.map(|e| Arc::clone(&e.handler))
				.collect()
		};

		let mut current = request;
		for handler in &handlers {
			match handler(current).await {
				BeforeHookResult::Continue(req) => {
					current = req;
				}
				blocked @ BeforeHookResult::Blocked(_) => {
					return blocked;
				}
			}
		}
		BeforeHookResult::Continue(current)
	}

	// -- After hooks -------------------------------------------------------

	/// Register an after-hook that transforms tool results.
	///
	/// Each handler receives the original request and the current result.
	/// Returns an unsubscribe closure.
	pub fn register_after(&self, handler: AfterHandler) -> impl Fn() + 'static {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let id = inner.next_handler_id();
		inner.after_handlers.push(AfterEntry {
			id,
			handler,
		});

		let state = Arc::clone(&self.inner);
		move || {
			let mut inner = state.lock().unwrap_or_else(|e| e.into_inner());
			inner.after_handlers.retain(|e| e.id != id);
		}
	}

	/// Run all after-hooks, chaining the result through each.
	///
	/// Each handler receives the original (unchanged) request and the
	/// previous handler's result.
	pub async fn run_after(
		&self,
		request: ToolCallRequest,
		result: ToolCallResult,
	) -> ToolCallResult {
		let handlers: Vec<AfterHandler> = {
			let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
			inner
				.after_handlers
				.iter()
				.map(|e| Arc::clone(&e.handler))
				.collect()
		};

		let mut current = result;
		for handler in &handlers {
			current = handler(AfterHookContext {
				request: request.clone(),
				result: current,
			})
			.await;
		}
		current
	}

	// -- Validate hooks ----------------------------------------------------

	/// Register a validate-hook that checks tool results.
	///
	/// Returns an unsubscribe closure.
	pub fn register_validate(&self, handler: ValidateHandler) -> impl Fn() + 'static {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let id = inner.next_handler_id();
		inner.validate_handlers.push(ValidateEntry {
			id,
			handler,
		});

		let state = Arc::clone(&self.inner);
		move || {
			let mut inner = state.lock().unwrap_or_else(|e| e.into_inner());
			inner.validate_handlers.retain(|e| e.id != id);
		}
	}

	/// Run all validate-hooks, concatenating their validation messages.
	///
	/// All handlers run regardless of what others return. Messages from all
	/// handlers are concatenated into a single `Vec<String>`.
	pub async fn run_validate(
		&self,
		request: ToolCallRequest,
		result: ToolCallResult,
	) -> Vec<String> {
		let handlers: Vec<ValidateHandler> = {
			let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
			inner
				.validate_handlers
				.iter()
				.map(|e| Arc::clone(&e.handler))
				.collect()
		};

		let mut all_messages = Vec::new();
		for handler in &handlers {
			let messages = handler(ValidateHookContext {
				request: request.clone(),
				result: result.clone(),
			})
			.await;
			all_messages.extend(messages);
		}
		all_messages
	}

	// -- Prompt transform hooks --------------------------------------------

	/// Register a prompt-transform hook that modifies system prompts.
	///
	/// Returns an unsubscribe closure.
	pub fn register_prompt_transform(&self, handler: PromptTransformHandler) -> impl Fn() + 'static {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let id = inner.next_handler_id();
		inner
			.prompt_transform_handlers
			.push(PromptTransformEntry {
				id,
				handler,
			});

		let state = Arc::clone(&self.inner);
		move || {
			let mut inner = state.lock().unwrap_or_else(|e| e.into_inner());
			inner.prompt_transform_handlers.retain(|e| e.id != id);
		}
	}

	/// Run all prompt-transform hooks, chaining the prompt string.
	///
	/// Each handler receives the output of the previous handler.
	pub async fn run_prompt_transform(&self, prompt: String) -> String {
		let handlers: Vec<PromptTransformHandler> = {
			let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
			inner
				.prompt_transform_handlers
				.iter()
				.map(|e| Arc::clone(&e.handler))
				.collect()
		};

		let mut current = prompt;
		for handler in &handlers {
			current = handler(current).await;
		}
		current
	}

	// -- Messages transform hooks ------------------------------------------

	/// Register a messages-transform hook that modifies conversation messages.
	///
	/// Returns an unsubscribe closure.
	pub fn register_messages_transform(&self, handler: MessagesTransformHandler) -> impl Fn() + 'static {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let id = inner.next_handler_id();
		inner
			.messages_transform_handlers
			.push(MessagesTransformEntry {
				id,
				handler,
			});

		let state = Arc::clone(&self.inner);
		move || {
			let mut inner = state.lock().unwrap_or_else(|e| e.into_inner());
			inner.messages_transform_handlers.retain(|e| e.id != id);
		}
	}

	/// Run all messages-transform hooks, chaining the message array.
	///
	/// Each handler receives the output of the previous handler.
	pub async fn run_messages_transform(
		&self,
		messages: Vec<ConversationMessage>,
	) -> Vec<ConversationMessage> {
		let handlers: Vec<MessagesTransformHandler> = {
			let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
			inner
				.messages_transform_handlers
				.iter()
				.map(|e| Arc::clone(&e.handler))
				.collect()
		};

		let mut current = messages;
		for handler in &handlers {
			current = handler(current).await;
		}
		current
	}

	// -- Compacting hooks --------------------------------------------------

	/// Register a compacting hook that transforms compaction summaries.
	///
	/// Each handler receives the original messages and the current summary.
	/// Returns an unsubscribe closure.
	pub fn register_compacting(&self, handler: CompactingHandler) -> impl Fn() + 'static {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let id = inner.next_handler_id();
		inner.compacting_handlers.push(CompactingEntry {
			id,
			handler,
		});

		let state = Arc::clone(&self.inner);
		move || {
			let mut inner = state.lock().unwrap_or_else(|e| e.into_inner());
			inner.compacting_handlers.retain(|e| e.id != id);
		}
	}

	/// Run all compacting hooks, chaining the summary string.
	///
	/// Each handler receives the original (unchanged) messages and the
	/// previous handler's summary.
	pub async fn run_compacting(
		&self,
		messages: Vec<ConversationMessage>,
		summary: String,
	) -> String {
		let handlers: Vec<CompactingHandler> = {
			let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
			inner
				.compacting_handlers
				.iter()
				.map(|e| Arc::clone(&e.handler))
				.collect()
		};

		let mut current = summary;
		for handler in &handlers {
			current = handler(CompactingHookContext {
				messages: messages.clone(),
				summary: current,
			})
			.await;
		}
		current
	}

	// -- Utility -----------------------------------------------------------

	/// Remove all registered hooks.
	///
	/// Note: the internal handler ID counter is intentionally *not* reset,
	/// preventing stale unsubscribe closures from matching newly-registered
	/// handlers that would otherwise recycle the same IDs.
	pub fn clear(&self) {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		inner.before_handlers.clear();
		inner.after_handlers.clear();
		inner.validate_handlers.clear();
		inner.prompt_transform_handlers.clear();
		inner.messages_transform_handlers.clear();
		inner.compacting_handlers.clear();
	}
}

impl Default for HookSystem {
	fn default() -> Self {
		Self::new()
	}
}
