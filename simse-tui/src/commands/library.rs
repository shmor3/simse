//! Library commands: `/add`, `/search`, `/recommend`, `/topics`, `/volumes`,
//! `/get`, `/delete`, `/librarians`.

use super::{BridgeAction, CommandOutput, OverlayAction};

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

	vec![
		CommandOutput::Info("Adding to library...".into()),
		CommandOutput::BridgeRequest(BridgeAction::LibraryAdd {
			topic: topic.into(),
			text: text.into(),
		}),
	]
}

/// `/search <query>` -- search the library.
pub fn handle_search(args: &str) -> Vec<CommandOutput> {
	let query = args.trim();
	if query.is_empty() {
		return vec![CommandOutput::Error(
			"Usage: /search <query>".into(),
		)];
	}

	vec![
		CommandOutput::Info(format!("Searching library for: {query}")),
		CommandOutput::BridgeRequest(BridgeAction::LibrarySearch {
			query: query.into(),
		}),
	]
}

/// `/recommend <query>` -- get recommendations from the library.
pub fn handle_recommend(args: &str) -> Vec<CommandOutput> {
	let query = args.trim();
	if query.is_empty() {
		return vec![CommandOutput::Error(
			"Usage: /recommend <query>".into(),
		)];
	}

	vec![
		CommandOutput::Info("Getting recommendations...".into()),
		CommandOutput::BridgeRequest(BridgeAction::LibraryRecommend {
			query: query.into(),
		}),
	]
}

/// `/topics` -- list all topics in the library.
pub fn handle_topics(_args: &str) -> Vec<CommandOutput> {
	vec![
		CommandOutput::Info("Listing library topics...".into()),
		CommandOutput::BridgeRequest(BridgeAction::LibraryTopics),
	]
}

/// `/volumes [topic]` -- list volumes, optionally filtered by topic.
pub fn handle_volumes(args: &str) -> Vec<CommandOutput> {
	let topic = args.trim();
	let topic = if topic.is_empty() {
		None
	} else {
		Some(topic.into())
	};
	vec![
		CommandOutput::Info("Listing library volumes...".into()),
		CommandOutput::BridgeRequest(BridgeAction::LibraryVolumes { topic }),
	]
}

/// `/get <id>` -- retrieve a volume by ID.
pub fn handle_get(args: &str) -> Vec<CommandOutput> {
	let id = args.trim();
	if id.is_empty() {
		return vec![CommandOutput::Error("Usage: /get <id>".into())];
	}

	vec![
		CommandOutput::Info("Retrieving volume...".into()),
		CommandOutput::BridgeRequest(BridgeAction::LibraryGet {
			id: id.into(),
		}),
	]
}

/// `/delete <id>` -- delete a volume by ID.
pub fn handle_delete(args: &str) -> Vec<CommandOutput> {
	let id = args.trim();
	if id.is_empty() {
		return vec![CommandOutput::Error("Usage: /delete <id>".into())];
	}

	vec![
		CommandOutput::Info("Deleting volume...".into()),
		CommandOutput::BridgeRequest(BridgeAction::LibraryDelete {
			id: id.into(),
		}),
	]
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
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "Adding to library..."));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibraryAdd { topic, text })
				if topic == "rust" && text == "Ownership is important"
		));
	}

	#[test]
	fn add_trims_whitespace() {
		let out = handle_add("  topic   some text  ");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(_)));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibraryAdd { topic, .. })
				if topic == "topic"
		));
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
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg.contains("ownership borrowing")));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibrarySearch { query })
				if query == "ownership borrowing"
		));
	}

	#[test]
	fn search_trims() {
		let out = handle_search("  hello  ");
		assert!(matches!(&out[0], CommandOutput::Info(_)));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibrarySearch { query })
				if query == "hello"
		));
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
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "Getting recommendations..."));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibraryRecommend { query })
				if query == "async patterns"
		));
	}

	// ── /topics ──────────────────────────────────────────

	#[test]
	fn topics_returns_bridge_request() {
		let out = handle_topics("");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "Listing library topics..."));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibraryTopics)
		));
	}

	// ── /volumes ─────────────────────────────────────────

	#[test]
	fn volumes_no_args() {
		let out = handle_volumes("");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "Listing library volumes..."));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibraryVolumes { topic })
				if topic.is_none()
		));
	}

	#[test]
	fn volumes_with_topic() {
		let out = handle_volumes("rust");
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(_)));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibraryVolumes { topic })
				if topic.as_deref() == Some("rust")
		));
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
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "Retrieving volume..."));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibraryGet { id })
				if id == "abc-123"
		));
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
		assert_eq!(out.len(), 2);
		assert!(matches!(&out[0], CommandOutput::Info(msg) if msg == "Deleting volume..."));
		assert!(matches!(
			&out[1],
			CommandOutput::BridgeRequest(BridgeAction::LibraryDelete { id })
				if id == "xyz-789"
		));
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
