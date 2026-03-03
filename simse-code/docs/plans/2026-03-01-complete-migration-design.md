# Complete simse-code → Rust Migration Design

## Goal

Achieve full feature parity between simse-code (TypeScript) and the Rust crates (simse-ui-core + simse-tui + simse-bridge), enabling complete removal of simse-code from the project. Every command, feature, and UX behavior must be present and working in Rust — including features that are currently placeholders in the TS version.

## Architecture

Three Rust crates with clear responsibilities:

- **simse-ui-core** — Platform-agnostic library. State machines, data models, business logic, library engine, VFS, tasks. No I/O, no rendering.
- **simse-tui** — Terminal UI via ratatui. Elm Architecture (Model/Update/View). All rendering, keyboard handling, overlays, widgets.
- **simse-bridge** — Async I/O layer. ACP client, MCP client, subprocess management, config loading, session persistence, storage backend.

## What Already Exists

| Area | Rust Module | Status |
|------|-------------|--------|
| Input state machine (14 ops) | `ui-core/input/state.rs` | Complete |
| Command registry (34 commands) | `ui-core/commands/registry.rs` | Complete |
| Settings schemas (6 files) | `ui-core/config/settings_schema.rs` | Complete |
| Diff parsing & formatting | `ui-core/diff.rs` | Complete |
| @-mention parsing | `ui-core/text/file_mentions.rs` | Complete |
| Image path detection | `ui-core/text/image_input.rs` | Complete |
| Tool types & formatting | `ui-core/tools/mod.rs` | Complete |
| Output item types | `ui-core/app.rs` | Complete |
| Conversation state | `ui-core/state/conversation.rs` | Complete |
| Permission modes & cycling | `ui-core/state/permissions.rs` | Complete |
| Agentic loop types | `ui-core/agentic_loop.rs` | Types only |
| Skill types | `ui-core/skills.rs` | Types only |
| App model + TEA (36 messages) | `tui/app.rs` | Complete |
| Async event loop + key mapping | `tui/main.rs` | Complete |
| Banner widget | `tui/banner.rs` | Complete |
| Output item rendering | `tui/output.rs` | Complete |
| JSON-RPC bridge client | `bridge/client.rs` | Complete |
| JSON-RPC protocol types | `bridge/protocol.rs` | Complete |

**Current test count: 150** (94 ui-core + 33 tui + 12 bridge unit + 9 bridge integration + 2 doc)

---

## Gaps: 31 Work Items in 7 Tiers

### Tier 1: Core Infrastructure

Everything depends on these. Must be built first.

#### 1. ACP Client (`simse-bridge`)

Full Agent Client Protocol implementation over JSON-RPC 2.0 / NDJSON stdio.

- Spawn ACP server subprocess (command + args from config)
- Protocol version 1 handshake (`initialize` → `initialized`)
- `session/new` → `session/prompt` → `session/update` notifications → response
- Streaming: parse `session/update` notifications for text deltas, tool calls, tool call updates
- Permission flow: `session/request_permission` with option selection
- `generate()`, `generate_stream()`, `embed()` methods
- Session modes: `session/set_config_option` for mode/model switching
- Timeout defaults: 60s per-request, 30s init handshake
- Connection health check (child process liveness)
- AbortSignal support for request cancellation

**TS reference:** simse core `acp-client.ts`, `acp-connection.ts`, `acp-results.ts`, `acp-adapters.ts`

#### 2. Config Loading (`simse-bridge`)

Load and merge hierarchical config from global + workspace directories.

- Global dir: `~/.simse/` (or `dirs::config_dir()`)
- Workspace dir: `.simse/` in cwd
- 8 config files: config.json, acp.json, mcp.json, embed.json, memory.json, summarize.json, settings.json, prompts.json
- Agent persona loading: `.simse/agents/*.md` with YAML frontmatter parsing
- Skill loading: `.simse/skills/{name}/SKILL.md` with allowed-tools CSV
- SIMSE.md workspace context (plain text)
- Precedence: CLI flags > workspace settings > global config
- MCP server skip if requiredEnv vars missing

**TS reference:** `config.ts`

#### 3. Session Store (`simse-bridge`)

Crash-safe JSONL session persistence.

- Session metadata: ID, title, message count, last updated, workDir
- Append-only JSONL writes (crash-safe)
- Operations: save, load, list, rename, delete
- Latest-session tracking by workDir
- CLI flags: `--continue` (resume latest for cwd), `--resume <id-prefix>`

**TS reference:** `session-store.ts`

### Tier 2: Library System

The knowledge base engine — the core value proposition.

#### 4. Storage Backend (`simse-bridge`)

File-based persistence with binary format.

