# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Rust crate builds
bun run build:vector-engine  # cd simse-vector && cargo build --release
bun run build:vfs-engine     # cd simse-vfs && cargo build --release
bun run build:acp-engine     # cd simse-acp && cargo build --release
bun run build:mcp-engine     # cd simse-mcp && cargo build --release
bun run build:core           # cd simse-core && cargo build --release
bun run build:tui            # cd simse-tui && cargo build --release
bun run build:vsh-engine     # cd simse-vsh && cargo build --release
bun run build:vnet-engine    # cd simse-vnet && cargo build --release

# Rust tests
cd simse-vector && cargo test  # Rust vector engine tests
cd simse-vfs && cargo test     # Rust VFS engine tests
cd simse-acp && cargo test     # Rust ACP engine tests
cd simse-mcp && cargo test     # Rust MCP engine tests
cd simse-vsh && cargo test     # Rust VSH engine tests
cd simse-vnet && cargo test    # Rust vnet engine tests
cd simse-core && cargo test    # Rust core orchestration tests
cd simse-ui-core && cargo test # Rust UI core tests
cd simse-bridge && cargo test  # Rust bridge tests
cd simse-tui && cargo test     # Rust TUI tests (unit + integration)
```

## Architecture

simse is a modular pipeline framework for orchestrating multi-step AI workflows. It connects to AI backends via **ACP** (Agent Client Protocol), exposes tools via **MCP** (Model Context Protocol), and provides a file-backed **library** (vector store) with compression, cataloging, deduplication, recommendation, and compendium (summarization).

The entire core is implemented in **Rust**. Each crate is a standalone binary communicating over JSON-RPC 2.0 / NDJSON stdio.

### Repository Layout

```tree
simse-core/                 # Pure Rust crate — orchestration library + JSON-RPC binary server
simse-vector/               # Pure Rust crate — vector store engine (JSON-RPC over stdio)
simse-vfs/                  # Pure Rust crate — virtual filesystem engine (vfs:// in-memory + file:// disk, JSON-RPC over stdio)
simse-acp/                  # Pure Rust crate — ACP engine (JSON-RPC over stdio)
simse-mcp/                  # Pure Rust crate — MCP engine (JSON-RPC over stdio)
simse-vsh/                  # Pure Rust crate — virtual shell engine (JSON-RPC over stdio)
simse-vnet/                 # Pure Rust crate — virtual network engine (JSON-RPC over stdio)
simse-ui-core/              # Pure Rust crate — platform-agnostic UI logic (no I/O)
simse-tui/                  # Pure Rust crate — terminal UI (ratatui, Elm Architecture)
simse-bridge/               # Pure Rust crate — async I/O bridge (ACP client, config, sessions, storage)
simse-engine/               # Pure Rust crate — core engine
simse-cloud/                # TypeScript — SaaS web app (React Router + Cloudflare)
simse-landing/              # TypeScript — Landing page (Remix)
simse-mailer/               # TypeScript — Email templates
```

### simse-core Module Layout

```tree
simse-core/
  Cargo.toml
  src/
    lib.rs                  # Module declarations + crate-root re-exports
    main.rs                 # Binary entry point (simse-core-engine, JSON-RPC server)
    context.rs              # CoreContext: top-level wiring struct
    error.rs                # SimseError enum with domain variants
    config.rs               # AppConfig + typed config structs
    logger.rs               # Structured Logger with child loggers
    events.rs               # EventBus: thread-safe pub/sub
    conversation.rs         # Conversation: message management, JSON serialization
    tasks.rs                # TaskList: CRUD, dependencies, blocking
    prompts.rs              # SystemPromptBuilder
    agentic_loop.rs         # run_agentic_loop: generate→parse→execute→repeat
    hooks.rs                # HookSystem: 6 hook types with chaining/blocking
    rpc_server.rs           # JSON-RPC dispatcher (48 methods across 9 domains)
    rpc_protocol.rs         # JSON-RPC 2.0 framing types
    rpc_transport.rs        # NDJSON stdio transport
    chain/                  # Chain execution (run_chain, ChainStep)
    agent/                  # Agent executor (dispatch steps)
    tools/                  # ToolRegistry, builtin/host/subagent tools
    library/                # Library, Stacks, Shelf, Librarian, Registry, CirculationDesk
    vfs/                    # VirtualFs, VfsDisk, VfsExec, validators
    server/                 # SessionManager with fork support
    utils/                  # retry, circuit_breaker, timeout
  tests/                    # Integration tests (779+ tests)
