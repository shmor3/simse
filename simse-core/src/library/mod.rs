//! Library orchestration layer wrapping `crate::adaptive::store::Store`.
//!
//! Provides:
//! - [`Library`] — high-level API with automatic embedding, event publishing, and shelf management
//! - [`Shelf`] — agent-scoped partition that tags and filters entries by shelf name
//! - Re-exports of [`query_dsl`] and [`prompt_inject`] from the vector engine
//!
//! The library wraps a `Store` behind `Arc<Mutex<_>>` so it can be shared
//! safely across async tasks and passed to shelves.

pub mod circulation;
pub mod librarian;
pub mod librarian_def;
pub mod librarian_reg;
pub mod prompt_inject;
pub mod query_dsl;
pub mod services;
pub mod shelf;

#[allow(clippy::module_inception)]
mod library;

pub use library::*;
pub use prompt_inject::*;
pub use query_dsl::*;
pub use shelf::*;
