# SimSE Rust Rewrite — Phase 3: UI Core State Models

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Expand simse-ui-core with full logic for commands, file mentions, diff display, image input, tool registry formatting, and settings schema. Port ~1800 LOC of platform-agnostic logic from TypeScript.

**Architecture:** Fully functional (pure functions, enums + pattern matching, no OOP). All modules are in simse-ui-core (no I/O — file reads happen in simse-bridge/simse-tui and pass data in).

**Tech Stack:** Rust, serde, serde_json, regex

---

## Task 11: Expand command registry with all command definitions

**Files:**
- Modify: `simse-ui-core/src/commands/registry.rs`
- Modify: `simse-ui-core/Cargo.toml` (may need to re-add deps)

**Step 1: Write failing tests**

Add to `simse-ui-core/src/commands/registry.rs` test module:

```rust
#[test]
fn all_commands_registered() {
    let cmds = all_commands();
    // Should have at least 20 commands from all categories
    assert!(cmds.len() >= 20);
}

#[test]
fn all_categories_represented() {
    let cmds = all_commands();
    let categories: std::collections::HashSet<_> = cmds.iter().map(|c| &c.category).collect();
    assert!(categories.contains(&CommandCategory::Meta));
    assert!(categories.contains(&CommandCategory::Library));
    assert!(categories.contains(&CommandCategory::Tools));
    assert!(categories.contains(&CommandCategory::Session));
    assert!(categories.contains(&CommandCategory::Config));
    assert!(categories.contains(&CommandCategory::Files));
    assert!(categories.contains(&CommandCategory::Ai));
}

#[test]
fn parse_bool_arg_on_off() {
    assert_eq!(parse_bool_arg("on", false), Some(true));
    assert_eq!(parse_bool_arg("off", true), Some(false));
    assert_eq!(parse_bool_arg("true", false), Some(true));
    assert_eq!(parse_bool_arg("false", true), Some(false));
    assert_eq!(parse_bool_arg("1", false), Some(true));
    assert_eq!(parse_bool_arg("0", true), Some(false));
}

#[test]
fn parse_bool_arg_toggle() {
    assert_eq!(parse_bool_arg("", true), Some(false));
    assert_eq!(parse_bool_arg("", false), Some(true));
}

#[test]
fn parse_bool_arg_invalid() {
    assert_eq!(parse_bool_arg("maybe", false), None);
}

#[test]
fn find_exit_by_alias() {
    let cmds = all_commands();
    let exit = find_command(&cmds, "q");
    assert!(exit.is_some());
    assert_eq!(exit.unwrap().name, "exit");
}
```

**Step 2: Implement**

