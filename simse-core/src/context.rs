//! Top-level context holding shared infrastructure for a simse application.
//!
//! `CoreContext` is the wiring struct that ties together the event bus, logger,
//! config, task list, hook system, session manager, and optionally the library
//! and VFS. Library and VFS are `Option` because they require engine binaries
//! (Rust subprocesses) that cannot be spawned during simple construction.

use std::sync::Arc;

use crate::config::AppConfig;
use crate::events::EventBus;
use crate::hooks::HookSystem;
use crate::library::Library;
use crate::logger::Logger;
use crate::server::session::SessionManager;
use crate::tasks::TaskList;
use crate::vfs::VirtualFs;

/// Top-level application context tying together shared infrastructure.
///
/// Holds references to the event bus, logger, config, task list, hook system,
/// session manager, and optionally the library and VFS.
///
/// # Construction
///
/// Use [`CoreContext::new`] with an [`AppConfig`] for defaults, then optionally
/// attach a library or VFS via the builder methods.
///
/// ```rust,no_run
/// use simse_core::{AppConfig, CoreContext};
///
/// let ctx = CoreContext::new(AppConfig::default());
/// assert!(ctx.library.is_none());
/// assert!(ctx.vfs.is_none());
/// ```
pub struct CoreContext {
	pub config: AppConfig,
	pub event_bus: Arc<EventBus>,
	pub logger: Logger,
	pub task_list: TaskList,
	pub hook_system: HookSystem,
	pub session_manager: SessionManager,
	pub library: Option<Library>,
	pub vfs: Option<VirtualFs>,
}

impl CoreContext {
	/// Create a new `CoreContext` with the given config and sensible defaults.
	///
	/// - Event bus: empty, shared via `Arc`
	/// - Logger: root logger with context `"simse"`
	/// - Task list: default options (max 100 tasks)
	/// - Hook system: empty
	/// - Session manager: empty
	/// - Library and VFS: `None` (attach later via builder methods)
	pub fn new(config: AppConfig) -> Self {
		Self {
			config,
			event_bus: Arc::new(EventBus::new()),
			logger: Logger::new("simse"),
			task_list: TaskList::new(None),
			hook_system: HookSystem::new(),
			session_manager: SessionManager::new(),
			library: None,
			vfs: None,
		}
	}

	/// Attach a library to the context.
	pub fn with_library(mut self, library: Library) -> Self {
		self.library = Some(library);
		self
	}

	/// Attach a virtual filesystem to the context.
	pub fn with_vfs(mut self, vfs: VirtualFs) -> Self {
		self.vfs = Some(vfs);
		self
	}
}