- Binary format: MAGIC (SIMK) + version (u16) + count (u32) + key-value entries with length prefixes
- Gzip compression (flate2, level 6)
- Atomic writes via temp file + rename
- Auto-detect gzip by magic bytes (0x1f 0x8b)
- StorageBackend trait: `load() → HashMap`, `save(HashMap)`, `close()`

**TS reference:** `storage.ts`

#### 5. Library Core (`simse-ui-core`)

Vector search, indexing, and knowledge management.

- **Stacks:** File-backed storage with write-lock serialization, v2 compressed format (Float32 base64 embeddings + gzipped index)
- **Library:** `add()`, `search()` (cosine similarity), `recommend()`, `compendium()`, `find_duplicates()`
- **Cosine similarity:** Pure function, clamped to [-1, 1]
- **Cataloging:** TopicIndex (auto-extracts topics), MetadataIndex (O(1) lookups), MagnitudeCache
- **Deduplication:** `check_duplicate()` (single cosine check), `find_duplicate_groups()` (greedy clustering)
- **Recommendation:** WeightProfile with recency decay + frequency scoring + similarity
- **Preservation:** Float32↔base64 encode/decode (~75% size reduction), gzip wrappers
- **Shelf:** Agent-scoped library partitions with auto-tagging and filtered search

**TS reference:** simse core `library/` directory (library.ts, stacks.ts, cosine.ts, cataloging.ts, deduplication.ts, recommendation.ts, preservation.ts, shelf.ts)

#### 6. Embedding (`simse-bridge`)

Vector embedding generation.

- Local embedder: in-process via ONNX runtime or candle (Rust native, replaces @huggingface/transformers)
- TEI bridge: HTTP client to Text Embeddings Inference server
- ACP-based embedding: delegate to ACP server's embed() method
- EmbeddingProvider trait: `embed(texts: &[&str]) → Vec<Vec<f32>>`
- Default model: nomic-ai/nomic-embed-text-v1.5

**TS reference:** `local-embedder.ts`, `tei-bridge.ts`, `providers.ts`

#### 7. Librarian + Topic Catalog + Circulation Desk (`simse-ui-core`)

LLM-driven library curation.

- **Librarian:** extract, summarize, classify_topic, reorganize, optimize
- **TopicCatalog:** Hierarchical topic classification with Levenshtein fuzzy matching (0.85 threshold), aliases, resolve/relocate/merge
- **CirculationDesk:** Async background job queue for extraction, compendium, reorganization
- **PatronLearning:** Adaptive query tracking and weight adaptation

**TS reference:** `librarian.ts`, `topic-catalog.ts`, `circulation-desk.ts`, `patron-learning.ts`

### Tier 3: Tool System

#### 8. Tool Registry — Full Implementation (`simse-ui-core` + `simse-bridge`)

- Discovery from MCP servers (async, via bridge)
- Execute tool calls with output truncation (maxOutputChars default 50,000)
- Built-in tools: `library_search`, `library_shelve`, `vfs_read`, `vfs_write`, `vfs_list`, `task_create`, `task_update`, `task_list`, `task_get`
- Subagent tools: spawn sub-loops as tool calls with shelf-scoped library
- Format definitions for system prompt injection (`<tool_use>` XML)
- Per-tool maxOutputChars override

**TS reference:** `tool-registry.ts`, `builtin-tools.ts`, `subagent-tools.ts`

#### 9. VFS (Virtual Filesystem) (`simse-ui-core`)

In-memory file sandbox for agent operations.

- **VFS (in-memory):** createFile, readFile, writeFile, deleteFile, listDir, stat, exists
- **VFS Disk:** Disk-backed with snapshots, save-to-disk, discard-changes
- **Validators:** JSON syntax check, trailing whitespace detection
- Diff generation: track original vs modified content

**TS reference:** simse core `vfs/` (vfs.ts, vfs-disk.ts, validators.ts)

#### 10. Task List (`simse-ui-core`)

Structured task management for agent workflows.

- CRUD: create, read, update, delete tasks
- Dependencies: blocks/blockedBy relationships
- Status: pending → in_progress → completed
- TaskItem: id, subject, description, status, blocks, blockedBy, metadata

**TS reference:** simse core `tasks/` (task-list.ts)

#### 11. MCP Client (`simse-bridge`)

Model Context Protocol client for external tool servers.

- Connect via stdio or HTTP transport (using `@modelcontextprotocol/sdk` equivalent)
- Tool/resource/prompt discovery
- Tool execution with retry (exponential backoff)
- Logging: setLoggingLevel, onLoggingMessage
- List-changed notifications for dynamic discovery
- Resource templates, completions, roots

