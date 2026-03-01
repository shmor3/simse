# simse-core: Full Rust Migration Design

## Goal

Create a single pure Rust library crate (`simse-core/`) that migrates all remaining TypeScript orchestration logic (~20k lines) into Rust, linking the 4 existing engine crates as library dependencies. No JSON-RPC between internal components — direct Rust function calls.

## Architecture

simse-core is a **library-only crate** (no `main.rs`). It sits on top of simse-acp, simse-mcp, simse-vector, and simse-vfs, calling them as Rust libraries rather than spawning child processes. simse-code (the UI/UX layer, being rewritten to Rust by a teammate) will link simse-core directly.

## Tech Stack

- Rust (tokio async runtime)
- serde + serde_json (serialization)
- thiserror v2 (error types)
- tracing + tracing-subscriber (logging)
- tokio-util (CancellationToken replacing AbortSignal)
- uuid (ID generation)

---

## Crate Structure

```
simse-core/
  Cargo.toml
  src/
    lib.rs              # pub mod declarations + re-exports (public API)
    error.rs            # Unified SimseError enum spanning all domains
    config.rs           # AppConfig, define_config, schema validation
    logger.rs           # Structured logger wrapping tracing
    events.rs           # EventBus (typed pub/sub via tokio::sync::broadcast)
    conversation.rs     # Message management, serialization, compaction
    tasks.rs            # TaskList CRUD, dependencies, blocking
    chain.rs            # Chain execution, prompt templates, step runner
    agent.rs            # Step dispatcher (ACP/MCP/library providers)
    agentic_loop.rs     # Multi-turn loop: conversation → ACP → tool exec → repeat
    prompts/
      mod.rs            # SystemPromptBuilder, environment, instruction discovery
      provider.rs       # Provider-specific prompt templates
    tools/
      mod.rs            # ToolRegistry: register, discover, execute, parse
      builtin.rs        # Library, VFS, task tool registrations
      subagent.rs       # Shelf-scoped subagent spawning
      delegation.rs     # Delegation patterns
      permissions.rs    # PermissionHandler trait
      host/
        mod.rs
        filesystem.rs   # File read/write/delete/list (tokio::fs)
        git.rs          # Git commands (tokio::process::Command)
        bash.rs         # Shell execution (tokio::process::Command)
        fuzzy_edit.rs   # Fuzzy text editing (string similarity matching)
    library/
      mod.rs            # Library high-level API (wraps simse_vector::VectorStore)
      shelf.rs          # Agent-scoped namespace partitions
      librarian.rs      # LLM-driven extract/summarize/classify (calls ACP)
      librarian_def.rs  # LibrarianDefinition validation + persistence
      librarian_reg.rs  # Multi-librarian management with lifecycle
      circulation.rs    # Async job queue (tokio::sync::mpsc) with auto-escalation
      services.rs       # Middleware hooking library into agentic loop
      prompt_inject.rs  # Memory context formatting
      query_dsl.rs      # Query DSL parsing
    vfs/
      mod.rs            # VirtualFs orchestration (wraps simse_vfs::Vfs)
      disk.rs           # Disk commit/load operations
      exec.rs           # Command execution passthrough
      validators.rs     # File content validators
    server/
      mod.rs            # MCP server tool exposure + session management
      session.rs        # Session CRUD, forking
    hooks.rs            # Lifecycle hook system
    utils/
      mod.rs
      retry.rs          # Exponential backoff + jitter, CancellationToken
      circuit_breaker.rs # Fault tolerance (threshold=5, reset=30s)
      health_monitor.rs # Sliding window stats
      timeout.rs        # Async timeout utility
```

---

## Error System

Single unified enum with domain variants + `#[from]` passthrough from engine crates:

```rust
#[derive(Debug, thiserror::Error)]
pub enum SimseError {
    #[error("config error: {message}")]
    Config { code: ConfigErrorCode, message: String },

    #[error("provider error: {message}")]
    Provider { code: ProviderErrorCode, message: String, status: Option<u16> },

    #[error("chain error: {message}")]
    Chain { code: ChainErrorCode, message: String },

    #[error("template error: {message}")]
    Template { code: TemplateErrorCode, message: String },

    #[error("MCP error: {message}")]
    Mcp { code: McpErrorCode, message: String },

    #[error("library error: {message}")]
    Library { code: LibraryErrorCode, message: String },

    #[error("loop error: {message}")]
    Loop { code: LoopErrorCode, message: String },

    #[error("resilience error: {message}")]
    Resilience { code: ResilienceErrorCode, message: String },

    #[error("task error: {message}")]
    Task { code: TaskErrorCode, message: String },

    #[error("tool error: {message}")]
    Tool { code: ToolErrorCode, message: String },

    #[error("VFS error: {message}")]
    Vfs { code: VfsErrorCode, message: String },

    // Passthrough from engine crates
    #[error(transparent)]
    Acp(#[from] simse_acp::AcpError),
    #[error(transparent)]
    McpEngine(#[from] simse_mcp::McpError),
    #[error(transparent)]
    Vector(#[from] simse_vector::VectorError),
    #[error(transparent)]
    VfsEngine(#[from] simse_vfs::VfsError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Other(String),
}
```

Pattern matching replaces TS duck-typing type guards.

---

## Core Modules

### Conversation