Replace `CommandCategory` with an enum, add `all_commands()` and `parse_bool_arg()`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandCategory {
    Meta,
    Library,
    Tools,
    Session,
    Config,
    Files,
    Ai,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDefinition {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub aliases: Vec<String>,
    pub category: CommandCategory,
    pub hidden: bool,
}

/// Parse "on"/"off"/"true"/"false"/"1"/"0" or empty (toggle).
pub fn parse_bool_arg(arg: &str, current: bool) -> Option<bool> {
    match arg.trim().to_lowercase().as_str() {
        "" => Some(!current),
        "on" | "true" | "1" => Some(true),
        "off" | "false" | "0" => Some(false),
        _ => None,
    }
}

/// All built-in command definitions.
pub fn all_commands() -> Vec<CommandDefinition> {
    let mut cmds = Vec::new();

    // Meta commands
    cmds.push(cmd("help", "Show available commands", "help", &["?"], CommandCategory::Meta));
    cmds.push(cmd("clear", "Clear conversation history", "clear", &[], CommandCategory::Meta));
    cmds.push(cmd("verbose", "Toggle verbose output", "verbose [on|off]", &["v"], CommandCategory::Meta));
    cmds.push(cmd("plan", "Toggle plan mode", "plan [on|off]", &[], CommandCategory::Meta));
    cmds.push(cmd("context", "Show context window usage", "context", &[], CommandCategory::Meta));
    cmds.push(cmd("compact", "Compact conversation history", "compact", &[], CommandCategory::Meta));
    cmds.push(cmd("exit", "Exit the application", "exit", &["quit", "q"], CommandCategory::Meta));

    // Library commands
    cmds.push(cmd("add", "Add a volume to the library", "add <topic> <text>", &[], CommandCategory::Library));
    cmds.push(cmd("search", "Search the library", "search <query>", &["s"], CommandCategory::Library));
    cmds.push(cmd("recommend", "Get recommendations", "recommend <query>", &["rec"], CommandCategory::Library));
    cmds.push(cmd("topics", "List all topics", "topics", &[], CommandCategory::Library));
    cmds.push(cmd("volumes", "List volumes", "volumes [topic]", &["ls"], CommandCategory::Library));
    cmds.push(cmd("get", "Get a volume by ID", "get <id>", &[], CommandCategory::Library));
    cmds.push(cmd("delete", "Delete a volume", "delete <id>", &["rm"], CommandCategory::Library));

    // Tools commands
    cmds.push(cmd("tools", "List available tools", "tools", &[], CommandCategory::Tools));
    cmds.push(cmd("agents", "List available agents", "agents", &[], CommandCategory::Tools));
    cmds.push(cmd("skills", "List available skills", "skills", &[], CommandCategory::Tools));

    // Session commands
    cmds.push(cmd("sessions", "List saved sessions", "sessions", &[], CommandCategory::Session));
    cmds.push(cmd("resume", "Resume a session", "resume <id>", &["r"], CommandCategory::Session));
    cmds.push(cmd("rename", "Rename current session", "rename <title>", &[], CommandCategory::Session));
    cmds.push(cmd("server", "Show active ACP server", "server", &[], CommandCategory::Session));
    cmds.push(cmd("model", "Show active model", "model", &[], CommandCategory::Session));
    cmds.push(cmd("mcp", "Show MCP status", "mcp", &[], CommandCategory::Session));
    cmds.push(cmd("acp", "Show ACP status", "acp", &[], CommandCategory::Session));

    // Config commands
    cmds.push(cmd("config", "Show configuration", "config", &[], CommandCategory::Config));
    cmds.push(cmd("settings", "Browse settings", "settings", &["set"], CommandCategory::Config));

    // Files commands
    cmds.push(cmd("files", "List VFS files", "files [path]", &[], CommandCategory::Files));
    cmds.push(cmd("save", "Save VFS to disk", "save [path]", &[], CommandCategory::Files));
    cmds.push(cmd("validate", "Validate VFS files", "validate [path]", &[], CommandCategory::Files));
    cmds.push(cmd("discard", "Discard VFS changes", "discard [path]", &[], CommandCategory::Files));
    cmds.push(cmd("diff", "Show VFS diffs", "diff [path]", &[], CommandCategory::Files));

    // AI commands
    cmds.push(cmd("chain", "Run a prompt chain", "chain <name> [args]", &["prompt"], CommandCategory::Ai));
    cmds.push(cmd("prompts", "List prompt templates", "prompts", &[], CommandCategory::Ai));

    cmds
}

fn cmd(name: &str, desc: &str, usage: &str, aliases: &[&str], category: CommandCategory) -> CommandDefinition {
    CommandDefinition {
        name: name.into(),
        description: desc.into(),
        usage: usage.into(),
        aliases: aliases.iter().map(|a| (*a).into()).collect(),
        category,
        hidden: false,
    }
}
```

Update `find_command` and `filter_commands` to use the new `CommandDefinition` (which now has `usage` field and typed `category`).

**Step 3: Run tests, commit**

---

## Task 12: Expand file-mentions with full logic

**Files:**
- Modify: `simse-ui-core/src/text/file_mentions.rs`
- Modify: `simse-ui-core/Cargo.toml` (add `regex` back)

**Step 1: Write failing tests**

```rust
#[test]
fn extract_mentions_from_input() {
    let (clean, mentions) = extract_mentions("check @src/main.rs please");
    assert_eq!(clean, "check  please");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0], "src/main.rs");
}

#[test]
fn extract_mentions_multiple() {
    let (clean, mentions) = extract_mentions("compare @a.ts and @b.ts");
    assert_eq!(mentions.len(), 2);
}

