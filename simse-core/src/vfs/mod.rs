//! VFS (Virtual FileSystem) orchestration layer wrapping `simse_vfs_engine`.
//!
//! Provides:
//! - [`VirtualFs`] — thread-safe wrapper around the engine VFS
//! - [`VfsDisk`] — commit VFS content to disk and load disk files into VFS
//! - [`VfsExec`] — pluggable command execution passthrough
//! - [`validators`] — file content validators for quality checks

pub mod disk;
pub mod exec;
pub mod validators;

#[allow(clippy::module_inception)]
pub mod vfs;

pub use disk::*;
pub use exec::*;
pub use validators::*;
pub use vfs::*;