```

### simse-core JSON-RPC Methods

| Domain | Methods |
|--------|---------|
| `core/` | `initialize`, `dispose`, `health` |
| `session/` | `create`, `get`, `list`, `delete`, `updateStatus`, `fork` |
| `conversation/` | `addUser`, `addAssistant`, `addToolResult`, `setSystemPrompt`, `getMessages`, `compact`, `clear`, `stats`, `toJson`, `fromJson` |
| `task/` | `create`, `get`, `list`, `listAvailable`, `update`, `delete` |
| `event/` | `subscribe`, `unsubscribe`, `publish` |
| `hook/` | `registerBefore`, `registerAfter`, `registerValidate`, `registerTransform`, `unregister`, `result` |
| `tool/` | `register`, `unregister`, `list`, `execute`, `batchExecute`, `parse`, `formatSystemPrompt`, `metrics`, `result` |
| `chain/` | `run`, `runNamed`, `stepResult` |
| `loop/` | `run`, `cancel` |

### Other Rust Crates

```tree
simse-vector/               # Pure Rust crate — vector store
  src/
    store.rs                # Main vector store implementation
    cosine.rs               # Cosine similarity (clamped [-1,1])
    persistence.rs          # Float32 base64 encoding, gzip, save/load
    cataloging.rs           # TopicIndex, MetadataIndex, MagnitudeCache
    deduplication.rs        # Duplicate detection & clustering
    recommendation.rs       # Scoring with recency/frequency
    text_search.rs          # Exact/substring/fuzzy/regex/token search
    inverted_index.rs       # BM25 text search indexing
    topic_catalog.rs        # Hierarchical topic classification
    learning.rs             # Adaptive learning engine
    query_dsl.rs            # Query DSL parsing
    prompt_injection.rs     # Memory context formatting

simse-vfs/                  # Pure Rust crate — virtual filesystem
  src/
    vfs.rs                  # Core VFS implementation (vfs:// in-memory backend)
    disk.rs                 # DiskFs: real filesystem operations (file:// backend, shadow history)
    diff.rs                 # Diff generation (Myers algorithm, shared by both backends)
    glob.rs                 # Glob pattern matching
    search.rs               # File search implementation

simse-acp/                  # Pure Rust crate — ACP engine
  src/
    client.rs               # AcpClient: multi-server pool, sessions, agents
    connection.rs           # Child process management, request/response tracking
    stream.rs               # Streaming state machine (futures::Stream)
    permission.rs           # Permission policy resolution
    resilience.rs           # Circuit breaker, health monitor, retry

simse-mcp/                  # Pure Rust crate — MCP engine
  src/
    client.rs               # McpClient: multi-server connections, tools, resources
    mcp_server.rs           # McpServer: tool/resource/prompt hosting
    rpc_server.rs           # JSON-RPC dispatcher wrapping client + server
    stdio_transport.rs      # Stdio transport for MCP server connections
    http_transport.rs       # HTTP transport for remote MCP servers

simse-vsh/                  # Pure Rust crate — virtual shell engine
  src/
    error.rs                # VshError enum with VSH_ code prefixes
    protocol.rs             # JSON-RPC request/response types (19 methods)
    transport.rs            # NdjsonTransport for JSON-RPC over stdio
    sandbox.rs              # SandboxConfig: path validation, command filtering
    executor.rs             # Command execution via tokio::process with timeouts
    shell.rs                # VirtualShell: session management, env, aliases, history
    server.rs               # VshServer: JSON-RPC dispatcher (async)

simse-vnet/                 # Pure Rust crate — virtual network engine
  src/
    error.rs                # VnetError enum with VNET_ code prefixes
    protocol.rs             # JSON-RPC request/response types (19 methods)
    transport.rs            # NdjsonTransport for JSON-RPC over stdio
    sandbox.rs              # NetSandboxConfig: host/port/protocol allowlist validation
    mock_store.rs           # MockStore: mock registry + glob pattern matching
    session.rs              # SessionManager: persistent connection tracking (WS, TCP)
    network.rs              # VirtualNetwork: core logic, mock HTTP, sandbox, metrics
    server.rs               # VnetServer: 19-method JSON-RPC dispatch
```

### TUI Crates (CLI Application)

```tree
simse-ui-core/              # Platform-agnostic UI logic (no I/O, no async)
  src/
    cli.rs                  # Non-interactive mode arg parsing
    state/
      conversation.rs       # ConversationBuffer with auto-compaction
      permission_manager.rs # Permission modes, rules, glob matching
      permissions.rs        # Permission mode/decision types
    input/
      keybindings.rs        # KeyCombo registry and matching
    tools/
      mod.rs                # Tool types, formatting, truncation
      parser.rs             # Tool call parser (XML blocks from LLM responses)
    commands/                # Command registry (34 commands)
    config/                  # Settings schemas