#[test]
fn extract_mentions_vfs() {
    let (_, mentions) = extract_mentions("read @vfs://output.json");
    assert_eq!(mentions.len(), 1);
    assert_eq!(mentions[0], "vfs://output.json");
}

#[test]
fn extract_mentions_deduplicates() {
    let (_, mentions) = extract_mentions("@file.rs and @file.rs again");
    assert_eq!(mentions.len(), 1);
}

#[test]
fn format_file_context() {
    let ctx = format_mention_context("src/main.rs", "fn main() {}", "file");
    assert!(ctx.contains("<file path=\"src/main.rs\">"));
    assert!(ctx.contains("fn main() {}"));
    assert!(ctx.contains("</file>"));
}

#[test]
fn format_volume_context() {
    let ctx = format_mention_context("abc123", "some text", "volume");
    assert!(ctx.contains("<volume id=\"abc123\">"));
}

#[test]
fn fuzzy_match_basic() {
    assert!(fuzzy_match("mn", "main"));
    assert!(fuzzy_match("abc", "a_b_c"));
    assert!(!fuzzy_match("xyz", "abc"));
}

#[test]
fn fuzzy_match_empty_query() {
    assert!(fuzzy_match("", "anything"));
}
```

**Step 2: Implement**

```rust
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

static MENTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"@(vfs://[\w./-]+|[\w./-]+(?:\.\w+)?)").unwrap()
});

/// Extract @-mentions from input. Returns (cleaned input, unique mention paths).
pub fn extract_mentions(input: &str) -> (String, Vec<String>) {
    let mut seen = HashSet::new();
    let mut mentions = Vec::new();
    let clean = MENTION_RE.replace_all(input, |caps: &regex::Captures| {
        let mention = caps[1].to_string();
        if seen.insert(mention.clone()) {
            mentions.push(mention);
        }
        ""
    });
    (clean.into_owned(), mentions)
}

/// Format a file/volume mention as XML context for AI prompts.
pub fn format_mention_context(path_or_id: &str, content: &str, kind: &str) -> String {
    match kind {
        "volume" => format!("<volume id=\"{path_or_id}\">\n{content}\n</volume>"),
        _ => format!("<file path=\"{path_or_id}\">\n{content}\n</file>"),
    }
}

