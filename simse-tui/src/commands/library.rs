//! Library commands: `/add`, `/search`, `/recommend`, `/topics`, `/volumes`,
//! `/get`, `/delete`, `/librarians`.

use super::{CommandOutput, OverlayAction};

/// `/add <topic> <text>` -- add a volume to the library.
pub fn handle_add(args: &str) -> Vec<CommandOutput> {
	let args = args.trim();
	if args.is_empty() {
		return vec![CommandOutput::Error(
			"Usage: /add <topic> <text>".into(),
		)];
	}

	// Split into topic (first word) and text (rest).
	let mut parts = args.splitn(2, ' ');
	let topic = parts.next().unwrap_or("");
	let text = parts.next().unwrap_or("").trim();

	if text.is_empty() {
		return vec![CommandOutput::Error(
			"Usage: /add <topic> <text> -- both a topic and text body are required.".into(),
		)];
	}

	vec![CommandOutput::Info(format!(
		"Would call bridge to add volume with topic=\"{topic}\" text=\"{text}\""
	))]
}

/// `/search <query>` -- search the library.
pub fn handle_search(args: &str) -> Vec<CommandOutput> {
	let query = args.trim();
	if query.is_empty() {
		return vec![CommandOutput::Error(
			"Usage: /search <query>".into(),
		)];
	}

	vec![CommandOutput::Info(format!(
		"Would call bridge to search library for \"{query}\""
	))]
}

/// `/recommend <query>` -- get recommendations from the library.
pub fn handle_recommend(args: &str) -> Vec<CommandOutput> {
	let query = args.trim();
	if query.is_empty() {
		return vec![CommandOutput::Error(
			"Usage: /recommend <query>".into(),
		)];
	}

	vec![CommandOutput::Info(format!(
		"Would call bridge to get recommendations for \"{query}\""
	))]
}

/// `/topics` -- list all topics in the library.
pub fn handle_topics(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::Info(
		"Would call bridge to list library topics".into(),
	)]
}

/// `/volumes [topic]` -- list volumes, optionally filtered by topic.
pub fn handle_volumes(args: &str) -> Vec<CommandOutput> {
	let topic = args.trim();
	if topic.is_empty() {
		vec![CommandOutput::Info(
			"Would call bridge to list all library volumes".into(),
		)]
	} else {
		vec![CommandOutput::Info(format!(
			"Would call bridge to list library volumes for topic \"{topic}\""
		))]
	}
}

/// `/get <id>` -- retrieve a volume by ID.
pub fn handle_get(args: &str) -> Vec<CommandOutput> {
	let id = args.trim();
	if id.is_empty() {
		return vec![CommandOutput::Error("Usage: /get <id>".into())];
	}

	vec![CommandOutput::Info(format!(
		"Would call bridge to get volume \"{id}\""
	))]
}

/// `/delete <id>` -- delete a volume by ID.
pub fn handle_delete(args: &str) -> Vec<CommandOutput> {
	let id = args.trim();
	if id.is_empty() {
		return vec![CommandOutput::Error("Usage: /delete <id>".into())];
	}

	vec![CommandOutput::Info(format!(
		"Would call bridge to delete volume \"{id}\""
	))]
}

/// `/librarians` -- open the librarian explorer overlay.
pub fn handle_librarians(_args: &str) -> Vec<CommandOutput> {
	vec![CommandOutput::OpenOverlay(OverlayAction::Librarians)]
}

#[cfg(test)]
mod tests {
	use super::*;

	// ── /add ─────────────────────────────────────────────

	#[test]
	fn add_empty_args_is_error() {
		let out = handle_add("");
		assert_eq!(out.len(), 1);
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("Usage")));
	}

	#[test]
	fn add_topic_only_is_error() {
		let out = handle_add("rust");
		assert_eq!(out.len(), 1);
		assert!(matches!(&out[0], CommandOutput::Error(msg) if msg.contains("topic and text")));
	}

	#[test]
	fn add_valid() {
		let out = handle_add("rust Ownership is important");
		assert_eq!(out.len(), 1);
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("topic=\"rust\"") && msg.contains("text=\"Ownership is important\""))
		);
	}

	#[test]
	fn add_trims_whitespace() {
		let out = handle_add("  topic   some text  ");
		assert_eq!(out.len(), 1);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("topic=\"topic\"")));
	}

	// ── /search ──────────────────────────────────────────

	#[test]
	fn search_empty_is_error() {
		let out = handle_search("");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	#[test]
	fn search_valid() {
		let out = handle_search("ownership borrowing");
		assert_eq!(out.len(), 1);
		assert!(
			matches!(&out[0], CommandOutput::Info(msg) if msg.contains("ownership borrowing"))
		);
	}

	#[test]
	fn search_trims() {
		let out = handle_search("  hello  ");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("\"hello\"")));
	}

	// ── /recommend ───────────────────────────────────────

	#[test]
	fn recommend_empty_is_error() {
		let out = handle_recommend("");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	#[test]
	fn recommend_valid() {
		let out = handle_recommend("async patterns");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("async patterns")));
	}

	// ── /topics ──────────────────────────────────────────

	#[test]
	fn topics_returns_info() {
		let out = handle_topics("");
		assert_eq!(out.len(), 1);
		assert!(matches!(&out[0], CommandOutput::Info(_)));
	}

	// ── /volumes ─────────────────────────────────────────

	#[test]
	fn volumes_no_args() {
		let out = handle_volumes("");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("all")));
	}

	#[test]
	fn volumes_with_topic() {
		let out = handle_volumes("rust");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("\"rust\"")));
	}

	// ── /get ─────────────────────────────────────────────

	#[test]
	fn get_empty_is_error() {
		let out = handle_get("");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	#[test]
	fn get_valid() {
		let out = handle_get("abc-123");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("abc-123")));
	}

	// ── /delete ──────────────────────────────────────────

	#[test]
	fn delete_empty_is_error() {
		let out = handle_delete("");
		assert!(matches!(&out[0], CommandOutput::Error(_)));
	}

	#[test]
	fn delete_valid() {
		let out = handle_delete("xyz-789");
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("xyz-789")));
	}

	// ── /librarians ──────────────────────────────────────

	#[test]
	fn librarians_opens_overlay() {
		let out = handle_librarians("");
		assert_eq!(out.len(), 1);
		assert!(matches!(
			&out[0],
			CommandOutput::OpenOverlay(OverlayAction::Librarians)
		));
	}
}