Message management, serialization, auto-compaction. `Conversation` struct with `Vec<ConversationMessage>`, system prompt, JSON serialization via serde, and `compact()` for summarization.

### Tasks

`TaskList` with CRUD, dependency tracking (`blocked_by` transitive resolution), status transitions (pending → in_progress → completed).

### Events

`EventBus` using `tokio::sync::broadcast` channels. Type-safe via sealed `Event` trait. Replaces TS callback arrays.

### Logger

Thin wrapper around `tracing`. Child loggers become `tracing::Span`s. No custom transport system.

---

## Orchestration Layer

### Chain

`Chain` struct with typed step variants (Generate, Search, Transform, Parallel, Conditional). Builder pattern for construction. `PromptTemplate` uses `{variable}` interpolation.

### Agent

`AgentExecutor` dispatches to ACP/MCP/library based on `ProviderRef` enum.

### Agentic Loop

Core cycle: conversation → ACP stream → check tool calls → execute tools → append results → repeat. Doom loop detection via `(tool_name, args_json)` tuple tracking. Auto-compaction with structured 6-section prompt. `CancellationToken` replaces `AbortSignal`.

### CoreContext

Shared dependency bag:

```rust
pub struct CoreContext {
    pub acp: simse_acp::AcpClient,
    pub mcp: simse_mcp::McpClient,
    pub vector: simse_vector::VectorStore,
    pub vfs: simse_vfs::Vfs,
    pub events: EventBus,
    pub logger: Logger,
    pub config: AppConfig,
}
```

---

## Tools System

### ToolRegistry

HashMap-based registry with async handlers (`ToolHandler` = boxed async fn). Output truncation at 50k chars default, per-tool overrides. MCP tool auto-discovery.

### Builtin Tools

Three registration functions: `register_library_tools()`, `register_vfs_tools()`, `register_task_tools()`. Direct Rust calls into engine crates.

### Host Tools

- `filesystem.rs`: read/write/delete/list via `tokio::fs`
- `git.rs`: git operations via `tokio::process::Command`
- `bash.rs`: shell execution with timeout and working dir
- `fuzzy_edit.rs`: string-similarity-based text replacement

### Subagent Tools

Spawn isolated sub-loops with dedicated `Shelf` (namespace-prefixed library partition) and independent `Conversation`.

### Permissions

`PermissionHandler` trait defined in simse-core. simse-code provides the concrete implementation (UI prompts).

---

## Library Orchestration

`Library` struct wrapping `simse_vector::VectorStore` directly — no JSON-RPC client/stacks layers (~724 lines of TS eliminated).

- **Shelf**: Namespace-prefixed wrapper for agent isolation
- **Librarian**: LLM-powered ops (extract, summarize, classify, reorganize) calling ACP directly
- **CirculationDesk**: `tokio::sync::mpsc` job queue with dual-threshold escalation
- **LibrarianRegistry**: Multi-librarian lifecycle management
- **LibraryServices**: Middleware hooking library into the agentic loop

---

## VFS Orchestration

`VirtualFs` wrapping `simse_vfs::Vfs` directly — no JSON-RPC client layer (~581 lines eliminated).

- **VfsDisk**: Disk commit/load operations
- **Validators**: File content validation (JSON syntax, trailing whitespace)
- **Exec**: Command execution passthrough

---

## Remaining Modules

- **Config**: `AppConfig` struct with serde deserialization + validation. `define_config()` factory.
- **Prompts**: `SystemPromptBuilder`, `discover_instructions()`, provider-specific templates.
- **Server**: `SessionManager` for MCP server sessions (CRUD, fork).
- **Hooks**: `HookSystem` with `BeforeToolCall`/`AfterToolCall`/`BeforeGenerate`/`AfterGenerate` events.
- **Utils**: retry (exponential backoff), circuit breaker (threshold=5, reset=30s), health monitor (sliding window), timeout.

---

## Migration Order

Dependency-driven, simple → complex:

1. error, logger, events, config (zero deps on other modules)
2. conversation, tasks (only depend on error)
3. utils (retry, circuit breaker, timeout)
4. prompts (depends on config)
5. chain, agent (depend on ACP/MCP engine crates)
6. tools (depends on chain, agent, library, vfs)
7. library orchestration (depends on simse-vector + ACP for librarian)
8. vfs orchestration (depends on simse-vfs)
9. agentic loop (depends on everything)
10. server, hooks (top-level wiring)
11. lib.rs public API surface

After migration: delete entire `src/` TypeScript directory.

---

## Estimated Size

- **simse-core**: ~7,500 lines of Rust
- **Replaces**: ~20,000 lines of TypeScript
- **Depends on**: ~15,000 lines across 4 engine crates

## Key Design Decisions

- **Library-only crate**: No `main.rs`, no binary. simse-code links it directly.
- **Direct Rust calls**: No JSON-RPC between simse-core and engine crates. Eliminates serialization overhead.
- **Single unified error enum**: One `SimseError` with domain variants, `#[from]` passthrough from engine crates.
- **CoreContext as dependency bag**: Passed to all async operations. Contains all 4 engine clients + events + logger + config.
- **CancellationToken over AbortSignal**: `tokio-util::CancellationToken` for cooperative cancellation.
- **tracing over custom logger**: Child loggers become spans. No custom transport system.
- **PermissionHandler trait**: simse-core defines the interface, simse-code provides the implementation.
