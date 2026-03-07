//! Cross-module integration tests for simse-ui-core.
//!
//! These tests exercise interactions between multiple modules that are not
//! covered by the per-file unit tests. They verify invariants that span
//! commands, permissions, input state, tool parsing, conversation, config
//! schemas, and text utilities.

use std::collections::HashSet;

use simse_cli::ui_core::commands::registry::{
	all_commands, filter_commands, find_command, CommandCategory,
};
use simse_cli::ui_core::config::settings_schema::all_config_schemas;
use simse_cli::ui_core::input::keybindings::{KeyCombo, KeybindingRegistry};
use simse_cli::ui_core::input::state::{self as input_state, InputState};
use simse_cli::ui_core::state::conversation::{ConversationBuffer, ConversationOptions};
use simse_cli::ui_core::state::permission_manager::PermissionManager;
use simse_cli::ui_core::state::permissions::{PermissionDecision, PermissionMode, PermissionRule};
use simse_cli::ui_core::text::file_mentions::{extract_at_query, extract_mentions, fuzzy_match};
use simse_cli::ui_core::tools::parser::parse_tool_calls;
use simse_cli::ui_core::tools::{truncate_output, ToolDefinition, ToolParameter};

// ---------------------------------------------------------------------------
// 1. All command names and aliases are globally unique
// ---------------------------------------------------------------------------

#[test]
fn all_command_names_and_aliases_are_unique() {
	let commands = all_commands();
	let mut seen = HashSet::new();

	for cmd in &commands {
		assert!(
			seen.insert(cmd.name.to_lowercase()),
			"Duplicate command name: {}",
			cmd.name
		);
		for alias in &cmd.aliases {
			assert!(
				seen.insert(alias.to_lowercase()),
				"Duplicate alias '{}' (on command '{}')",
				alias,
				cmd.name
			);
		}
	}
}

// ---------------------------------------------------------------------------
// 2. All commands have non-empty descriptions, usage, and a valid category
// ---------------------------------------------------------------------------

#[test]
fn all_commands_have_complete_definitions() {
	let commands = all_commands();

	for cmd in &commands {
		assert!(
			!cmd.name.is_empty(),
			"Command has empty name: {:?}",
			cmd
		);
		assert!(
			!cmd.description.is_empty(),
			"Command '{}' has empty description",
			cmd.name
		);
		assert!(
			!cmd.usage.is_empty(),
			"Command '{}' has empty usage",
			cmd.name
		);
		// Verify usage starts with the command name (convention)
		assert!(
			cmd.usage.starts_with(&cmd.name),
			"Command '{}' usage '{}' does not start with its name",
			cmd.name,
			cmd.usage
		);
	}
}

// ---------------------------------------------------------------------------
// 3. Parsed tool calls checked against PermissionManager
// ---------------------------------------------------------------------------
//
// Simulates: LLM produces a response with tool_use blocks -> parser extracts
// tool calls -> permission manager decides Allow/Deny/Ask for each.

#[test]
fn parsed_tool_calls_checked_by_permission_manager() {
	let response = r#"I'll read and write a file for you.
<tool_use>
{ "name": "vfs_read", "arguments": { "path": "/src/main.rs" } }
</tool_use>
<tool_use>
{ "name": "vfs_write", "arguments": { "path": "/src/out.rs", "content": "fn main() {}" } }
</tool_use>
<tool_use>
{ "name": "bash", "arguments": { "command": "cargo build" } }
</tool_use>"#;

	let parsed = parse_tool_calls(response);
	assert_eq!(parsed.tool_calls.len(), 3);

	// Default mode: read-only allowed, writes and bash ask
	let pm_default = PermissionManager::new(PermissionMode::Default);
	assert_eq!(
		pm_default.check(&parsed.tool_calls[0].name, None),
		PermissionDecision::Allow,
		"vfs_read should be allowed in Default mode"
	);
	assert_eq!(
		pm_default.check(&parsed.tool_calls[1].name, None),
		PermissionDecision::Ask,
		"vfs_write should ask in Default mode"
	);
	assert_eq!(
		pm_default.check(&parsed.tool_calls[2].name, None),
		PermissionDecision::Ask,
		"bash should ask in Default mode"
	);

	// Plan mode: read-only allowed, writes and bash denied
	let pm_plan = PermissionManager::new(PermissionMode::Plan);
	assert_eq!(
		pm_plan.check(&parsed.tool_calls[0].name, None),
		PermissionDecision::Allow,
		"vfs_read should be allowed in Plan mode"
	);
	assert_eq!(
		pm_plan.check(&parsed.tool_calls[1].name, None),
		PermissionDecision::Deny,
		"vfs_write should be denied in Plan mode"
	);
	assert_eq!(
		pm_plan.check(&parsed.tool_calls[2].name, None),
		PermissionDecision::Deny,
		"bash should be denied in Plan mode"
	);
}

// ---------------------------------------------------------------------------
// 4. Input state editing + command filtering (slash command workflow)
// ---------------------------------------------------------------------------
//
// Simulates: user types "/hel" in the input box, then we use that prefix
// to filter available commands. Exercises input::state + commands::registry.

