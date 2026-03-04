# simse-code → Rust Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Complete the migration from simse-code (TypeScript/Ink) to three Rust crates (simse-ui-core, simse-tui, simse-bridge), achieving full feature parity with zero regressions, then delete simse-code.

**Architecture:** Three Rust crates: simse-ui-core (platform-agnostic state machines, data models, business logic — no I/O), simse-tui (ratatui terminal UI with Elm Architecture), simse-bridge (async I/O: ACP client, config, sessions, storage, MCP).

**Tech Stack:** Rust, ratatui, crossterm, tokio, serde_json, flate2, pulldown-cmark, clap, candle/ort (embedding), dirs, uuid, chrono.

---

## Already Complete (reference only — do NOT re-implement)

- simse-ui-core: input state machine, command registry (34 cmds), settings schemas, diff, file mentions, image input, tool types, output item types, conversation state, permission modes, agentic loop types, skill types
- simse-tui: app model (36 msgs), event loop, banner widget, output rendering
- simse-bridge: JSON-RPC client, protocol types, ACP client (connect/generate/stream/embed/permissions), config loading (8 files + agents/skills/SIMSE.md + precedence), session store (create/append/load/list/get/rename/remove/latest), JSON I/O (read/write/append/read_lines)

---

## Task 0: Storage Backend

**Crate:** `simse-bridge`

**Files:**
- Create: `simse-bridge/src/storage.rs`
- Modify: `simse-bridge/src/lib.rs` — add `pub mod storage;`
- Modify: `simse-bridge/Cargo.toml` — ensure `flate2` dependency exists
- Test: inline `#[cfg(test)] mod tests`

**TS reference:** `simse-code/storage.ts`

**Step 1: Write failing tests**

```rust
// simse-bridge/src/storage.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn load_missing_file_returns_empty() {
        let dir = TempDir::new().unwrap();
        let backend = FileStorageBackend::new(dir.path().join("nope.dat"), Default::default());
        let data = backend.load().await.unwrap();
        assert!(data.is_empty());
    }

    #[tokio::test]
    async fn roundtrip_save_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("store.dat");
        let backend = FileStorageBackend::new(path, Default::default());
        let mut data = HashMap::new();
        data.insert("key1".into(), b"value1".to_vec());
        data.insert("key2".into(), b"value2".to_vec());
        backend.save(&data).await.unwrap();
        let loaded = backend.load().await.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.get("key1").unwrap(), b"value1");
    }

    #[tokio::test]
    async fn atomic_write_survives_crash() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("atomic.dat");
        let backend = FileStorageBackend::new(path.clone(), StorageOptions { atomic_write: true, ..Default::default() });
        let mut data = HashMap::new();
        data.insert("k".into(), b"v".to_vec());
        backend.save(&data).await.unwrap();
        // tmp file should not remain
        assert!(!path.with_extension("dat.tmp").exists());
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let mut data = HashMap::new();
        data.insert("hello".into(), b"world".to_vec());
        data.insert("num".into(), vec![1, 2, 3, 4]);
        let bytes = serialize(&data);
        let restored = deserialize(&bytes).unwrap();
        assert_eq!(restored, data);
    }

    #[test]
    fn deserialize_detects_bad_magic() {
        let bad = b"BAAD\x00\x01\x00\x00\x00\x00";
        assert!(deserialize(bad).is_err());
    }
}
```

