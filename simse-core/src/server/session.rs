//! Session manager for multi-session orchestration.
//!
//! Ports `src/server/session-manager.ts` (~119 lines) to Rust.
//!
//! Each session owns a `Conversation` and an `EventBus`. The `SessionManager`
//! provides create, get, delete, list, status update, and fork operations.
//! Fork clones the conversation via JSON serialization (matching TS behavior)
//! and creates a fresh event bus for the new session.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::conversation::Conversation;
use crate::events::EventBus;

// ---------------------------------------------------------------------------
// SessionStatus
// ---------------------------------------------------------------------------

/// Status of a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
	Active,
	Completed,
	Aborted,
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// A single session containing a conversation and event bus.
pub struct Session {
	pub id: String,
	pub conversation: Conversation,
	pub event_bus: EventBus,
	pub status: SessionStatus,
	pub created_at: u64,
	pub updated_at: u64,
}

// ---------------------------------------------------------------------------
// SessionInfo
// ---------------------------------------------------------------------------

/// Lightweight metadata snapshot of a session (no heavy fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
	pub id: String,
	pub status: SessionStatus,
	pub created_at: u64,
	pub updated_at: u64,
	pub message_count: usize,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the current Unix timestamp in milliseconds.
fn now_millis() -> u64 {
	SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64
}

// ---------------------------------------------------------------------------
// SessionManagerInner
// ---------------------------------------------------------------------------

struct SessionManagerInner {
	sessions: HashMap<String, Session>,
	id_counter: u64,
}

impl SessionManagerInner {
	fn new() -> Self {
		Self {
			sessions: HashMap::new(),
			id_counter: 0,
		}
	}

	/// Generate a session ID in the format `sess_{timestamp}_{counter_base36}`.
	fn generate_id(&mut self) -> String {
		let ts = now_millis();
		let count = self.id_counter;
		self.id_counter = self.id_counter.wrapping_add(1);
		format!("sess_{ts}_{}", radix36(count))
	}
}

/// Convert a `u64` to a base-36 string (digits + lowercase a-z).
fn radix36(mut n: u64) -> String {
	if n == 0 {
		return "0".to_string();
	}
	const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
	let mut buf = Vec::new();
	while n > 0 {
		buf.push(DIGITS[(n % 36) as usize]);
		n /= 36;
	}
	buf.reverse();
	String::from_utf8(buf).expect("base36 chars are always valid UTF-8")
}

// ---------------------------------------------------------------------------
// SessionManager
// ---------------------------------------------------------------------------

/// Thread-safe manager for multiple sessions.
///
/// Stores sessions in an `Arc<Mutex<...>>` map keyed by session ID.
/// Provides CRUD operations plus fork (clone conversation into a new session).
#[derive(Clone)]
pub struct SessionManager {
	inner: Arc<Mutex<SessionManagerInner>>,
}

impl SessionManager {
	/// Create a new empty session manager.
	pub fn new() -> Self {
		Self {
			inner: Arc::new(Mutex::new(SessionManagerInner::new())),
		}
	}

	/// Create a new session with an empty conversation and event bus.
	///
	/// Returns the generated session ID.
	pub fn create(&self) -> String {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let id = inner.generate_id();
		let now = now_millis();
		let session = Session {
			id: id.clone(),
			conversation: Conversation::new(None),
			event_bus: EventBus::new(),
			status: SessionStatus::Active,
			created_at: now,
			updated_at: now,
		};
		inner.sessions.insert(id.clone(), session);
		id
	}

	/// Get lightweight metadata for a session.
	pub fn get_info(&self, id: &str) -> Option<SessionInfo> {
		let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		inner.sessions.get(id).map(|s| SessionInfo {
			id: s.id.clone(),
			status: s.status.clone(),
			created_at: s.created_at,
			updated_at: s.updated_at,
			message_count: s.conversation.message_count(),
		})
	}

	/// Execute a closure with a mutable reference to a session.
	///
	/// Returns `None` if the session does not exist.
	pub fn with_session<F, R>(&self, id: &str, f: F) -> Option<R>
	where
		F: FnOnce(&mut Session) -> R,
	{
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		inner.sessions.get_mut(id).map(f)
	}

	/// Delete a session by ID.
	///
	/// Returns `true` if the session existed and was removed.
	pub fn delete(&self, id: &str) -> bool {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		inner.sessions.remove(id).is_some()
	}

	/// List metadata for all sessions.
	pub fn list(&self) -> Vec<SessionInfo> {
		let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		inner
			.sessions
			.values()
			.map(|s| SessionInfo {
				id: s.id.clone(),
				status: s.status.clone(),
				created_at: s.created_at,
				updated_at: s.updated_at,
				message_count: s.conversation.message_count(),
			})
			.collect()
	}

	/// Update the status of a session.
	///
	/// Returns `true` if the session was found and updated.
	pub fn update_status(&self, id: &str, status: SessionStatus) -> bool {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		match inner.sessions.get_mut(id) {
			Some(session) => {
				session.status = status;
				session.updated_at = now_millis();
				true
			}
			None => false,
		}
	}

	/// Fork a session by cloning its conversation into a new session.
	///
	/// The new session gets a fresh event bus and an `Active` status.
	/// Conversation state is cloned via JSON serialization (matching the
	/// TypeScript `toJSON`/`fromJSON` behaviour).
	///
	/// Returns the new session ID, or `None` if the source session does not exist.
	pub fn fork(&self, id: &str) -> Option<String> {
		let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
		let json = inner.sessions.get(id).map(|s| s.conversation.to_json())?;

		let new_id = inner.generate_id();
		let now = now_millis();

		let mut new_conversation = Conversation::new(None);
		new_conversation.from_json(&json);

		let forked = Session {
			id: new_id.clone(),
			conversation: new_conversation,
			event_bus: EventBus::new(),
			status: SessionStatus::Active,
			created_at: now,
			updated_at: now,
		};
		inner.sessions.insert(new_id.clone(), forked);
		Some(new_id)
	}
}

impl Default for SessionManager {
	fn default() -> Self {
		Self::new()
	}
}