simse-tui/                  # Terminal UI (ratatui + crossterm + tokio)
  src/
    app.rs                  # App model (Elm Architecture: Model/Update/View)
    event_loop.rs           # TuiRuntime: bridges App to simse-bridge
    cli_args.rs             # CLI argument parsing
    onboarding.rs           # First-run setup detection
    dispatch.rs             # Command dispatch routing
    markdown.rs             # Markdown→ratatui with syntax highlighting
    spinner.rs              # Animated thinking spinner
    autocomplete.rs         # /command autocomplete
    at_mention.rs           # @file path autocomplete
    status_bar.rs           # Status bar rendering
    tool_call_box.rs        # Tool call display with diff
    error_box.rs            # Error display
    dialogs/                # Permission + confirm dialogs
    overlays/               # Settings, librarian, setup, ollama wizard
    commands/               # Feature command handlers (library, session, config, files, ai, tools, meta)

simse-bridge/               # Async I/O bridge (tokio)
  src/
    client.rs               # JSON-RPC client (subprocess management)
    acp_client.rs           # ACP client (connect, generate, stream, embed)
    config.rs               # Config loading (8 files, agents, skills, SIMSE.md)
    session_store.rs        # JSONL session persistence
    storage.rs              # Binary storage backend (SIMK format, gzip, atomic writes)
    tool_registry.rs        # Tool registry (register, discover, execute)
    agentic_loop.rs         # Agentic loop (conversation→ACP→parse→execute→repeat)
    json_io.rs              # JSON/JSONL utilities
```

### Key Patterns

- **Rust-first architecture**: All core logic is in Rust. TS packages (simse-cloud, simse-landing, simse-mailer) are application layers only.
- **JSON-RPC 2.0 / NDJSON stdio**: All Rust crates expose their APIs via JSON-RPC over newline-delimited JSON on stdin/stdout. Tracing/logs go to stderr.
- **Callback pattern**: Tools, hooks, chains, and loops registered from external callers use oneshot channels + JSON-RPC notifications for async callback execution.
- **CoreContext wiring**: `CoreContext` ties together EventBus, Logger, AppConfig, TaskList, HookSystem, SessionManager, ToolRegistry, and optional Library/VFS.
- **Error format**: `{ code: -32000, message: "...", data: { coreCode: "NOT_INITIALIZED" | "SESSION_NOT_FOUND" | ... } }`
- **Doom loop detection**: The agentic loop tracks consecutive identical tool calls. After `maxIdenticalToolCalls` (default 3), it fires callbacks and injects a system warning.
- **Tool output truncation**: `ToolRegistryOptions.maxOutputChars` (default 50,000) caps tool output. Per-tool override via `ToolDefinition.maxOutputChars`.
- **Session forking**: `SessionManager.fork(id)` clones conversation state, creates fresh event bus and new ID.
- **Structured compaction**: Auto-compaction requests 6 sections (Goal, Progress, Current State, Key Decisions, Relevant Files, Next Steps).
- **Arc<AtomicBool> for health flags**: Connection health shared between spawned reader tasks and main struct.

### ACP Protocol

The ACP engine (`simse-acp/`) exposes the [Agent Client Protocol](https://agentclientprotocol.com) over JSON-RPC 2.0 / NDJSON stdio.

**Protocol details:**
- **Protocol version**: 1
- **Field naming**: camelCase throughout (`sessionId`, `stopReason`, `agentInfo`)
- **Session lifecycle**: `session/new` → `session/prompt` → `session/update` notifications → response
- **Permission flow**: Agent sends `session/request_permission`; client responds with `allow_once`/`allow_always`/`reject_once`/`reject_always`
- **Tool call lifecycle**: `tool_call` → `tool_call_update` (in_progress) → `tool_call_update` (completed)
- **Timeout defaults**: `timeoutMs` = 60s, `initTimeoutMs` = 30s (both overridable)

### MCP Protocol

The MCP engine (`simse-mcp/`) implements the [Model Context Protocol](https://modelcontextprotocol.io) over JSON-RPC 2.0 / NDJSON stdio.

**Protocol details:**
- **Client**: Connects to external MCP servers via stdio or HTTP transport
- **Server**: Exposes simse capabilities as MCP tools
- **Features**: Logging, list-changed notifications, completions, roots, resource templates

### Library System

The library subsystem uses a **library analogy**. The storage engine is in Rust (`simse-vector/`), with orchestration in `simse-core/src/library/`.

**Rust engine** (`simse-vector/src/`) — all vector operations via JSON-RPC:
- Store, persistence, cataloging, deduplication, recommendation, text search, BM25, topic classification, adaptive learning

**simse-core library layer** (`simse-core/src/library/`) — orchestration:
- Library, Stacks, Shelf, Librarian, LibrarianRegistry, CirculationDesk

### Formatting

- **Biome** (not ESLint/Prettier): tabs for indentation, single quotes, semicolons (for TS packages)
- **Rust**: standard `rustfmt` defaults, clippy with `-D warnings`
