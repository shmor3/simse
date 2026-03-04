# TUI Production Audit — Design Document

**Date:** 2026-03-04
**Scope:** simse-tui, simse-ui-core, simse-bridge

## Problem

The TUI migration created correct module structure and tests, but 26+ command handlers return placeholder strings ("Would call bridge to ...") instead of performing real operations. Additionally, 6 built-in tool handlers use `StubHandler` that returns dummy results, MCP tool discovery is empty, VFS @-mention completion returns nothing, and overlay screens (Librarians, Setup) aren't functional.

## Audit Findings

### CRITICAL — 26 placeholder command handlers

| Category | Commands | Stub Count |
|----------|----------|------------|
| Library | /add, /search, /recommend, /topics, /volumes, /get, /delete | 7 |
| Session | /sessions, /resume, /rename, /server, /model, /mcp, /acp | 7 |
| Files | /files, /save, /validate, /discard, /diff | 5 |
| Tools | /tools, /agents, /skills | 3 |
| Config | /init, /config, /factory-reset, /factory-reset-project | 4 |
| AI | /chain, /prompts | 2 |

All return `CommandOutput::Info("Would call bridge to ...")`.

### CRITICAL — 6 StubHandler tool handlers

`simse-bridge/src/tool_registry.rs` — `library_search`, `library_shelve`, `vfs_read`, `vfs_write`, `vfs_list`, `vfs_tree` all use `StubHandler` returning `"[{name}] Not yet connected — stub result."`.

### HIGH — Empty MCP tool discovery

`tool_registry.rs:399` — `discover_mcp_tools()` is a no-op with TODO comment.

### HIGH — VFS @-mention stub

`at_mention.rs:262` — `complete_vfs()` returns empty Vec.

### HIGH — Overlay screens not functional

`app.rs:552-564` — Librarians and Setup overlays show info messages instead of opening real screens.

## Architecture

### Async Command Pattern

Commands that need bridge data follow this flow:

```
User types /sessions
  → app.rs dispatch_command()
    → returns BridgeAction::ListSessions
      → app.rs sends AppMessage::BridgeRequest(action)
        → event_loop processes it async via TuiRuntime
          → TuiRuntime calls bridge method
            → bridge calls JSON-RPC to engine
              → result comes back as AppMessage::BridgeResult
                → app.rs update() renders the result
```

### New Types

```rust
// commands/mod.rs
pub enum CommandOutput {
    Success(String),
    Error(String),
    Info(String),
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    OpenOverlay(OverlayAction),
    BridgeRequest(BridgeAction),  // NEW
}

pub enum BridgeAction {
    // Library
    LibraryAdd { topic: String, text: String },
    LibrarySearch { query: String },
    LibraryRecommend { query: String },
    LibraryTopics,
    LibraryVolumes { topic: Option<String> },
    LibraryGet { id: String },
    LibraryDelete { id: String },
    // Session
    ListSessions,
    ResumeSession { id: String },
    RenameSession { title: String },
    SwitchServer { name: String },
    ShowServer,
    SwitchModel { name: String },
    ShowModel,
    McpStatus,
    McpRestart,
    AcpStatus,
    AcpRestart,
    // Files
    ListFiles { path: Option<String> },
    SaveFiles { path: Option<String> },
    ValidateFiles { path: Option<String> },
    DiscardFile { path: String },
    DiffFiles { path: Option<String> },
    // Tools
    ListTools { filter: Option<String> },
    ListAgents,
    ListSkills,
    // Config
    InitConfig { force: bool },
    ShowConfig { key: Option<String> },
    FactoryReset,
    FactoryResetProject,
    // AI
    RunChain { name: String, args: String },
    ListPrompts,
}
```

### TuiRuntime Extensions

New methods on `TuiRuntime` for each bridge action category:

- `list_sessions()` → calls `SessionStore::list()`
- `resume_session(id)` → loads session from store, replaces conversation
- `rename_session(title)` → updates session metadata
- `switch_server(name)` / `show_server()` → reconnect ACP or show current
- `switch_model(name)` / `show_model()` → update generation options
- `mcp_status()` / `mcp_restart()` → query/restart MCP connections
- `acp_status()` / `acp_restart()` → query/restart ACP connection
- `list_files()` / `save_files()` / etc. → JSON-RPC to simse-vfs engine
- `list_tools()` / `list_agents()` / `list_skills()` → query registries
- `search_library()` / `add_volume()` / etc. → JSON-RPC to simse-vector engine
- `show_config()` / `init_config()` / `factory_reset()` → config operations
- `run_chain()` / `list_prompts()` → chain execution and prompt listing

### Tool Handler Implementations

Replace `StubHandler` with real handlers that call JSON-RPC subprocess engines:

- `LibrarySearchHandler` → spawns/reuses simse-vector process, sends `store/search`
- `LibraryShelveHandler` → sends `store/add` to simse-vector
- `VfsReadHandler` → spawns/reuses simse-vfs process, sends `vfs/read`
- `VfsWriteHandler` → sends `vfs/write` to simse-vfs
- `VfsListHandler` → sends `vfs/list` to simse-vfs
- `VfsTreeHandler` → sends `vfs/tree` to simse-vfs

### MCP Discovery

Wire `discover_mcp_tools()` to:
1. Read MCP server configs from `LoadedConfig`
2. For each connected server, send `tools/list`
3. Register each tool as `mcp:{server}/{name}` with a handler that calls `tools/call`

### VFS @-Mention Completion

Wire `complete_vfs()` to call `TuiRuntime::vfs_list()` for path completions under `vfs://` prefix.

## Phases

1. **Core plumbing** — BridgeAction enum, AppMessage wiring, TuiRuntime dispatch
2. **Session & config commands** — Real session/config operations
3. **Library commands** — Real library operations via simse-vector JSON-RPC
4. **File commands** — Real VFS operations via simse-vfs JSON-RPC
5. **Tool/AI commands** — Tool listing, chain execution
6. **Tool handlers** — Replace StubHandlers with real JSON-RPC handlers
7. **MCP discovery** — Wire up MCP tool discovery
8. **VFS completion** — Wire up @-mention VFS completion
9. **Overlay screens** — Librarians and Setup overlays with real data
