//! Typed, synchronous publish/subscribe event bus with handler error isolation.
//!
//! Ports `src/events/event-bus.ts` + `src/events/types.ts` (~236 lines) to Rust.
//!
//! - `EventBus` struct backed by `Arc<Mutex<...>>` for thread safety
//! - `subscribe()` returns a closure that removes the handler by ID
//! - `subscribe_all()` registers a wildcard handler for all event types
//! - `publish()` uses `std::panic::catch_unwind` to isolate handler panics
//! - `clear()` removes all handlers (both per-event and global)
//! - Event type constants in the `event_types` module

use serde_json::Value;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Type aliases (clippy type_complexity fix)
// ---------------------------------------------------------------------------

type HandlerFn = dyn Fn(&Value) + Send + Sync;
type GlobalHandlerFn = dyn Fn(&str, &Value) + Send + Sync;

// ---------------------------------------------------------------------------
// Handler identification
// ---------------------------------------------------------------------------

type HandlerId = u64;

struct Handler {
	id: HandlerId,
	callback: Arc<HandlerFn>,
}

struct GlobalHandler {
	id: HandlerId,
	callback: Arc<GlobalHandlerFn>,
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct EventBusInner {
	next_id: HandlerId,
	handlers: HashMap<String, Vec<Handler>>,
	global_handlers: Vec<GlobalHandler>,
}

impl EventBusInner {
	fn new() -> Self {
		Self {
			next_id: 0,
			handlers: HashMap::new(),
			global_handlers: Vec::new(),
		}
	}

	fn next_handler_id(&mut self) -> HandlerId {
		let id = self.next_id;
		self.next_id = self.next_id.wrapping_add(1);
		id
	}
}

// ---------------------------------------------------------------------------
// EventBus
// ---------------------------------------------------------------------------

/// A typed, synchronous publish/subscribe event bus.
///
/// - `publish` delivers a payload to every subscriber of the given event type.
/// - `subscribe` registers a handler and returns an unsubscribe closure.
/// - `subscribe_all` registers a wildcard handler that receives every event.
/// - Handler panics are caught and isolated — one bad handler never kills others.
#[derive(Clone)]
pub struct EventBus {
	inner: Arc<Mutex<EventBusInner>>,
}

impl EventBus {
	/// Create a new empty event bus.
	pub fn new() -> Self {
		Self {
			inner: Arc::new(Mutex::new(EventBusInner::new())),
		}
	}

	/// Register a handler for a specific event type.
	///
	/// Returns an unsubscribe closure. Calling it removes the handler.
	/// The unsubscribe closure is idempotent.
	pub fn subscribe<F>(&self, event_type: &str, handler: F) -> impl Fn()
	where
		F: Fn(&Value) + Send + Sync + 'static,
	{
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let id = inner.next_handler_id();
		let event_key = event_type.to_string();

		inner
			.handlers
			.entry(event_key.clone())
			.or_default()
			.push(Handler {
				id,
				callback: Arc::new(handler),
			});

		let bus_inner = Arc::clone(&self.inner);
		move || {
			let mut inner = bus_inner.lock().unwrap_or_else(|e| e.into_inner());
			if let Some(handlers) = inner.handlers.get_mut(&event_key) {
				handlers.retain(|h| h.id != id);
			}
		}
	}

	/// Register a wildcard handler that fires for every event type.
	///
	/// The handler receives `(event_type, payload)`.
	/// Returns an unsubscribe closure. The unsubscribe closure is idempotent.
	pub fn subscribe_all<F>(&self, handler: F) -> impl Fn()
	where
		F: Fn(&str, &Value) + Send + Sync + 'static,
	{
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let id = inner.next_handler_id();

		inner.global_handlers.push(GlobalHandler {
			id,
			callback: Arc::new(handler),
		});

		let bus_inner = Arc::clone(&self.inner);
		move || {
			let mut inner = bus_inner.lock().unwrap_or_else(|e| e.into_inner());
			inner.global_handlers.retain(|h| h.id != id);
		}
	}

