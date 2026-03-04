# simse-code Migration Design

## Goal

Complete the migration from simse-code (TypeScript/Ink) to three Rust crates, enabling full deletion of simse-code with zero regressions. Every command, feature, and UX behavior must work in the Rust stack — including features that are currently stubs in TS.

## Architecture

Three Rust crates with clear separation:

- **simse-ui-core** — Platform-agnostic library. State machines, data models, business logic. No I/O, no rendering.
- **simse-tui** — Terminal UI via ratatui. Elm Architecture (Model/Update/View). All rendering, keyboard handling, overlays, widgets.
- **simse-bridge** — Async I/O layer. ACP client, subprocess management, config loading, session persistence, storage backend.

## Existing Design Reference

The full gap analysis is documented in `simse-code/docs/plans/2026-03-01-complete-migration-design.md`. It identifies 31 work items across 7 tiers:

| Tier | Area | Items |
|------|------|-------|
| 1 | Core Infrastructure | ACP Client, Config Loading, Session Store |
| 2 | Library System | Storage Backend, Library Core, Embedding, Librarian |
| 3 | Tool System | Tool Registry, VFS, Task List, MCP Client |
| 4 | Agentic Loop | Core AI interaction engine |
| 5 | TUI Components | Markdown, Spinner, Permission Dialog, Confirm Dialog, Settings Explorer, Librarian Explorer, Setup/Onboarding, Command Autocomplete, @-Mention Autocomplete |
| 6 | Feature Commands | Library, Session, AI, File, Config, Tool commands |
| 7 | CLI Infrastructure | Non-Interactive Mode, Permission Manager, Keybinding Manager, JSON I/O |

## What's Already Built

### simse-ui-core (18 files, ~94 tests)
- Input state machine (14 ops) — Complete
- Command registry (34 commands) — Complete
- Settings schemas (6 files) — Complete
- Diff parsing & formatting — Complete
- @-mention parsing — Complete
- Image path detection — Complete
- Tool types & formatting — Complete
- Output item types — Complete
- Conversation state — Complete
- Permission modes & cycling — Complete
- Agentic loop types — Types only
- Skill types — Types only

### simse-tui (4 files, ~33 tests)
- App model + TEA (36 messages) — Complete
- Async event loop + key mapping — Complete
- Banner widget — Complete
- Output item rendering — Complete

### simse-bridge (7 files + tests, ~12 unit + 9 integration tests)
- JSON-RPC bridge client — Complete
- JSON-RPC protocol types — Complete
- ACP client (connect, generate, stream, embed, permissions) — Complete
- Config loading (all 8 config files, agents, skills, SIMSE.md, precedence) — Complete
- Session store types — Complete
- JSON I/O utilities — Complete

## Remaining Work

### Phase 5: Core Infrastructure (Tier 7 + Tier 1 gaps)
- Session store: JSONL persistence (save/load/list/rename/delete)
- Keybinding manager
- Permission manager (full: 4 modes, rules, persistence)

### Phase 6: Library System (Tier 2)
- Storage backend (binary format, gzip, atomic writes)
- Library core (vector search, cosine, cataloging, dedup, recommendation, preservation, shelf)
- Embedding provider (ONNX/candle native, TEI bridge, ACP-based)
- Librarian + Topic Catalog + Circulation Desk

### Phase 7: Tool System + Agentic Loop (Tier 3 + 4)
- Tool registry full implementation (discovery, execute, builtins, subagent tools)
- VFS (in-memory + disk-backed)
- Task list (CRUD, dependencies)
- MCP client
- Agentic loop (conversation → ACP → parse → execute → repeat)

### Phase 8: TUI Components (Tier 5)
- Markdown rendering with syntax highlighting
- Thinking spinner (animated)
- Permission dialog
- Confirm dialog
- Settings explorer
- Librarian explorer
- Setup & onboarding wizards
- Command autocomplete
- @-mention autocomplete

### Phase 9: Feature Commands + CLI (Tier 6 + 7)
- All 34 commands working (library, session, AI, file, config, tool)
- Non-interactive mode (-p flag)

### Phase 10: Integration & Cleanup
- Full integration testing
- E2E test equivalents
- Delete simse-code

## Tech Decisions

- **Embedding:** candle or ort (ONNX Runtime) for native Rust
- **MCP:** rmcp crate or minimal JSON-RPC client
- **Markdown:** pulldown-cmark + ratatui Spans
- **Config:** serde_json, custom frontmatter parser (already built)
- **Async:** tokio for all I/O, synchronous business logic in simse-ui-core