**Step 2: Run tests — expect FAIL (types don't exist yet)**

Run: `cd simse-bridge && cargo test storage`

**Step 3: Implement storage backend**

Binary format: MAGIC (`SIMK`, 4 bytes) + version (u16 BE) + count (u32 BE) + entries (key_len u32 BE + key bytes + val_len u32 BE + val bytes). Gzip on save (flate2, level 6). Auto-detect gzip on load (magic bytes 0x1f 0x8b). Atomic writes via tmp+rename.

Key types:
- `StorageOptions { atomic_write: bool, compression_level: u32 }`
- `FileStorageBackend { path, options }`
- Methods: `new()`, `async load() -> Result<HashMap<String, Vec<u8>>>`, `async save(&HashMap) -> Result<()>`, `async close() -> Result<()>`
- Functions: `fn serialize(data) -> Vec<u8>`, `fn deserialize(bytes) -> Result<HashMap>`

**Step 4: Run tests — expect PASS**

Run: `cd simse-bridge && cargo test storage`

**Step 5: Commit**

```bash
git add simse-bridge/src/storage.rs simse-bridge/src/lib.rs
git commit -m "feat(simse-bridge): add file storage backend with binary format and gzip"
```

---

## Task 1: Conversation Buffer

**Crate:** `simse-ui-core`

**Files:**
- Modify: `simse-ui-core/src/state/conversation.rs` — extend with full conversation buffer
- Test: inline `#[cfg(test)] mod tests`

**TS reference:** `simse-code/conversation.ts`

The existing `conversation.rs` has types. This task implements the full buffer: `addUser`, `addAssistant`, `addToolResult`, `setSystemPrompt`, `loadMessages`, `toMessages`, `serialize`, `clear`, `compact`, `messageCount`, `estimatedChars`, `needsCompaction`.

Key behavior:
- `serialize()`: format each message as `[Role]\ncontent` joined by `\n\n`
- `compact(summary)`: replace all messages with one user message `[Conversation summary]\n{summary}`
- `needsCompaction`: returns true when `estimatedChars > auto_compact_chars` (default 100,000)
- `maxMessages`: optional cap — trims oldest non-system messages when exceeded
- `loadMessages`: clears and replays, extracting system messages to set system_prompt

**Step 1-5:** Write tests → verify fail → implement → verify pass → commit.

---

## Task 2: Permission Manager

**Crate:** `simse-ui-core`

**Files:**
- Create: `simse-ui-core/src/state/permission_manager.rs`
- Modify: `simse-ui-core/src/state/mod.rs` — add `pub mod permission_manager;`
- Test: inline tests

**TS reference:** `simse-code/permission-manager.ts`

Types:
- `PermissionMode`: `Default`, `AcceptEdits`, `Plan`, `DontAsk`
- `PermissionDecision`: `Allow`, `Deny`, `Ask`
- `PermissionRule { tool: String, pattern: Option<String>, policy: PermissionDecision }`
- `PermissionManager` struct with `mode`, `rules: Vec<PermissionRule>`, `config_path`

Methods:
- `check(tool_name, args) -> PermissionDecision` — rules first (glob matching), then mode-based
- `get_mode()`, `set_mode()`, `cycle_mode()` (Default → AcceptEdits → Plan → DontAsk → Default)
- `add_rule()`, `remove_rule()`, `get_rules()`
- `save(path)`, `load(path)` — JSON file `{ mode, rules }`
- `format_mode() -> String`

Tool categories (static sets):
- `WRITE_TOOLS`: vfs_write, vfs_delete, vfs_rename, vfs_mkdir, file_write, file_edit, file_create
- `BASH_TOOLS`: bash, shell, exec, execute, run_command
- `READ_ONLY_TOOLS`: vfs_read, vfs_list, vfs_stat, vfs_search, vfs_diff, file_read, glob, grep, library_search, library_list, task_list, task_get

Glob matching: `*` → `.*`, `?` → `.` (simple regex conversion).

---

## Task 3: Keybinding Manager

**Crate:** `simse-ui-core`

**Files:**
- Create: `simse-ui-core/src/input/keybindings.rs`
- Modify: `simse-ui-core/src/input/mod.rs` — add `pub mod keybindings;`
- Test: inline tests

**TS reference:** `simse-code/keybindings.ts`

Types:
- `KeyCombo { name: String, ctrl: bool, shift: bool, meta: bool }`
- `KeybindingEntry { combo: KeyCombo, label: String, id: usize }`
- `KeybindingRegistry` struct with `entries: Vec<KeybindingEntry>`, `double_tap_ms: u64`

Methods:
- `register(combo, label) -> usize` (returns entry ID)
- `unregister(id)`
- `matches(combo, event) -> bool`
- `find_match(event) -> Option<&KeybindingEntry>`
- `list() -> &[KeybindingEntry]`
- `combo_to_string(combo) -> String` (e.g. "Ctrl+C")

Note: The actual handler execution happens in simse-tui's event loop, not here. This crate only stores the registry and does matching. Double-tap detection is state tracked in simse-tui.

---

## Task 4: Non-Interactive Mode

**Crate:** `simse-ui-core`

**Files:**
- Create: `simse-ui-core/src/cli.rs`
- Modify: `simse-ui-core/src/lib.rs` — add `pub mod cli;`
- Test: inline tests

**TS reference:** `simse-code/non-interactive.ts`

Types:
- `NonInteractiveArgs { prompt: String, format: OutputFormat, server_name: Option<String>, agent_id: Option<String> }`
- `OutputFormat`: `Text`, `Json`
- `NonInteractiveResult { output: String, model: String, duration_ms: u64, exit_code: i32 }`

Functions:
- `parse_non_interactive_args(args: &[String]) -> Option<NonInteractiveArgs>` — parse `-p`, `--prompt`, `--format`, `--server`, `--agent`
- `format_non_interactive_result(result, format) -> String` — text passthrough or JSON with tabs
- `is_non_interactive(args: &[String]) -> bool`

---

## Task 5: Tool Registry

**Crate:** `simse-ui-core` (types + format) + `simse-bridge` (execution)

**Files:**
- Modify: `simse-ui-core/src/tools/mod.rs` — add full tool types
- Create: `simse-bridge/src/tool_registry.rs`
- Modify: `simse-bridge/src/lib.rs` — add `pub mod tool_registry;`
- Test: inline tests in both

**TS reference:** `simse-code/tool-registry.ts`

### simse-ui-core types (already partial — extend):
- `ToolParameter { type_name: String, description: String, required: bool }`
- `ToolDefinition { name: String, description: String, parameters: BTreeMap<String, ToolParameter> }`
- `ToolCallRequest { id: String, name: String, arguments: serde_json::Value }`
- `ToolCallResult { id: String, name: String, output: String, is_error: bool, diff: Option<String> }`
- `fn format_for_system_prompt(tools: &[ToolDefinition]) -> String` — XML-style `<tool_use>` format

### simse-bridge execution:
- `ToolRegistry` struct with `tools: HashMap<String, RegisteredTool>`
- `RegisteredTool { definition: ToolDefinition, handler: Box<dyn ToolHandler> }`
- `trait ToolHandler: Send + Sync { async fn execute(&self, args: Value) -> Result<ToolHandlerOutput> }`
- Built-in tool registration (library_search, library_shelve, vfs_read, vfs_write, vfs_list, vfs_tree)
- MCP tool discovery (iterate connected servers, list tools, register as `mcp:{server}/{name}`)
- `execute(call) -> ToolCallResult` — lookup by name, call handler, catch errors
- `discover()` — clear + register builtins + discover MCP tools
- Output truncation: `max_output_chars` (default 50,000), per-tool override

---

## Task 6: Tool Call Parser

**Crate:** `simse-ui-core`

**Files:**
- Create: `simse-ui-core/src/tools/parser.rs`
- Modify: `simse-ui-core/src/tools/mod.rs` — add `pub mod parser;`
- Test: inline tests

**TS reference:** `simse-code/loop.ts` (parseToolCalls function)

Function: `parse_tool_calls(response: &str) -> ParsedResponse`
- `ParsedResponse { text: String, tool_calls: Vec<ToolCallRequest> }`
- Match `<tool_use>...</tool_use>` blocks with regex
- Parse JSON inside each block: `{ id, name, arguments }`
- Auto-generate IDs if missing: `call_1`, `call_2`, etc.
- Strip matched blocks from text
- Skip malformed JSON silently

---

## Task 7: Agentic Loop

**Crate:** `simse-bridge`

**Files:**
- Create: `simse-bridge/src/agentic_loop.rs`
- Modify: `simse-bridge/src/lib.rs` — add `pub mod agentic_loop;`
- Test: inline tests

**TS reference:** `simse-code/loop.ts`

Types:
- `AgenticLoopOptions { acp_client, tool_registry, conversation, max_turns (default 10), server_name, agent_id, system_prompt, signal, agent_manages_tools }`
- `LoopTurn { turn: usize, turn_type: TurnType, text: Option<String>, tool_calls: Vec<ToolCallRequest>, tool_results: Vec<ToolCallResult> }`
- `AgenticLoopResult { final_text: String, turns: Vec<LoopTurn>, total_turns: usize, hit_turn_limit: bool, aborted: bool }`
- `LoopCallbacks` — trait with optional methods: `on_stream_delta`, `on_stream_start`, `on_tool_call_start`, `on_tool_call_end`, `on_turn_complete`, `on_error`, `on_permission_check`, `on_agent_tool_call`, `on_agent_tool_call_update`, `on_doom_loop`, `on_compaction`, `on_token_usage`

Core loop logic:
1. Add user message to conversation
2. Build system prompt (tool defs + user prompt, skip if agent_manages_tools)
3. For each turn (1..=max_turns):
   a. Check abort signal
   b. Auto-compact if `conversation.needs_compaction && turn > 1`
   c. Serialize conversation → stream from ACP
   d. Parse tool calls (skip if agent_manages_tools)
   e. If no tool calls → final response, return
   f. Doom loop detection: track `tool_key` (name+args hash), warn after 3 identical
   g. Execute tool calls (check permission first)
   h. Add tool results to conversation
4. If loop exits without text response → hit_turn_limit = true

---

## Task 8: Markdown Renderer

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/markdown.rs`
- Modify: `simse-tui/Cargo.toml` — add `pulldown-cmark` dependency
- Test: inline tests

**TS reference:** `simse-code/components/chat/markdown.tsx`

Function: `render_markdown(text: &str, width: u16) -> Vec<Line<'static>>`

Block-level:
- H1: cyan bold, H2: bold, H3: underline
- Fenced code blocks with language detection → syntax highlighting
- Bullet lists (• prefix with indent), numbered lists, task lists (☐/☑)
- Blockquotes (dim `│` prefix)
- Tables (simple grid)
- Horizontal rules (dim `─` repeated)

Inline:
- `` `code` `` → cyan
- `**bold**` → bold modifier
- `*italic*` → italic modifier
- `~~strikethrough~~` → dim
- `[links](url)` → blue underline

Syntax highlighting (via simple keyword/regex matching):
- JS/TS: keywords (const, let, function, return, if, else, for, while, async, await) cyan, strings green, numbers yellow, comments dim
- Python: keywords (def, class, import, from, return, if, else, for, while, with, as) cyan
- Bash: keywords (if, then, else, fi, for, do, done, while, case, esac) cyan
- JSON: keys cyan, string values green, numbers yellow, booleans/null magenta
- Rust: keywords (fn, let, mut, pub, struct, enum, impl, use, mod, match, if, else, for, while, loop, return, async, await) cyan

Use `pulldown-cmark` for parsing, then convert events to ratatui `Span`s.

---

## Task 9: Thinking Spinner

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/spinner.rs`
- Test: inline tests

**TS reference:** `simse-code/components/shared/spinner.tsx`

Struct: `ThinkingSpinner { frames, frame_idx, verb, started_at, token_count, server_name }`

- Frame characters: `['·', '✲', '*', '✶', '✻', '✽']` cycling at ~120ms
- Random verb from 25 options: Thinking, Pondering, Brewing, Cooking, Dreaming, Weaving, Crafting, Musing, Conjuring, Scheming, Plotting, Imagining, Composing, Mulling, Ruminating, Contemplating, Deliberating, Considering, Reflecting, Meditating, Analyzing, Processing, Computing, Reasoning, Evaluating
- Display: `{frame} {verb}... {elapsed}s {tokens} tokens ({server})`
- Methods: `new(server_name)`, `tick() -> bool` (returns true if frame changed), `render(area, frame)`, `set_token_count(n)`, `elapsed() -> Duration`

---

## Task 10: Permission Dialog

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/dialogs/permission.rs`
- Create: `simse-tui/src/dialogs/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/components/input/permission-dialog.tsx`

Centered overlay popup:
- Header: `⚠ simse wants to run {tool_name}({primary_arg})`
- Body: shows full args formatted
- Keys: `y` (allow once), `n` (deny), `a` (allow always)
- Styling: bordered box, yellow warning color, key hints at bottom

Function: `render_permission_dialog(frame, area, request: &PermissionRequest)`
- `PermissionRequest` already exists in simse-ui-core

---

## Task 11: Confirm Dialog

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/dialogs/confirm.rs`
- Modify: `simse-tui/src/dialogs/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/components/input/confirm-dialog.tsx`

Centered overlay popup:
- Header: message text (e.g. "Delete all global configs?")
- Two options: "No, cancel" (default selected) / "Yes, proceed"
- Up/Down navigation between options
- "Yes" requires typing "yes" in text input field before confirming
- Escape always cancels

State: `ConfirmDialogState { message: String, selected: usize, yes_input: String }`

---

## Task 12: Settings Explorer

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/overlays/settings.rs`
- Create: `simse-tui/src/overlays/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/components/input/settings-explorer.tsx`

Multi-level overlay:
1. File list: show config files (config.json, acp.json, mcp.json, embed.json, memory.json, summarize.json, settings.json, prompts.json)
2. Field list: browse fields of selected file with current values
3. Edit mode: type-specific editors
   - Text: TextInput
   - Number: text input with presets
   - Boolean: toggle
   - Select: dropdown

State: `SettingsExplorerState { level: Level, selected_file, selected_field, edit_value, saved_indicator }`

Navigation: ↑↓ fields, ↵ edit, ← back, Esc dismiss

Save indicator: "Saved ✓" shown for 1.5s after save.

---

## Task 13: Librarian Explorer

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/overlays/librarian.rs`
- Modify: `simse-tui/src/overlays/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/components/input/librarian-explorer.tsx`

Two modes:
1. List mode: browse librarians, "+ New librarian..." at bottom
2. Detail mode: edit name, description, permissions, topic preferences

Field validation:
- Names: kebab-case only
- Permissions/topics: comma-separated arrays

Delete with confirmation (uses confirm dialog).
Auto-save on field edit.

State: `LibrarianExplorerState { mode: ListOrDetail, librarians, selected, editing_field, edit_value }`

---

## Task 14: Setup Selector & Ollama Wizard

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/overlays/setup.rs`
- Create: `simse-tui/src/overlays/ollama_wizard.rs`
- Modify: `simse-tui/src/overlays/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/components/input/setup-selector.tsx`, `ollama-wizard.tsx`

### Setup Selector
Preset list:
- Claude Code — writes acp.json for Claude Code bridge
- Ollama — opens Ollama wizard
- Copilot — writes acp.json for GitHub Copilot bridge
- Custom — text input for command + args

Navigation: ↑↓, Enter to select, Esc to cancel.

### Ollama Wizard
Multi-step flow:
1. URL input (default: http://localhost:11434)
2. Model selection (fetches from Ollama API: GET /api/tags)
3. Writes acp.json on completion

### Onboarding
Detects no ACP config on startup → triggers setup selector automatically.

---

## Task 15: Command Autocomplete

**Crate:** `simse-tui`

**Files:**
- Modify: `simse-tui/src/app.rs` — extend PromptMode and input rendering
- Test: inline tests

**TS reference:** `simse-code/components/input/prompt-input.tsx` (autocomplete section)

Behavior:
- `/` prefix activates autocomplete mode
- Filter commands by typed prefix
- Show up to 8 matching commands with descriptions in popup above input
- Ghost text for single match (accepted with → key)
- Tab/Enter to accept selected command
- Escape to cancel autocomplete
- Typing narrows matches in real-time
- ↑↓ to navigate matches

Integrate with existing `PromptMode::Autocomplete { selected, matches }`.

---

## Task 16: @-Mention Autocomplete

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/autocomplete.rs`
- Modify: `simse-tui/src/app.rs` — add AtMention prompt mode
- Test: inline tests

**TS reference:** `simse-code/file-mentions.ts`, `components/input/prompt-input.tsx`

Behavior:
- `@` prefix triggers file path walking
- Uses `std::fs::read_dir` to list directory entries
- Directory traversal: paths ending in `/` keep mode active for subdirectory browsing
- Volume ID completion: 8-char hex prefix → match against library volume IDs
- VFS path completion: `vfs://` prefix
- Excludes: node_modules, .git, dist, build, target, .cache, __pycache__
- Show up to 8 matching paths in popup above input
- Tab/Enter to accept, Escape to cancel

State: `AtMentionState { prefix: String, entries: Vec<String>, selected: usize }`

---

## Task 17: Library Feature Commands

**Crate:** `simse-tui` (dispatch) + `simse-bridge` (execution)

**Files:**
- Create: `simse-tui/src/commands/library.rs`
- Create: `simse-tui/src/commands/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/features/library/commands.ts`

Implement all library slash commands in the TUI dispatch:

| Command | Implementation |
|---------|----------------|
| `/add <topic> <text>` | Call bridge → embed text → store in library with topic |
| `/search <query>` | Call bridge → vector similarity search → display results with scores |
| `/recommend <query>` | Call bridge → weighted search → display |
| `/topics` | Call bridge → list all topics with volume counts |
| `/volumes [topic]` | Call bridge → list volumes, optionally filtered |
| `/get <id>` | Call bridge → display full volume by ID |
| `/delete <id>` | Show confirm dialog → call bridge → remove volume |
| `/librarians` | Open librarian explorer overlay |

Each command produces `OutputItem::CommandResult` or `OutputItem::Error`.

---

## Task 18: Session Feature Commands

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/commands/session.rs`
- Modify: `simse-tui/src/commands/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/features/session/commands.ts`

| Command | Implementation |
|---------|----------------|
| `/sessions` | List saved sessions with ID, title, date, message count |
| `/resume <id>` | Load session messages into conversation, display info |
| `/rename <title>` | Update current session title |
| `/server` | Show active ACP server name |
| `/model` | Show active model name |
| `/mcp` | Show MCP connection status |
| `/acp` | Show ACP connection status |

---

## Task 19: Config Feature Commands

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/commands/config.rs`
- Modify: `simse-tui/src/commands/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/features/config/commands.ts`, `init.ts`, `setup.ts`, `reset.ts`

| Command | Implementation |
|---------|----------------|
| `/setup [preset]` | Open setup selector overlay (or apply preset directly if arg given) |
| `/init [--force]` | Scan cwd files, generate SIMSE.md via ACP AI call |
| `/config` | Show current resolved config (server, agent, model, library, MCP) |
| `/settings` | Open settings explorer overlay |
| `/factory-reset` | Confirm dialog → delete all global configs in data_dir |
| `/factory-reset-project` | Confirm dialog → delete `.simse/` dir and `SIMSE.md` |

---

## Task 20: File Feature Commands

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/commands/files.rs`
- Modify: `simse-tui/src/commands/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/features/files/commands.ts`

| Command | Implementation |
|---------|----------------|
| `/files [path]` | List VFS directory contents (type + name per entry) |
| `/save [path]` | Write VFS files to disk (with confirmation) |
| `/validate [path]` | Run content validators (JSON syntax, trailing whitespace) |
| `/discard [path]` | Revert VFS changes |
| `/diff [path]` | Show unified diffs using diff renderer |

---

## Task 21: AI Feature Commands

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/commands/ai.rs`
- Modify: `simse-tui/src/commands/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/features/ai/commands.ts`

| Command | Implementation |
|---------|----------------|
| `/chain <name> [args]` | Look up named prompt chain from config → execute steps sequentially |
| `/prompts` | List available prompt templates with descriptions |

---

## Task 22: Tool Feature Commands

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/commands/tools.rs`
- Modify: `simse-tui/src/commands/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/features/tools/commands.ts`

| Command | Implementation |
|---------|----------------|
| `/tools` | List available tools with name + description |
| `/agents` | List available ACP agents from config |
| `/skills` | List available skills with name + description |

---

## Task 23: Meta Feature Commands

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/commands/meta.rs`
- Modify: `simse-tui/src/commands/mod.rs`
- Test: inline tests

**TS reference:** `simse-code/features/meta/commands.ts`, `components.tsx`

These are already partially implemented in `app.rs` dispatch. Complete the remaining:

| Command | Implementation |
|---------|----------------|
| `/help` | Already works — ensure all categories show |
| `/clear` | Already works |
| `/exit`, `/quit`, `/q` | Already works |
| `/verbose [on\|off]` | Already works |
| `/plan [on\|off]` | Already works |
| `/context` | Already works — extend with context grid display |
| `/compact` | Trigger auto-compaction via bridge |
| `/shortcuts` | Show keyboard shortcuts overlay |

The context grid should show: server name, model, agent, permission mode, tokens, context %, library status.

---

## Task 24: Full Command Dispatch Integration

**Crate:** `simse-tui`

**Files:**
- Modify: `simse-tui/src/app.rs` — replace stub `dispatch_command` with full implementation

Currently `dispatch_command` handles only help/clear/exit/verbose/plan/context/compact and shows "not yet implemented" for everything else. This task wires all the command modules (Tasks 17-23) into the dispatch, replacing the stubs.

Each command module exposes a function like:
```rust
pub fn execute(app: &mut App, bridge: &Bridge, arg: &str) -> Vec<OutputItem>
```

The dispatch function matches the command name, calls the appropriate module, and appends results to `app.output`.

---

## Task 25: TUI Event Loop Integration

**Crate:** `simse-tui`

**Files:**
- Modify: `simse-tui/src/main.rs` — integrate bridge, agentic loop, async commands

This task wires the simse-tui main event loop to the simse-bridge:

1. **Startup:** Load config → spawn ACP client → create session → detect onboarding
2. **User submit (non-command):** Start agentic loop → stream events → update App model
3. **User submit (command):** Dispatch to command module → some commands are async (call bridge)
4. **Permission flow:** When agentic loop requests permission → show permission dialog → forward response
5. **Abort:** Escape during loop → cancel via tokio CancellationToken
6. **Session persistence:** Append messages to session store after each user/assistant turn
7. **Spinner:** Show thinking spinner during streaming, update with token counts
8. **Resize:** Handle terminal resize events
9. **CLI args:** Parse clap args (--continue, --resume, -p, --format, --server, --agent)
10. **Non-interactive:** If `-p` flag → run single generation → print result → exit

---

## Task 26: CLI Argument Parsing

**Crate:** `simse-tui`

**Files:**
- Modify: `simse-tui/src/main.rs` — add clap argument parsing

Add clap derive for CLI args:
```rust
#[derive(Parser)]
#[command(name = "simse", about = "SimSE - AI workflow orchestration")]
struct Cli {
    /// Non-interactive prompt
    #[arg(short = 'p', long)]
    prompt: Option<String>,
    /// Output format (text or json)
    #[arg(long, default_value = "text")]
    format: String,
    /// Continue last session for current directory
    #[arg(long)]
    continue_session: bool,
    /// Resume a specific session by ID prefix
    #[arg(long)]
    resume: Option<String>,
    /// Override ACP server
    #[arg(long)]
    server: Option<String>,
    /// Override agent ID
    #[arg(long)]
    agent: Option<String>,
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}
```

---

## Task 27: Onboarding Flow

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/src/onboarding.rs`
- Modify: `simse-tui/src/main.rs` — check for ACP config on startup

**TS reference:** `simse-code/features/config/onboarding.ts`

On startup, if no ACP servers are configured:
1. Show welcome banner with "No AI server configured"
2. Automatically open setup selector overlay
3. After setup completes, reload config and connect

---

## Task 28: Status Bar Enhancement

**Crate:** `simse-tui`

**Files:**
- Modify: `simse-tui/src/app.rs` — enhance `render_status_line`

Extend the status bar to show:
- Permission mode with (shift+tab) hint
- Current server name
- Current model name
- "esc to interrupt" when loop is active
- Plan mode indicator
- Verbose indicator
- Token count and context %
- "? for shortcuts" hint

All separated by `·` dots, right-aligned stats.

---

## Task 29: Tool Call Box Rendering

**Crate:** `simse-tui`

**Files:**
- Modify: `simse-tui/src/output.rs` — enhance tool call rendering

**TS reference:** `simse-code/components/chat/tool-call-box.tsx`

Enhanced tool call display:
- Completed tools: `✓ tool_name(primary_arg) {summary} [{duration}ms]` in green
- Failed tools: `✗ tool_name(primary_arg) {error}` in red
- Active tools: `⋯ tool_name(primary_arg)` in yellow with spinner
- Diff display: if tool has diff, show unified diff below with +/- coloring
- Verbose mode: show full args JSON

---

## Task 30: Error Box Rendering

**Crate:** `simse-tui`

**Files:**
- Modify: `simse-tui/src/output.rs` — add error box

**TS reference:** `simse-code/components/shared/error-box.tsx`

Display errors in a red-bordered box with "Error" title and the message text.

---

## Task 31: Integration Tests

**Crate:** `simse-tui`

**Files:**
- Create: `simse-tui/tests/integration.rs`

End-to-end tests covering:
1. App startup → banner visible
2. Submit text → appears in output
3. Command dispatch → all commands produce correct output types
4. History navigation → up/down cycles through history
5. Ctrl+C double-press → quits
6. Escape during overlay → dismisses
7. Permission mode cycling
8. Non-interactive mode parsing

---

## Task 32: Build System Integration

**Files:**
- Modify: `Cargo.toml` (workspace root) — ensure simse-tui is in workspace members
- Modify: `package.json` (root) — add build:tui script

Add workspace member and build script:
```toml
# Cargo.toml workspace members
members = ["simse-tui", "simse-ui-core", "simse-bridge", ...]
```

```json
// package.json scripts
"build:tui": "cd simse-tui && cargo build --release"
```

Update CLAUDE.md to document the new build commands.

---

## Task 33: Final Verification & Cleanup

**Files:**
- Run all tests across all three crates
- Verify feature parity against simse-code feature list
- Update CLAUDE.md with new crate documentation

**Verification checklist:**
- [ ] All 34 commands work
- [ ] Agentic loop streams correctly
- [ ] Permission dialog shows and responds
- [ ] Settings explorer navigates and saves
- [ ] Librarian explorer CRUD works
- [ ] Setup/onboarding flow works
- [ ] Session resume/continue works
- [ ] Non-interactive mode works
- [ ] Markdown renders correctly
- [ ] Spinner animates
- [ ] Autocomplete works for both / and @
- [ ] Diff display renders
- [ ] Storage backend persists correctly
- [ ] All tests pass

---

## Task 34: Delete simse-code

**Files:**
- Delete: `simse-code/` directory (entire tree)
- Modify: `CLAUDE.md` — remove simse-code references, add simse-tui/simse-ui-core/simse-bridge
- Modify: `package.json` — remove simse-code scripts

**Only after Task 33 verification passes.**

```bash
git rm -r simse-code/
git commit -m "chore: remove simse-code — migration to Rust TUI complete"
```

---

## Dependency Graph

```
Task 0 (Storage) ─────────────────────────────────┐
Task 1 (Conversation) ────────────────────────────┤
Task 2 (Permission Manager) ─────────────────────┤
Task 3 (Keybinding Manager) ─────────────────────┤
Task 4 (Non-Interactive) ────────────────────────┤
                                                    ├─→ Task 5 (Tool Registry) ──→ Task 6 (Parser) ──→ Task 7 (Agentic Loop)
                                                    │
Task 8 (Markdown) ───────────────────────────────┤
Task 9 (Spinner) ────────────────────────────────┤
Task 10 (Permission Dialog) ─────────────────────┤
Task 11 (Confirm Dialog) ────────────────────────┤
                                                    ├─→ Task 12 (Settings Explorer)
                                                    ├─→ Task 13 (Librarian Explorer)
                                                    ├─→ Task 14 (Setup/Ollama/Onboarding)
                                                    ├─→ Task 15 (Command Autocomplete)
                                                    ├─→ Task 16 (@-Mention Autocomplete)
                                                    │
Task 5-7 + Task 8-16 ──→ Task 17-23 (Feature Commands) ──→ Task 24 (Dispatch Integration)
Task 24 + Task 25 (Event Loop) + Task 26 (CLI Args) + Task 27 (Onboarding) ──→ Task 28-30 (UI Polish)
Task 28-30 ──→ Task 31 (Integration Tests) ──→ Task 32 (Build System) ──→ Task 33 (Verification) ──→ Task 34 (Delete simse-code)
```

## Independent Tasks (can run in parallel)

- Tasks 0-4 are all independent
- Tasks 8-16 are all independent (after 0-4)
- Tasks 17-23 are all independent (after 5-7 and relevant UI tasks)