#[test]
fn input_state_slash_command_autocomplete() {
	// Start with empty input
	let mut state = InputState {
		value: String::new(),
		cursor: 0,
		anchor: None,
	};

	// Type "/" character by character
	state = input_state::insert(&state, "/");
	assert_eq!(state.value, "/");
	assert_eq!(state.cursor, 1);

	// Type "hel"
	state = input_state::insert(&state, "h");
	state = input_state::insert(&state, "e");
	state = input_state::insert(&state, "l");
	assert_eq!(state.value, "/hel");
	assert_eq!(state.cursor, 4);

	// Extract the prefix after "/"
	let prefix = &state.value[1..]; // "hel"
	let commands = all_commands();
	let matches = filter_commands(&commands, prefix);

	// Should match "help" at minimum
	assert!(
		matches.iter().any(|c| c.name == "help"),
		"filter_commands with 'hel' should match 'help'"
	);

	// Backspace to "/he", re-filter
	state = input_state::backspace(&state);
	assert_eq!(state.value, "/he");
	let prefix2 = &state.value[1..];
	let matches2 = filter_commands(&commands, prefix2);
	assert!(matches2.iter().any(|c| c.name == "help"));

	// Select the command via find_command
	let found = find_command(&commands, "help");
	assert!(found.is_some());
	assert_eq!(found.unwrap().category, CommandCategory::Meta);
}

// ---------------------------------------------------------------------------
// 5. Conversation with tool results feeds permission-relevant tool names
// ---------------------------------------------------------------------------
//
// Exercises conversation + permission_manager: verifying that tool names
// from conversation tool results are consistently permission-checkable.

#[test]
fn conversation_tool_results_match_permission_tool_names() {
	let conv = ConversationBuffer::new(ConversationOptions {
		system_prompt: Some("You are an assistant.".into()),
		..ConversationOptions::default()
	});

	// Simulate an agentic loop: user asks, assistant responds with tool calls,
	// tool results come back, then assistant gives final answer.
	let conv = conv.add_user("Read the config file and write a summary.");
	let conv = conv.add_assistant("I'll read the file first.");

	let tool_calls = vec![
		("tc_1", "file_read", "/app/config.toml contents here"),
		("tc_2", "file_write", "Wrote summary to /app/summary.md"),
		("tc_3", "bash", "cargo test output: 42 tests passed"),
	];

	let pm = PermissionManager::new(PermissionMode::AcceptEdits);
	// Add a rule: deny bash even in AcceptEdits mode
	let pm = pm.add_rule(PermissionRule {
		tool: "bash".to_string(),
		pattern: None,
		policy: PermissionDecision::Deny,
	});

	let mut conv = conv;
	for (id, name, content) in &tool_calls {
		conv = conv.add_tool_result(id, name, content);

		let decision = pm.check(name, None);
		match *name {
			"file_read" => assert_eq!(decision, PermissionDecision::Allow),
			"file_write" => assert_eq!(decision, PermissionDecision::Allow),
			"bash" => assert_eq!(
				decision,
				PermissionDecision::Deny,
				"Rule should override AcceptEdits mode for bash"
			),
			_ => {}
		}
	}

	let conv = conv.add_assistant("All done. Here is your summary.");

	// Verify conversation state
	let messages = conv.to_messages();
	// system + user + assistant + 3 tool results + assistant = 7
	assert_eq!(messages.len(), 7);

	// Verify serialization includes tool result labels
	let serialized = conv.serialize();
	assert!(serialized.contains("[Tool Result: file_read]"));
	assert!(serialized.contains("[Tool Result: file_write]"));
	assert!(serialized.contains("[Tool Result: bash]"));
}

// ---------------------------------------------------------------------------
// 6. Config schema keys are unique within each file and across all schemas
// ---------------------------------------------------------------------------

#[test]
fn config_schema_field_keys_are_unique_within_files() {
	let schemas = all_config_schemas();

	for schema in &schemas {
		let mut keys = HashSet::new();
		for field in &schema.fields {
			assert!(
				keys.insert(&field.key),
				"Duplicate field key '{}' in config file '{}'",
				field.key,
				schema.filename
			);
			// Also verify every field has a non-empty label and description
			assert!(
				!field.label.is_empty(),
				"Field '{}' in '{}' has empty label",
				field.key,
				schema.filename
			);
			assert!(
				!field.description.is_empty(),
				"Field '{}' in '{}' has empty description",
				field.key,
				schema.filename
			);
		}
	}
}

// ---------------------------------------------------------------------------
// 7. Keybinding-driven input editing workflow
// ---------------------------------------------------------------------------
//
// Simulates: user has keybindings registered, types text, uses Ctrl+A to
// select all, then types replacement text. Exercises keybindings + input state.

