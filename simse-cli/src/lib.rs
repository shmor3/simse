//! simse-cli — SimSE terminal interface.
//!
//! This crate merges the platform-agnostic UI logic (ui_core) with the
//! ratatui-based terminal frontend into a single CLI binary.

pub mod ui_core;
pub mod app;
pub mod at_mention;
pub mod autocomplete;
pub mod banner;
pub mod cli_args;
pub mod commands;
pub mod config;
pub mod dialogs;
pub mod dispatch;
pub mod error_box;
pub mod event_loop;
pub mod json_io;
pub mod levenshtein;
pub mod markdown;
pub mod onboarding;
pub mod output;
pub mod overlays;
pub mod session_store;
pub mod spinner;
pub mod status_bar;
pub mod tool_call_box;
