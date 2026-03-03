//! simse-ui-core — Platform-agnostic UI logic for SimSE
//!
//! This crate contains all state machines, data models, and business logic
//! that any SimSE frontend (TUI, web, native) can reuse. It has no
//! rendering or I/O dependencies.

pub mod app;
pub mod agentic_loop;
pub mod commands;
pub mod config;
pub mod diff;
pub mod input;
pub mod skills;
pub mod state;
pub mod text;
pub mod tools;