#[test]
fn keybinding_triggers_select_all_then_replace() {
	// Set up keybinding registry
	let registry = KeybindingRegistry::new();
	let (registry, select_all_id) = registry.register(KeyCombo::new("a").ctrl(), "Select All");
	let (registry, _copy_id) = registry.register(KeyCombo::new("c").ctrl(), "Copy");

	// Start with some typed text
	let mut state = InputState {
		value: "old text".into(),
		cursor: 8,
		anchor: None,
	};

	// Simulate Ctrl+A keypress
	let event = KeyCombo::new("a").ctrl();
	let matched = registry.find_match(&event);
	assert!(matched.is_some());
	assert_eq!(matched.unwrap().id, select_all_id);
	assert_eq!(matched.unwrap().label, "Select All");

	// Apply select_all to input state
	state = input_state::select_all(&state);
	assert_eq!(state.anchor, Some(0));
	assert_eq!(state.cursor, 8);

	// Type replacement text (replaces selection)
	state = input_state::insert(&state, "new text");
	assert_eq!(state.value, "new text");
	assert_eq!(state.cursor, 8);
	assert!(state.anchor.is_none());

	// Verify formatting the keybinding
	let label = KeybindingRegistry::combo_to_string(&KeyCombo::new("a").ctrl());
	assert_eq!(label, "Ctrl+A");
}

// ---------------------------------------------------------------------------
// 8. File mentions extracted from input feed fuzzy command matching
// ---------------------------------------------------------------------------
//
// Exercises: input text with @mentions -> extract_mentions -> remaining text
// used to find_command. This simulates a user typing both a command and file
// mentions in the same input.

#[test]
fn file_mentions_and_command_parsing_from_input() {
	let input = "search @src/lib.rs @src/main.rs for pattern";

	// Extract @mentions
	let (cleaned, mentions) = extract_mentions(input);
	assert_eq!(mentions.len(), 2);
	assert_eq!(mentions[0], "src/lib.rs");
	assert_eq!(mentions[1], "src/main.rs");

	// The cleaned text (with mentions removed) could be used for command lookup
	let first_word = cleaned.trim().split_whitespace().next().unwrap_or("");
	let commands = all_commands();
	let found = find_command(&commands, first_word);
	assert!(found.is_some());
	assert_eq!(found.unwrap().name, "search");
	assert_eq!(found.unwrap().category, CommandCategory::Library);

	// Verify fuzzy matching on the mention paths
	assert!(fuzzy_match("lib", "src/lib.rs"));
	assert!(fuzzy_match("mn", "src/main.rs"));
	assert!(!fuzzy_match("test", "src/main.rs"));

	// Verify at-query extraction for autocomplete
	let partial_input = "search @src/m";
	assert_eq!(extract_at_query(partial_input), Some("src/m"));
}

// ---------------------------------------------------------------------------
// 9. Tool output truncation + tool definition formatting + conversation
// ---------------------------------------------------------------------------
//
// End-to-end: define a tool, format it for system prompt, simulate its output
// being truncated, then add the result to a conversation.

#[test]
fn tool_definition_format_truncate_and_conversation_flow() {
	// Define a tool
	let mut params = std::collections::HashMap::new();
	params.insert(
		"path".to_string(),
		ToolParameter {
			param_type: "string".into(),
			description: "File path to read".into(),
			required: true,
		},
	);
	let tool = ToolDefinition {
		name: "file_read".into(),
		description: "Read a file from disk".into(),
		parameters: params,
		category: simse_cli::ui_core::tools::ToolCategory::default(),
		annotations: None,
		timeout_ms: None,
		max_output_chars: Some(100),
	};

	// Format for system prompt
	let formatted = simse_cli::ui_core::tools::format_tool_definition(&tool);
	assert!(formatted.contains("### file_read"));
	assert!(formatted.contains("path (string, required)"));

	// Simulate tool execution with large output
	let big_output = "x".repeat(200);
	let max_chars = tool.max_output_chars.unwrap();
	let truncated = truncate_output(&big_output, max_chars);
	assert!(truncated.len() < big_output.len());
	assert!(truncated.ends_with("[OUTPUT TRUNCATED]"));

	// Add the truncated output as a tool result in the conversation
	let conv = ConversationBuffer::new(ConversationOptions {
		system_prompt: Some(simse_cli::ui_core::tools::format_tools_for_system_prompt(&[
			tool,
		])),
		..ConversationOptions::default()
	});
	let conv = conv.add_user("Read the file at /app/big.log");
	let conv = conv.add_tool_result("tc_1", "file_read", &truncated);
	let conv = conv.add_assistant("The file was too large, output was truncated.");

	// Verify the conversation structure
	let messages = conv.to_messages();
	assert_eq!(messages.len(), 4); // system + user + tool_result + assistant

	// Verify the system prompt contains the tool definition
	assert!(messages[0].content.contains("<tool_use>"));
	assert!(messages[0].content.contains("file_read"));
	assert!(messages[0].content.contains("</tool_use>"));

	// Verify the tool result contains truncation marker
	assert!(messages[2].content.contains("[OUTPUT TRUNCATED]"));
	assert_eq!(messages[2].tool_name.as_deref(), Some("file_read"));

	// Verify permission check for the tool used
	let pm = PermissionManager::new(PermissionMode::Default);
	assert_eq!(pm.check("file_read", None), PermissionDecision::Allow);
}