/// Simple fuzzy match: all chars in query appear in order in target.
pub fn fuzzy_match(query: &str, target: &str) -> bool {
    let mut target_chars = target.chars();
    for qc in query.chars() {
        let qc_lower = qc.to_ascii_lowercase();
        loop {
            match target_chars.next() {
                Some(tc) if tc.to_ascii_lowercase() == qc_lower => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

/// Format file size as human-readable string.
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{}KB", bytes / 1024)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Check if a string looks like a volume ID (8 hex chars).
pub fn is_volume_id(s: &str) -> bool {
    s.len() == 8 && s.chars().all(|c| c.is_ascii_hexdigit())
}
```

**Step 3: Run tests, commit**

---

## Task 13: Expand diff module with inline diff and rendering

**Files:**
- Modify: `simse-ui-core/src/diff.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn compute_inline_diff_changed_middle() {
    let result = compute_inline_diff("hello world", "hello rust");
    assert_eq!(result.old_segments.len(), 2); // "hello " unchanged, "world" changed
    assert_eq!(result.new_segments.len(), 2); // "hello " unchanged, "rust" changed
}

#[test]
fn compute_inline_diff_identical() {
    let result = compute_inline_diff("same", "same");
    assert_eq!(result.old_segments.len(), 1);
    assert!(!result.old_segments[0].changed);
}

#[test]
fn compute_inline_diff_empty() {
    let result = compute_inline_diff("", "new");
    assert_eq!(result.new_segments.len(), 1);
    assert!(result.new_segments[0].changed);
}

#[test]
fn pair_diff_lines_matches_removes_with_adds() {
    let lines = vec![
        DiffLine::Removed("old1".into()),
        DiffLine::Removed("old2".into()),
        DiffLine::Added("new1".into()),
        DiffLine::Added("new2".into()),
    ];
    let paired = pair_diff_lines(&lines);
    assert_eq!(paired.len(), 2); // 2 pairs
}

#[test]
fn format_diff_stats() {
    let hunks = vec![DiffHunk {
        old_start: 1, old_count: 2, new_start: 1, new_count: 3,
        lines: vec![
            DiffLine::Context("ctx".into()),
            DiffLine::Removed("old".into()),
            DiffLine::Added("new1".into()),
            DiffLine::Added("new2".into()),
        ],
    }];
    let (additions, deletions) = count_diff_stats(&hunks);
    assert_eq!(additions, 2);
    assert_eq!(deletions, 1);
}
```

**Step 2: Implement**

Add to `diff.rs`:

```rust
#[derive(Debug, Clone)]
pub struct InlineDiffSegment {
    pub text: String,
    pub changed: bool,
}

#[derive(Debug, Clone)]
pub struct InlineDiffResult {
    pub old_segments: Vec<InlineDiffSegment>,
    pub new_segments: Vec<InlineDiffSegment>,
}

/// Compute inline diff between two strings by finding common prefix/suffix.
pub fn compute_inline_diff(old: &str, new: &str) -> InlineDiffResult { ... }

/// Pair contiguous remove/add blocks for inline highlighting.
pub fn pair_diff_lines(lines: &[DiffLine]) -> Vec<(String, String)> { ... }

/// Count additions and deletions across hunks.
pub fn count_diff_stats(hunks: &[DiffHunk]) -> (usize, usize) { ... }

/// Format a hunk header string.
pub fn format_hunk_header(hunk: &DiffHunk) -> String { ... }
```

**Step 3: Run tests, commit**

---

## Task 14: Add image input detection

**Files:**
- Create: `simse-ui-core/src/text/image_input.rs`
- Modify: `simse-ui-core/src/text/mod.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn detect_image_paths_in_input() {
    let (clean, paths) = detect_image_paths("check image.png please");
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0], "image.png");
    assert!(!clean.contains("image.png"));
}

#[test]
fn detect_multiple_image_formats() {
    let (_, paths) = detect_image_paths("see a.jpg b.webp c.gif");
    assert_eq!(paths.len(), 3);
}

#[test]
fn detect_no_images() {
    let (clean, paths) = detect_image_paths("no images here");
    assert_eq!(paths.len(), 0);
    assert_eq!(clean, "no images here");
}

#[test]
fn mime_type_mapping() {
    assert_eq!(image_mime_type("png"), Some("image/png"));
    assert_eq!(image_mime_type("jpg"), Some("image/jpeg"));
    assert_eq!(image_mime_type("jpeg"), Some("image/jpeg"));
    assert_eq!(image_mime_type("gif"), Some("image/gif"));
    assert_eq!(image_mime_type("webp"), Some("image/webp"));
    assert_eq!(image_mime_type("svg"), Some("image/svg+xml"));
    assert_eq!(image_mime_type("txt"), None);
}

#[test]
fn format_size_ranges() {
    assert_eq!(super::file_mentions::format_size(500), "500B");
    assert_eq!(super::file_mentions::format_size(2048), "2KB");
    assert_eq!(super::file_mentions::format_size(1_500_000), "1.4MB");
}
```

**Step 2: Implement**

```rust
use regex::Regex;
use std::sync::LazyLock;

static IMAGE_PATH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|\s)((?:\.{0,2}[\\/])?[\w./-]+\.(?:png|jpg|jpeg|gif|webp|bmp|svg))(?:\s|$)").unwrap()
});

/// Detect image file paths in input. Returns (cleaned input, paths).
pub fn detect_image_paths(input: &str) -> (String, Vec<String>) { ... }

/// Get MIME type for an image file extension (lowercase, no dot).
pub fn image_mime_type(ext: &str) -> Option<&'static str> {
    match ext {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}
```

**Step 3: Run tests, commit**

---

## Task 15: Expand settings schema with all config file definitions

**Files:**
- Modify: `simse-ui-core/src/config/settings_schema.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn all_config_schemas_present() {
    let schemas = all_config_schemas();
    assert_eq!(schemas.len(), 6);
    let filenames: Vec<_> = schemas.iter().map(|s| s.filename.as_str()).collect();
    assert!(filenames.contains(&"config.json"));
    assert!(filenames.contains(&"acp.json"));
    assert!(filenames.contains(&"embed.json"));
    assert!(filenames.contains(&"memory.json"));
    assert!(filenames.contains(&"summarize.json"));
    assert!(filenames.contains(&"settings.json"));
}

