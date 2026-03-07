//! PTY tests verifying that simse-tui starts up correctly.

use super::r#mod::*;
use tempfile::TempDir;

#[test]
fn startup_shows_banner() {
	let tmp = TempDir::new().unwrap();
	let h = spawn_simse(tmp.path());
	h.wait_for_text("simse v").expect("banner should contain 'simse v'");
}

#[test]
fn startup_shows_input_prompt() {
	let tmp = TempDir::new().unwrap();
	let h = spawn_simse(tmp.path());
	h.wait_for_text("Input").expect("input block title 'Input' should appear");
}

#[test]
fn startup_shows_status_bar() {
	let tmp = TempDir::new().unwrap();
	let h = spawn_simse(tmp.path());
	// The status bar renders on the very last row. Due to ConPTY + vtparse
	// limitations on Windows, the last row's content may not be captured by
	// the VT parser. Instead, verify the app's default idle status via the
	// banner area which shows "Ready." when the loop is idle.
	h.wait_for_text("Ready.")
		.expect("app should show 'Ready.' status in banner");
}

#[test]
fn startup_shows_version() {
	let tmp = TempDir::new().unwrap();
	let h = spawn_simse(tmp.path());
	h.wait_for_text("v0.").expect("version string 'v0.' should appear in banner");
}

#[test]
fn startup_shows_tips() {
	let tmp = TempDir::new().unwrap();
	let h = spawn_simse(tmp.path());
	h.wait_for_text("Tips").expect("'Tips' heading should appear");
	h.wait_for_text("/help").expect("'/help' tip should appear");
}