**TS reference:** `tools.ts`, simse core `mcp/` (mcp-client.ts)

### Tier 4: Agentic Loop

#### 12. Agentic Loop (`simse-tui` + `simse-bridge`)

The core AI interaction engine.

- Conversation → ACP stream → parse tool calls → execute → repeat
- Tool call parsing from `<tool_use>` XML blocks (regex: `/<tool_use>\s*([\s\S]*?)\s*<\/tool_use>/g`)
- Permission enforcement per tool call
- Doom loop detection: max 3 identical consecutive tool calls (same name + args)
- Auto-compaction when conversation exceeds threshold (structured 6-section prompt)
- maxTurns limit with hit_turn_limit flag
- AbortSignal between turns
- Callbacks: onStreamDelta, onStreamStart, onToolCallStart, onToolCallEnd, onTurnComplete, onError, onPermissionCheck, onDoomLoop, onCompaction, onTokenUsage
- Library context injection: search library before generate, inject results as system context
- Q&A pair auto-storage: store user prompt + assistant response as volume

**TS reference:** `loop.ts`, `hooks/use-agentic-loop.ts`

### Tier 5: TUI Components & UX

#### 13. Markdown Rendering (`simse-tui`)

Terminal markdown renderer with syntax highlighting.

- **Block-level:** H1 (cyan bold), H2 (bold), H3 (underline); fenced code blocks with lang detection; bullet/numbered/task lists; blockquotes (dim │ prefix); tables; horizontal rules
- **Inline:** \`code\` (cyan), **bold**, *italic*, ~~strikethrough~~ (dim), [links](url) (blue underline)
- **Syntax highlighting:** JS/TS (keywords/strings/numbers), Python, Bash, JSON (keys/values/literals)

**TS reference:** `components/chat/markdown.tsx`

#### 14. Thinking Spinner (`simse-tui`)

Animated processing indicator.

- Bouncing character animation: `· ✲ * ✶ ✻ ✽` (~120ms per frame)
- Random verb on each start: Thinking, Pondering, Brewing, Cooking, etc. (25 options)
- Display: elapsed time, token count, server name
- Shows when streaming with no text yet

**TS reference:** `components/shared/spinner.tsx`

#### 15. Permission Dialog (`simse-tui`)

Tool execution approval overlay.

- Display: `⚠ simse wants to run tool_name(primary_arg)`
- Keys: y (allow once), n (deny), a (allow always)
- Blocks input until resolved
- Integrates with permission manager rules

**TS reference:** `components/input/permission-dialog.tsx`

#### 16. Confirm Dialog (`simse-tui`)

High-stakes confirmation overlay.

- Two options: "No, cancel" (default) / "Yes, proceed"
- Up/Down navigation
- "Yes" requires typing "yes" in text input
- Escape always cancels
- Used for: factory reset, destructive operations

**TS reference:** `components/input/confirm-dialog.tsx`

#### 17. Settings Explorer (`simse-tui`)

Interactive config browser/editor overlay.

- List config files → select → browse fields → edit
- Field types: text (TextInput), number (with presets), boolean (toggle), select (dropdown)
- Live save with "Saved ✓" indicator (1.5s)
- Navigation: ↑↓ fields, ↵ edit, ← back, Esc dismiss

**TS reference:** `components/input/settings-explorer.tsx`

#### 18. Librarian Explorer (`simse-tui`)

Librarian CRUD overlay.

- List mode: browse librarians, "+ New librarian..."
- Detail mode: edit name, description, permissions, topic preferences
- Field validation: kebab-case names, comma-separated arrays
- Delete with confirmation
- Auto-save on field edit

**TS reference:** `components/input/librarian-explorer.tsx`

#### 19. Setup & Onboarding (`simse-tui`)

First-run configuration flow.

- **Setup selector:** Claude Code / Ollama / Copilot / Custom presets
- **Ollama wizard:** URL input → model selection
- **Onboarding:** Detects no ACP config → triggers setup flow
- Writes config files on completion

**TS reference:** `components/input/setup-selector.tsx`, `onboarding-wizard.tsx`, `ollama-wizard.tsx`

#### 20. Command Autocomplete (`simse-tui`)

Slash-command completion in prompt input.

- `/` prefix activates filtering
- Shows up to 8 matching commands with descriptions
- Ghost text for single match (accepted with →)
- Tab/Enter to accept, Escape to cancel
- Typing narrows matches in real-time

**TS reference:** `components/input/prompt-input.tsx`

#### 21. @-Mention Autocomplete (`simse-tui`)

File path completion in prompt input.

- `@` prefix triggers file path walking
- Directory traversal (paths ending in `/` keep mode active)
- Volume ID completion (8-char hex prefix)
- VFS path completion (`vfs://`)
- Excludes: node_modules, .git, dist, build, etc.