	/// Publish an event to all registered handlers.
	///
	/// Per-event handlers fire first (in registration order), then global
	/// handlers. Panics in individual handlers are caught and logged via
	/// `tracing::error` — they never propagate to other handlers or the caller.
	///
	/// The mutex lock is released before invoking handlers, so handlers may
	/// safely call `subscribe()`, `publish()`, `clear()`, etc. on the same
	/// `EventBus` without deadlocking.
	pub fn publish(&self, event_type: &str, payload: Value) {
		let (event_handlers, global_handlers) = {
			let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
			let event_handlers: Vec<Arc<HandlerFn>> = inner
				.handlers
				.get(event_type)
				.map(|v| v.iter().map(|h| Arc::clone(&h.callback)).collect())
				.unwrap_or_default();
			let global_handlers: Vec<Arc<GlobalHandlerFn>> = inner
				.global_handlers
				.iter()
				.map(|h| Arc::clone(&h.callback))
				.collect();
			(event_handlers, global_handlers)
		}; // lock released here

		// Fire per-event handlers
		for cb in &event_handlers {
			let cb = Arc::clone(cb);
			let p = AssertUnwindSafe(&payload);
			if let Err(_panic) = catch_unwind(AssertUnwindSafe(move || {
				cb(&p);
			})) {
				tracing::error!(
					event_type = %event_type,
					"[EventBus] handler panicked"
				);
			}
		}

		// Fire global handlers
		for cb in &global_handlers {
			let cb = Arc::clone(cb);
			let et = event_type.to_string();
			let p = AssertUnwindSafe(&payload);
			if let Err(_panic) = catch_unwind(AssertUnwindSafe(move || {
				cb(&et, &p);
			})) {
				tracing::error!(
					event_type = %event_type,
					"[EventBus] global handler panicked"
				);
			}
		}
	}

	/// Remove all handlers (both per-event and global).
	pub fn clear(&self) {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		inner.handlers.clear();
		inner.global_handlers.clear();
	}
}

impl Default for EventBus {
	fn default() -> Self {
		Self::new()
	}
}

// ---------------------------------------------------------------------------
// Event type constants
// ---------------------------------------------------------------------------

/// Well-known event type string constants.
///
/// These match the TypeScript `EventPayloadMap` keys. Using `&str` constants
/// (rather than an enum) keeps the system extensible — custom events can use
/// any string without modifying this module.
pub mod event_types {
	// Stream events
	pub const STREAM_START: &str = "stream.start";
	pub const STREAM_TOKEN: &str = "stream.token";
	pub const STREAM_ERROR: &str = "stream.error";
	pub const STREAM_RETRY: &str = "stream.retry";
	pub const STREAM_END: &str = "stream.end";

	// Loop events
	pub const LOOP_START: &str = "loop.start";
	pub const LOOP_COMPLETE: &str = "loop.complete";
	pub const LOOP_TURN_START: &str = "loop.turn_start";
	pub const LOOP_TURN_END: &str = "loop.turn_end";
	pub const LOOP_TOOL_START: &str = "loop.tool_start";
	pub const LOOP_TOOL_END: &str = "loop.tool_end";
	pub const LOOP_DOOM_LOOP: &str = "loop.doom_loop";
	pub const LOOP_COMPACTION: &str = "loop.compaction";

	// Library events
	pub const LIBRARY_SEARCH: &str = "library.search";
	pub const LIBRARY_STORE: &str = "library.store";
	pub const LIBRARY_DELETE: &str = "library.delete";
	pub const LIBRARY_RECOMMEND: &str = "library.recommend";
	pub const LIBRARY_COMPENDIUM: &str = "library.compendium";
	pub const LIBRARY_CLASSIFY: &str = "library.classify";
	pub const LIBRARY_DEDUPLICATE: &str = "library.deduplicate";
	pub const LIBRARY_ORGANIZE: &str = "library.organize";
	pub const LIBRARY_EXPORT: &str = "library.export";

	// Chain events
	pub const CHAIN_START: &str = "chain.start";
	pub const CHAIN_STEP: &str = "chain.step";
	pub const CHAIN_END: &str = "chain.end";
	pub const CHAIN_ERROR: &str = "chain.error";

	// Task events
	pub const TASK_CREATE: &str = "task.create";
	pub const TASK_UPDATE: &str = "task.update";
	pub const TASK_DELETE: &str = "task.delete";

	// Conversation events
	pub const CONVERSATION_ADD: &str = "conversation.add";
	pub const CONVERSATION_COMPACT: &str = "conversation.compact";
}
