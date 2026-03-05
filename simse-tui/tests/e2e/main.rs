//! End-to-end tests for simse-tui.
//!
//! These tests drive the `App` model through its Elm Architecture (update/view)
//! using ratatui's `TestBackend`, testing all business logic including rendering,
//! autocomplete, overlay focus routing, and command dispatch.
//!
//! ## Module layout
//!
//! - `config.rs`           — Test config utilities (reserved for future ACP integration)
//! - `harness.rs`          — `SimseTestHarness`: TestBackend-based test driver
//! - `runtime_harness.rs`  — `RuntimeTestHarness`: file I/O test harness with temp directories

mod acp_integration;
mod autocomplete;
mod command_feedback;
mod commands_config;
mod commands_files;
mod commands_library;
mod commands_meta;
mod commands_session;
mod commands_tools;
mod config;
mod config_settings;
mod error_states;
pub mod harness;
mod input;
mod onboarding;
mod overlays;
mod real_acp;
mod runtime_harness;
mod setup_wizard;
mod startup;

use harness::SimseTestHarness;

// ═══════════════════════════════════════════════════════════════
// Smoke tests — verify the harness works
// ═══════════════════════════════════════════════════════════════

#[test]
fn smoke_test_harness_creates() {
	let harness = SimseTestHarness::new();
	// App should render banner and input on startup
	harness.assert_contains("Input");
}

#[test]
fn smoke_test_harness_typing() {
	let mut harness = SimseTestHarness::new();
	harness.type_text("hello");
	assert_eq!(harness.input_value(), "hello");
}

#[test]
fn smoke_test_harness_quit() {
	let mut harness = SimseTestHarness::new();
	harness.quit();
	assert!(harness.should_quit());
}