**TS reference:** `components/input/prompt-input.tsx`, `file-mentions.ts`

### Tier 6: Feature Commands (all working)

#### 22. Library Commands

| Command | Behavior |
|---------|----------|
| `/add <topic> <text>` | Embed text, store in topic |
| `/search <query>` | Vector similarity search, display results with scores |
| `/recommend <query>` | Weighted search (recency + frequency + similarity) |
| `/topics` | List all topics with volume counts |
| `/volumes [topic]` | List volumes, optionally filtered |
| `/get <id>` | Display full volume by ID |
| `/delete <id>` | Remove volume with confirmation |
| `/librarians` | Open librarian explorer overlay |

#### 23. Session Commands

| Command | Behavior |
|---------|----------|
| `/sessions` | List saved sessions with metadata |
| `/resume <id>` | Switch to previous session |
| `/rename <title>` | Rename current session |
| `/server` | Show active ACP server name |
| `/model` | Show active model name |
| `/mcp` | Show MCP connection status |
| `/acp` | Show ACP connection status |

#### 24. AI Commands

| Command | Behavior |
|---------|----------|
| `/chain <name> [args]` | Execute named prompt chain |
| `/prompts` | List available prompt templates |

#### 25. File Commands

| Command | Behavior |
|---------|----------|
| `/files [path]` | List VFS directory contents |
| `/save [path]` | Write VFS files to disk |
| `/validate [path]` | Run content validators |
| `/discard [path]` | Revert VFS changes |
| `/diff [path]` | Show unified diffs |

#### 26. Config Commands

| Command | Behavior |
|---------|----------|
| `/setup [preset]` | Configure ACP server (interactive or preset) |
| `/init [--force]` | Scan cwd, generate SIMSE.md via AI |
| `/config` | Show current configuration |
| `/settings` | Open settings explorer overlay |
| `/factory-reset` | Delete all global configs (with confirmation) |
| `/factory-reset-project` | Delete .simse/ and SIMSE.md (with confirmation) |

#### 27. Tool Commands

| Command | Behavior |
|---------|----------|
| `/tools` | List available tools with descriptions |
| `/agents` | List available ACP agents |
| `/skills` | List available skills |

### Tier 7: CLI Infrastructure

#### 28. Non-Interactive Mode

- `-p <prompt>` flag for single-shot generation
- `--format text|json` output format
- `--server`, `--agent` overrides
- Exit with result after completion

#### 29. Permission Manager — Full Implementation

- Four modes: default, acceptEdits, plan, dontAsk
- Tool categorization: WRITE_TOOLS, BASH_TOOLS, READ_ONLY_TOOLS
- Persistent rule storage (`permissions.json`)
- Glob pattern matching for args
- `check()`, `addRule()`, `removeRule()`, `save()`, `load()`

#### 30. Keybinding Manager

- Hotkey registry with handler callbacks
- Double-tap detection (Ctrl+C)
- Attach/detach from terminal input stream
- List all bindings for shortcuts overlay

#### 31. JSON I/O Utilities

- `read_json_file<T>(path)` → `Option<T>`
- `write_json_file(path, data)` — with tab indentation, auto-create dirs
- `append_json_line(path, data)` — JSONL append
- `read_json_lines<T>(path)` → `Vec<T>`

---

## Implementation Order

Build bottom-up: infrastructure → engine → tools → loop → UI → commands.

**Phase 5:** Tier 7 (JSON I/O, keybindings) + Tier 1 (ACP client, config, sessions)
**Phase 6:** Tier 2 (storage, library, embedding, librarian)
**Phase 7:** Tier 3 (tool registry, VFS, tasks, MCP) + Tier 4 (agentic loop)
**Phase 8:** Tier 5 (markdown, spinner, dialogs, autocomplete, overlays)
**Phase 9:** Tier 6 (all feature commands working) + Tier 7 remainder (non-interactive, permission manager)
**Phase 10:** Integration testing, remove simse-code

## Tech Decisions

- **Embedding:** Use `candle` or `ort` (ONNX Runtime) for native Rust embedding instead of HuggingFace JS
- **MCP:** Use `rmcp` crate or implement minimal JSON-RPC client (MCP SDK has no official Rust version yet)
- **Markdown:** Implement custom parser using `pulldown-cmark` for parsing + ratatui Spans for rendering
- **Config:** Use `serde_json` for all JSON config files, custom frontmatter parser for .md files
- **Async:** All I/O through tokio, all business logic synchronous in simse-ui-core