#[test]
fn config_json_has_log_level() {
    let schema = get_config_schema("config.json").unwrap();
    let field = schema.fields.iter().find(|f| f.key == "logLevel").unwrap();
    assert!(matches!(field.field_type, FieldType::Select { .. }));
}

#[test]
fn memory_json_has_enabled_bool() {
    let schema = get_config_schema("memory.json").unwrap();
    let field = schema.fields.iter().find(|f| f.key == "enabled").unwrap();
    assert!(matches!(field.field_type, FieldType::Boolean));
}

#[test]
fn get_unknown_schema_returns_none() {
    assert!(get_config_schema("nonexistent.json").is_none());
}
```

**Step 2: Implement**

Add `ConfigFileSchema`, `all_config_schemas()`, and `get_config_schema()`.

**Step 3: Run tests, commit**

---

## Task 16: Add tool registry formatting

**Files:**
- Modify: `simse-ui-core/src/tools/mod.rs`

**Step 1: Write failing tests**

```rust
#[test]
fn format_tool_for_system_prompt() {
    let tool = ToolDefinition {
        name: "library_search".into(),
        description: "Search the library".into(),
        parameters: vec![
            ToolParameter { name: "query".into(), param_type: "string".into(), description: "Search query".into(), required: true },
            ToolParameter { name: "maxResults".into(), param_type: "number".into(), description: "Max results".into(), required: false },
        ],
    };
    let formatted = format_tool_definition(&tool);
    assert!(formatted.contains("library_search"));
    assert!(formatted.contains("query (string, required)"));
    assert!(formatted.contains("maxResults (number)"));
}

#[test]
fn format_tools_system_prompt_header() {
    let tools = vec![ToolDefinition {
        name: "test_tool".into(),
        description: "A test".into(),
        parameters: vec![],
    }];
    let prompt = format_tools_for_system_prompt(&tools);
    assert!(prompt.contains("<tool_use>"));
    assert!(prompt.contains("test_tool"));
}

#[test]
fn truncate_tool_output_short() {
    let output = "short output";
    assert_eq!(truncate_output(output, 1000), output);
}

#[test]
fn truncate_tool_output_long() {
    let output = "x".repeat(100);
    let truncated = truncate_output(&output, 50);
    assert_eq!(truncated.len(), 50 + "[OUTPUT TRUNCATED]".len());
    assert!(truncated.ends_with("[OUTPUT TRUNCATED]"));
}
```

**Step 2: Implement**

Add `format_tool_definition()`, `format_tools_for_system_prompt()`, and `truncate_output()`.

**Step 3: Run tests, commit**

---

## Task 17: Add output types and core enums

**Files:**
- Modify: `simse-ui-core/src/app.rs`

**Step 1: Implement core output/state types**

Port the key types from `ink-types.ts`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputItem {
    Message { role: String, text: String },
    ToolCall(ToolCallState),
    CommandResult { text: String },
    Error { message: String },
    Info { text: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallState {
    pub id: String,
    pub name: String,
    pub args: String,
    pub status: ToolCallStatus,
    pub started_at: i64,
    pub duration_ms: Option<u64>,
    pub summary: Option<String>,
    pub error: Option<String>,
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCallStatus {
    Active,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: String,
    pub tool_name: String,
    pub args: serde_json::Value,
    pub options: Vec<PermissionOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionOption {
    pub id: String,
    pub label: String,
}
```

**Step 2: Run tests, commit**

---

## Implementation Notes

- **No file I/O in simse-ui-core**: Functions like `detect_image_paths` and `extract_mentions` only parse text and return paths. Actual file reading happens in simse-bridge or simse-tui.
- **Regex**: Re-add `regex` to simse-ui-core Cargo.toml for mention/image parsing.
- **LazyLock**: Use `std::sync::LazyLock` (stable in Rust 1.80+) for compiled regex statics.
- **Each task should be committed separately** with descriptive messages.
