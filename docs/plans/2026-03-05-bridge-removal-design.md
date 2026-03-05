# Bridge Removal Refactor Design

**Date:** 2026-03-05
**Status:** Approved

## Goal

Remove `simse-bridge` crate by having `simse-tui` and `simse-ui-core` depend directly on `simse-core`. Eliminate type duplication by using simse-core as the single source of truth for shared types.

## Architecture After Refactor

```
simse-tui
  ├── simse-core (direct library dependency)
  ├── simse-ui-core
  │   └── simse-core (for unified types)
  └── local modules:
      ├── session_store.rs (JSONL persistence)
      └── json_io.rs (file utilities)
```

## Key Decisions

1. **simse-core as direct library** — not JSON-RPC subprocess. In-process Rust types.
2. **CoreContext-centric** — use CoreContext as the backbone (wires EventBus, Logger, AppConfig, TaskList, HookSystem, SessionManager, ToolRegistry, Library, VFS).
3. **Unified types** — simse-ui-core depends on simse-core and uses its types directly. No conversion layers.
4. **Bridge-unique code to simse-tui** — only session_store.rs and json_io.rs move (everything else has a simse-core equivalent).

## Type Unification

simse-ui-core replaces its own types with simse-core's:

| simse-ui-core (remove) | simse-core (use) |
|---|---|
| ConversationBuffer | simse_core::Conversation |
| ToolCallRequest | simse_core::tools::ToolCallRequest |
| ToolCallResult | simse_core::tools::ToolCallResult |
| ToolDefinition | simse_core::tools::ToolDefinition |
| ToolParameter | simse_core::tools::ToolParameter |
| ToolHandlerOutput | simse_core::tools::ToolHandlerOutput |
| AgenticLoopResult / LoopTurn | simse_core::agentic_loop::* |

simse-ui-core keeps: commands, keybindings, permissions, input handling, tool call parsing, CLI args.

## Module Migration

### Replaced by simse-core

| Bridge Module | simse-core Replacement |
|---|---|
| config.rs (LoadedConfig, ConfigOptions) | simse_core::AppConfig |
| acp_client.rs + acp_types.rs | simse_acp::AcpClient (via simse-core) |
| client.rs + protocol.rs | simse_acp::connection + simse_core::rpc_protocol |
| agentic_loop.rs + LoopCallbacks | simse_core::agentic_loop |
| tool_registry.rs | simse_core::tools::ToolRegistry |

### Moved to simse-tui

| Bridge Module | Reason |
|---|---|
| session_store.rs | JSONL persistence unique to TUI (simse-core has in-memory only) |
| json_io.rs | File I/O utilities used by session store |

### Removed

| Bridge Module | Reason |
|---|---|
| storage.rs | Evaluate usage; remove if unused |

## simse-tui Changes

- **main.rs**: Replace `load_config()` with `AppConfig` init, create `CoreContext`
- **event_loop.rs**: Use `CoreContext` for tool registry, agentic loop, ACP client. Keep local `SessionStore`.
- **Cargo.toml**: Remove `simse-bridge`, add `simse-core`

## simse-ui-core Changes

- **Cargo.toml**: Add `simse-core` dependency
- Replace own type definitions with simse-core re-exports
- Update all modules that reference replaced types
- Keep UI-specific logic unchanged

## Testing

- Unit tests for moved modules (session_store, json_io)
- Integration tests for CoreContext initialization in TUI context
- Integration tests for agentic loop with simse-core API
- Preserve existing simse-core tests (779+)
