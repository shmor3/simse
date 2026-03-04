//! File commands: `/files`, `/save`, `/validate`, `/discard`, `/diff`.

use super::{BridgeAction, CommandOutput};

/// `/files [path]` -- list files in the virtual filesystem.
pub fn handle_files(args: &str) -> Vec<CommandOutput> {
	let path = args.trim();
	if path.is_empty() {
		vec![CommandOutput::BridgeRequest(BridgeAction::ListFiles {
			path: None,
		})]
	} else {
		// Basic path validation.
		if path.contains('\0') {
			return vec![CommandOutput::Error(
				"Invalid path: contains null bytes".into(),
			)];
		}
		vec![CommandOutput::BridgeRequest(BridgeAction::ListFiles {
			path: Some(path.into()),
		})]
	}
}

/// `/save [path]` -- save a virtual file to disk.
pub fn handle_save(args: &str) -> Vec<CommandOutput> {
	let path = args.trim();
	if path.is_empty() {
		vec![CommandOutput::BridgeRequest(BridgeAction::SaveFiles {
			path: None,
		})]
	} else {
		if path.contains('\0') {
			return vec![CommandOutput::Error(
				"Invalid path: contains null bytes".into(),
			)];
		}
		vec![CommandOutput::BridgeRequest(BridgeAction::SaveFiles {
			path: Some(path.into()),
		})]
	}
}

/// `/validate [path]` -- validate virtual file contents.
pub fn handle_validate(args: &str) -> Vec<CommandOutput> {
	let path = args.trim();
	if path.is_empty() {
		vec![CommandOutput::BridgeRequest(BridgeAction::ValidateFiles {
			path: None,
		})]
	} else {
		if path.contains('\0') {
			return vec![CommandOutput::Error(
				"Invalid path: contains null bytes".into(),
			)];
		}
		vec![CommandOutput::BridgeRequest(BridgeAction::ValidateFiles {
			path: Some(path.into()),
		})]
	}
}

/// `/discard [path]` -- discard virtual file changes.
pub fn handle_discard(args: &str) -> Vec<CommandOutput> {
	let path = args.trim();
	if path.is_empty() {
		return vec![CommandOutput::Error(
			"Usage: /discard <path> -- specify which file to discard".into(),
		)];
	}

	if path.contains('\0') {
		return vec![CommandOutput::Error(
			"Invalid path: contains null bytes".into(),
		)];
	}

	vec![CommandOutput::BridgeRequest(BridgeAction::DiscardFile {
		path: path.into(),
	})]
}

/// `/diff [path]` -- show diff of virtual file changes.
pub fn handle_diff(args: &str) -> Vec<CommandOutput> {
	let path = args.trim();
	if path.is_empty() {
		vec![CommandOutput::BridgeRequest(BridgeAction::DiffFiles {
			path: None,
		})]
	} else {
		if path.contains('\0') {
			return vec![CommandOutput::Error(
				"Invalid path: contains null bytes".into(),
			)];
		}
		vec![CommandOutput::BridgeRequest(BridgeAction::DiffFiles {
			path: Some(path.into()),
		})]
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// ── /files ───────────────────────────────────────────

	#[test]
	fn files_no_args_lists_all() {
		let out = handle_files("");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::ListFiles { path: None })
		));
	}

	#[test]
	fn files_with_path() {
		let out = handle_files("src/main.rs");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::ListFiles { path: Some(p) }) if p == "src/main.rs"
		));
	}

	#[test]
	fn files_null_byte_is_error() {
		let out = handle_files("bad\0path");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("null")));
	}

	#[test]
	fn files_trims_whitespace() {
		let out = handle_files("  /some/path  ");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::ListFiles { path: Some(p) }) if p == "/some/path"
		));
	}

	// ── /save ────────────────────────────────────────────

	#[test]
	fn save_no_args_saves_all() {
		let out = handle_save("");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::SaveFiles { path: None })
		));
	}

	#[test]
	fn save_with_path() {
		let out = handle_save("output.txt");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::SaveFiles { path: Some(p) }) if p == "output.txt"
		));
	}

	#[test]
	fn save_null_byte_is_error() {
		let out = handle_save("bad\0file");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	// ── /validate ────────────────────────────────────────

	#[test]
	fn validate_no_args_validates_all() {
		let out = handle_validate("");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::ValidateFiles { path: None })
		));
	}

	#[test]
	fn validate_with_path() {
		let out = handle_validate("config.toml");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::ValidateFiles { path: Some(p) }) if p == "config.toml"
		));
	}

	#[test]
	fn validate_null_byte_is_error() {
		let out = handle_validate("x\0y");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	// ── /discard ─────────────────────────────────────────

	#[test]
	fn discard_no_args_is_error() {
		let out = handle_discard("");
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("Usage")));
	}

	#[test]
	fn discard_with_path() {
		let out = handle_discard("temp.rs");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::DiscardFile { path }) if path == "temp.rs"
		));
	}

	#[test]
	fn discard_null_byte_is_error() {
		let out = handle_discard("a\0b");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	// ── /diff ────────────────────────────────────────────

	#[test]
	fn diff_no_args_shows_all() {
		let out = handle_diff("");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::DiffFiles { path: None })
		));
	}

	#[test]
	fn diff_with_path() {
		let out = handle_diff("lib.rs");
		assert!(matches!(
			&out[0],
			CommandOutput::BridgeRequest(BridgeAction::DiffFiles { path: Some(p) }) if p == "lib.rs"
		));
	}

	#[test]
	fn diff_null_byte_is_error() {
		let out = handle_diff("z\0z");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}
}
