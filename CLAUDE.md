# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
bun run build              # Bundle with Bun → dist/
bun run typecheck          # tsc --noEmit (strict mode)
bun run lint               # Biome check
bun run lint:fix           # Biome check --write
bun test                   # bun test
bun test --watch           # bun test --watch
bun test --coverage        # bun test --coverage
bun run build:vector-engine  # cd simse-vector && cargo build --release
bun run build:vfs-engine     # cd simse-vfs && cargo build --release
bun run build:acp-engine     # cd simse-acp && cargo build --release
bun run build:mcp-engine     # cd simse-mcp && cargo build --release
bun run build:core           # cd simse-core && cargo build --release
cd simse-vector && cargo test  # Rust vector engine tests
cd simse-vfs && cargo test     # Rust VFS engine tests
cd simse-acp && cargo test     # Rust ACP engine tests
cd simse-mcp && cargo test     # Rust MCP engine tests
cd simse-core && cargo test    # Rust core orchestration tests
```

## Architecture

simse is a modular pipeline framework for orchestrating multi-step AI workflows. It connects to AI backends via **ACP** (Agent Client Protocol), exposes tools via **MCP** (Model Context Protocol), and provides a file-backed **library** (vector store) with compression, cataloging, deduplication, recommendation, and compendium (summarization).

### Repository Layout

```tree
src/                        # TypeScript — main package
simse-core/                 # Pure Rust crate — orchestration library (links all engines)
simse-vector/               # Pure Rust crate — vector store engine (JSON-RPC over stdio)
simse-vfs/                  # Pure Rust crate — virtual filesystem engine (JSON-RPC over stdio)
simse-acp/                  # Pure Rust crate — ACP engine (JSON-RPC over stdio)
simse-mcp/                  # Pure Rust crate — MCP engine (JSON-RPC over stdio)
simse-engine/               # Pure Rust crate — core engine (unchanged)
```

The Rust crates are standalone binaries spawned as child processes by the TS client code. They communicate over JSON-RPC 2.0 / NDJSON stdio.

### Module Layout

```tree
src/
  lib.ts                    # Barrel exports (public API surface)
  logger.ts                 # Structured logger with child loggers
  errors/                   # Error hierarchy split by domain
    base.ts                 # SimseError interface, createSimseError, toError, wrapError
    config.ts               # Config error factories + guards
    provider.ts             # Provider error factories + guards (incl. HTTP errors)
    chain.ts                # Chain error factories + guards
    template.ts             # Template error factories + guards
    mcp.ts                  # MCP error factories + guards
    library.ts              # Re-exports library errors from ai/library/errors.ts
    loop.ts                 # Agentic loop error factories + guards
    resilience.ts           # CircuitBreaker/Timeout error factories + guards
    tasks.ts                # Task list error factories + guards
    tools.ts                # Tool registry error factories + guards
    vfs.ts                  # Re-exports VFS errors from ai/vfs/errors.ts
    index.ts                # Barrel re-export
  config/
    schema.ts               # Typed config validation (semantic-only, no runtime type guards)
    settings.ts             # AppConfig type + defineConfig()
  ai/
    shared/
      logger.ts             # Minimal Logger + EventBus interfaces, createNoopLogger
                             # Shared by library/ and vfs/ (subset of root logger.ts)
    acp/
      acp-engine-client.ts  # JSON-RPC client spawning simse-acp Rust engine
      acp-client.ts         # Thin client delegating to acp-engine-client
                             # generate(), generateStream(), chat(), embed()
                             # Session management, mode/model switching
      acp-adapters.ts       # EmbeddingProvider + TextGenerationProvider adapters for ACP
      local-embedder.ts     # In-process embedding via @huggingface/transformers
      tei-bridge.ts         # Text Embeddings Inference (TEI) HTTP bridge
      types.ts              # ACP types: sessions, content blocks, streaming, permissions,
                             # tool calls, sampling params, model/mode info
      index.ts              # Barrel re-export
    mcp/
      mcp-engine-client.ts  # JSON-RPC client spawning simse-mcp Rust engine
      mcp-client.ts         # Thin client delegating to mcp-engine-client
                             # tools, resources, prompts, completions, logging, roots
      mcp-server.ts         # MCP server with tool handler callback pattern
                             # generate, run-chain, list-agents, library-search,
                             # library-shelve, vfs-*, task-* tools
      types.ts              # MCP types: tools, resources, prompts, logging,
                             # completions, roots, resource templates, annotations
      index.ts              # Barrel re-export
    agent/
      agent-executor.ts     # Step execution dispatcher (acp/mcp/library providers)
      types.ts              # AgentResult, AgentStepConfig, ParallelConfig, SwarmMerge
      index.ts              # Barrel re-export
    chain/
      chain.ts              # createChain factory, createChainFromDefinition, runNamedChain
      prompt-template.ts    # PromptTemplate interface + createPromptTemplate
      format.ts             # formatSearchResults helper
      types.ts              # Provider, ChainStepConfig, StepResult, ChainCallbacks
      index.ts              # Barrel re-export
    conversation/
      conversation.ts       # createConversation factory: message management,
                             # serialization, auto-compaction
      types.ts              # ConversationRole, ConversationMessage, ConversationOptions
      index.ts              # Barrel re-export
    loop/
      agentic-loop.ts       # createAgenticLoop: conversation → ACP stream → tool exec → repeat
                             # maxTurns, AbortSignal, auto-compaction, streaming retry
                             # Doom loop detection: maxIdenticalToolCalls, onDoomLoop callback
                             # Structured compaction: compactionPrompt override, onPreCompaction hook
      types.ts              # AgenticLoopOptions, LoopTurn, AgenticLoopResult, LoopCallbacks
      index.ts              # Barrel re-export
    library/                # TS client layer for the vector store (talks to simse-vector Rust engine)
      index.ts              # Barrel re-export (public API surface)
      client.ts             # VectorClient: JSON-RPC client spawning simse-vector engine
      library.ts            # Library (createLibrary): add/search/recommend/compendium/findDuplicates
      stacks.ts             # Stacks (createStacks): async wrapper over Rust engine
      shelf.ts              # Shelf (createShelf): agent-scoped library partition
      librarian.ts          # Librarian (createLibrarian, createDefaultLibrarian):
                             # extract, summarize, classifyTopic, reorganize, optimize
      librarian-definition.ts  # LibrarianDefinition validation & persistence
      librarian-registry.ts    # Multi-librarian management
      circulation-desk.ts   # CirculationDesk (createCirculationDesk): async background queue
      library-services.ts   # LibraryServices middleware (createLibraryServices)
      prompt-injection.ts   # formatMemoryContext: structured/natural memory context for prompts
      query-dsl.ts          # Query DSL parsing (parseQuery)
      errors.ts             # Library/Embedding/Stacks error factories + guards
      types.ts              # All library/search/deduplication/recommendation/compendium types
    vfs/                    # TS client layer for the virtual filesystem (talks to simse-vfs Rust engine)
      index.ts              # Barrel re-export (public API surface)
      client.ts             # VFSClient: JSON-RPC client spawning simse-vfs engine
      vfs.ts                # VirtualFS (createVirtualFS): async wrapper over Rust engine
      vfs-disk.ts           # VFSDisk (createVFSDisk): disk commit/load operations
      exec.ts               # VFSExecutor: command execution passthrough
      validators.ts         # File content validators (JSON syntax, trailing whitespace, etc.)
      path-utils.ts         # Path validation & utilities (normalizePath, validatePath, etc.)
      errors.ts             # VFS error factories + guards
      types.ts              # VirtualFS, VFSDirEntry, VFSReadResult, VFSWriteOptions
    tasks/
      task-list.ts          # createTaskList factory: CRUD, dependencies, blocking
      types.ts              # TaskItem, TaskStatus, TaskList, TaskCreateInput, TaskUpdateInput
      index.ts              # Barrel re-export
    tools/
      tool-registry.ts      # createToolRegistry: register, discover, execute, parse
                             # Tool output truncation: maxOutputChars (registry + per-tool)
      builtin-tools.ts      # registerLibraryTools, registerVFSTools, registerTaskTools
      subagent-tools.ts     # registerSubagentTools: spawn sub-loops as tool calls
                             # Shelf-scoped library integration for subagent isolation
      types.ts              # ToolDefinition, ToolHandler, ToolRegistry, ToolCallRequest
      index.ts              # Barrel re-export
  utils/
    retry.ts                # Retry with exponential backoff + jitter, AbortSignal support
    circuit-breaker.ts      # Circuit breaker pattern for fault tolerance
    health-monitor.ts       # Health monitoring with sliding window stats
    timeout.ts              # withTimeout utility for Promise-based timeouts

simse-core/                 # Pure Rust library crate — orchestration layer
  Cargo.toml
  src/
    lib.rs                  # Module declarations + crate-root re-exports
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
    chain/                  # Chain execution (run_chain, ChainStep)
    agent/                  # Agent executor (dispatch steps)
    tools/                  # ToolRegistry, builtin/host/subagent tools
    library/                # Library, Stacks, Shelf, Librarian, Registry, CirculationDesk
    vfs/                    # VirtualFs, VfsDisk, VfsExec, validators
    server/                 # SessionManager with fork support
    utils/                  # retry, circuit_breaker, timeout
  tests/                    # Integration tests (745 tests)

simse-vector/               # Pure Rust crate
  Cargo.toml
  src/
    lib.rs                  # Library entry point
    main.rs                 # Binary entry point (JSON-RPC server)
    server.rs               # JSON-RPC request dispatcher
    transport.rs            # Stdio & HTTP transport
    protocol.rs             # JSON-RPC protocol definitions
    types.rs                # Protocol & internal types
    store.rs                # Main vector store implementation
    cosine.rs               # Cosine similarity (clamped [-1,1])
    persistence.rs          # Float32 base64 encoding, gzip, save/load
    cataloging.rs           # TopicIndex, MetadataIndex, MagnitudeCache
    deduplication.rs        # Duplicate detection & clustering
    recommendation.rs       # Scoring with recency/frequency
    text_search.rs          # Exact/substring/fuzzy/regex/token search
    text_cache.rs           # Text content caching (LRU)
    inverted_index.rs       # BM25 text search indexing
    topic_catalog.rs        # Hierarchical topic classification
    learning.rs             # Adaptive learning engine
    query_dsl.rs            # Query DSL parsing
    prompt_injection.rs     # Memory context formatting
    error.rs                # Rust error types
  tests/
    integration.rs          # Integration tests

simse-vfs/                  # Pure Rust crate
  Cargo.toml
  src/
    lib.rs                  # Library entry point
    main.rs                 # Binary entry point (JSON-RPC server)
    server.rs               # JSON-RPC request dispatcher
    transport.rs            # Stdio transport
    protocol.rs             # JSON-RPC protocol definitions
    vfs.rs                  # Core VFS implementation
    diff.rs                 # Diff generation
    glob.rs                 # Glob pattern matching
    search.rs               # File search implementation
    path.rs                 # Path utilities
    error.rs                # Rust error types
  tests/
    integration.rs          # Integration tests

simse-acp/                  # Pure Rust crate
  Cargo.toml
  src/
    lib.rs                  # Library entry point
    main.rs                 # Binary entry point (JSON-RPC server)
    server.rs               # JSON-RPC request dispatcher
    transport.rs            # NDJSON stdio transport
    protocol.rs             # ACP + JSON-RPC protocol definitions
    connection.rs           # Child process management, request/response tracking
    permission.rs           # Permission policy resolution (AutoApprove/Deny/Prompt)
    stream.rs               # Streaming state machine (futures::Stream<Item = StreamChunk>)
    resilience.rs           # Circuit breaker, health monitor, retry with backoff
    client.rs               # AcpClient orchestration: multi-server pool, sessions, agents
    error.rs                # Rust error types (AcpError)
  tests/
    integration.rs          # Integration tests

simse-mcp/                  # Pure Rust crate
  Cargo.toml
  src/
    lib.rs                  # Library entry point
    main.rs                 # Binary entry point (JSON-RPC server)
    rpc_server.rs           # JSON-RPC request dispatcher (wraps McpClient + McpServer)
    rpc_transport.rs        # NDJSON stdio transport for the RPC wrapper
    protocol.rs             # MCP + JSON-RPC protocol definitions
    stdio_transport.rs      # Stdio transport for connecting to MCP servers
    http_transport.rs       # HTTP transport for connecting to MCP servers
    client.rs               # McpClient: multi-server connections, tools, resources, prompts
    mcp_server.rs           # McpServer: tool/resource/prompt hosting for external clients
    error.rs                # Rust error types (McpError)
  tests/
    integration.rs          # Integration tests
```

### Key Patterns

- **Factory functions over classes**: Every module exports a `createXxx()` factory returning a readonly interface. No classes in the codebase.
- **Immutable returns**: Factory functions use `Object.freeze()` on returned objects.
- **Error hierarchy**: `createSimseError` is the base; specialized factories (`createProviderError`, `createConfigError`, `createLibraryError`, etc.) add typed `code` fields. Type guards use duck-typing on `code`.
- **`toError(unknown)`**: Always wrap catch-block errors with `toError()` from `errors/index.js` before accessing `.message`.
- **ESM-only**: All imports use `.js` extensions (`import { foo } from './bar.js'`). The `verbatimModuleSyntax` tsconfig flag is enabled — use `import type` for type-only imports.
- **Shared logger interface**: `src/ai/shared/logger.ts` defines a minimal `Logger` + `EventBus` interface used by `library/` and `vfs/`. The root `src/logger.ts` is a superset — never import the root logger from library/vfs code.
- **Rust engine subprocess pattern**: `library/client.ts`, `vfs/client.ts`, `acp/acp-engine-client.ts`, and `mcp/mcp-engine-client.ts` each spawn a Rust binary as a child process, communicate over JSON-RPC 2.0 / NDJSON stdio, and handle lifecycle (spawn, health check, dispose).
- **In-flight promise deduplication**: `load()`, `initialize()`, MCP `start()`, and MCP `connect()` use a stored promise to deduplicate concurrent callers. The pattern is: check for existing promise → create if missing → clear in `.finally()`.
- **Doom loop detection**: The agentic loop tracks consecutive identical tool calls (same name + JSON-stringified args). After `maxIdenticalToolCalls` (default 3), it fires `onDoomLoop` callback, publishes `loop.doom_loop` event, and injects a system warning into the conversation.
- **Tool output truncation**: `ToolRegistryOptions.maxOutputChars` (default 50,000) caps tool output to prevent context overflow. Per-tool `ToolDefinition.maxOutputChars` overrides the registry default. Truncated output gets an `[OUTPUT TRUNCATED]` suffix.
- **Session forking**: `SessionManager.fork(id)` creates a new session with cloned conversation state via `toJSON()`/`fromJSON()`, a fresh event bus, and new ID/timestamp.
- **Structured compaction**: When auto-compaction fires, the prompt requests 6 sections (Goal, Progress, Current State, Key Decisions, Relevant Files, Next Steps). `AgenticLoopOptions.compactionPrompt` overrides the default. `LoopCallbacks.onPreCompaction` can inject extra context before summarization.
- **Connection health check**: ACP connection health is managed in Rust (`connection.rs` uses `Arc<AtomicBool>` updated on process EOF). `withResilience` auto-reconnects unhealthy connections before retry attempts.

### ACP Protocol

The ACP engine is implemented in Rust (`simse-acp/`) and exposes the [Agent Client Protocol](https://agentclientprotocol.com) over JSON-RPC 2.0 / NDJSON stdio. The TS layer (`src/ai/acp/acp-engine-client.ts`) is a thin JSON-RPC client that spawns the Rust binary.

**Rust engine** (`simse-acp/src/`) — handles all ACP operations:
- **Client** (`client.rs`): Multi-server connection pool, session caching, agent discovery, resilient request execution
- **Connection** (`connection.rs`): Child process management, NDJSON parsing, request/response tracking, notification routing
- **Stream** (`stream.rs`): `futures::Stream` state machine with sliding-window timeout, cancellation, permission-aware suspension
- **Permission** (`permission.rs`): Policy-based resolution (AutoApprove/Deny/Prompt)
- **Resilience** (`resilience.rs`): Circuit breaker (Closed/Open/HalfOpen), health monitor (sliding window), retry with exponential backoff + jitter
- **Server** (`server.rs`): JSON-RPC dispatcher routing requests to AcpClient

**Protocol details:**
- **Protocol version**: 1
- **Field naming**: camelCase throughout (`sessionId`, `stopReason`, `agentInfo`, not snake_case)
- **Session lifecycle**: `session/new` → `session/prompt` → `session/update` notifications → response
- **Permission flow**: Agent sends `session/request_permission` with options array; client selects `allow_once`/`allow_always`/`reject_once`/`reject_always` via `{ outcome: { outcome: "selected", optionId } }`
- **Session modes**: Set via `session/set_config_option` (configOptionId: "mode", groupId: modeId)
- **Tool call lifecycle**: `tool_call` → `tool_call_update` (in_progress) → `tool_call_update` (completed) — all via `session/update` notifications
- **Sampling params**: `temperature`, `maxTokens`, `topP`, `topK`, `stopSequences` passed in prompt metadata
- **Agent fallback**: When no agentId is configured, falls back to server name
- **Timeout defaults**: `timeoutMs` = 60s (per-request), `initTimeoutMs` = 30s (initialize handshake). Both overridable via `ACPConnectionOptions`.
- **Connection health**: `isHealthy` checks child process liveness (not killed, no exit code). `withResilience` auto-reconnects before retrying failed operations.
- **Retry events**: `stream.retry` includes `delayMs` and `nextAttemptAt` timestamp for UI countdown display

### MCP Protocol

The MCP engine is implemented in Rust (`simse-mcp/`) and implements the [Model Context Protocol](https://modelcontextprotocol.io) over JSON-RPC 2.0 / NDJSON stdio. The TS layer (`src/ai/mcp/mcp-engine-client.ts`) is a thin JSON-RPC client that spawns the Rust binary.

**Rust engine** (`simse-mcp/src/`) — handles all MCP operations:
- **Client** (`client.rs`): Multi-server connections (stdio + HTTP), tool/resource/prompt aggregation, circuit breaker + health monitor per server, retry with backoff
- **Server** (`mcp_server.rs`): Tool/resource/prompt registration and hosting, capability advertisement, logging level management, workspace roots tracking
- **Stdio transport** (`stdio_transport.rs`): Child process management, MCP initialize/initialized handshake, notification routing
- **HTTP transport** (`http_transport.rs`): HTTP POST-based JSON-RPC transport for remote MCP servers
- **RPC server** (`rpc_server.rs`): JSON-RPC dispatcher wrapping both McpClient and McpServer, tool execution callback pattern for TS integration
- **RPC transport** (`rpc_transport.rs`): NDJSON stdio transport for the outer JSON-RPC wrapper

**Protocol details:**
- **Client**: Connects to external MCP servers via stdio or HTTP transport
- **Server**: Exposes simse capabilities as MCP tools (generate, run-chain, list-agents, library-search, library-shelve, vfs-*, task-*)
- **Logging**: `setLoggingLevel()` + `onLoggingMessage()` for structured log collection
- **List-changed**: Notification handlers for dynamic tool/resource/prompt discovery
- **Completions**: `complete()` for argument autocomplete
- **Roots**: `setRoots()` + `sendRootsListChanged()` for workspace awareness
- **Resource templates**: `listResourceTemplates()` for URI pattern discovery
- **Retry**: Tool calls and resource reads use exponential backoff

### Library System

The library subsystem uses a **library analogy** throughout. The core storage engine is implemented in Rust (`simse-vector/`), while higher-level orchestration services are in TypeScript (`src/ai/library/`).

**Rust engine** (`simse-vector/src/`) — handles all vector operations via JSON-RPC:

- **Store** (`store.rs`): Add, search, delete, recommend, duplicate detection, topic operations
- **Persistence** (`persistence.rs`): Float32↔base64 encoding (~75% size reduction), gzip compression, save/load
- **Cataloging** (`cataloging.rs`): TopicIndex, MetadataIndex, MagnitudeCache
- **Deduplication** (`deduplication.rs`): Cosine-based duplicate detection & clustering
- **Recommendation** (`recommendation.rs`): Vector similarity + recency decay + frequency scoring
- **Text search** (`text_search.rs`, `inverted_index.rs`): Exact/substring/fuzzy/regex/token/BM25 modes
- **Learning** (`learning.rs`): Adaptive weight adaptation from query feedback

**TypeScript client layer** (`src/ai/library/`) — orchestration and LLM integration:

1. **Library** (`library.ts`): High-level API wrapping the Rust engine client
2. **Stacks** (`stacks.ts`): Async wrapper spawning the Rust engine subprocess
3. **Client** (`client.ts`): JSON-RPC transport to the Rust engine
4. **Shelf** (`shelf.ts`): Agent-scoped library partitions. Subagents get dedicated shelves via `subagent-tools.ts`.
5. **Librarian** (`librarian.ts`): LLM-driven extraction, summarization, classification, reorganization. `createDefaultLibrarian(acpClient)` wraps any ACP client.
6. **LibrarianDefinition** (`librarian-definition.ts`): Validation & file-based persistence of librarian configs.
7. **LibrarianRegistry** (`librarian-registry.ts`): Multi-librarian management with connection lifecycle.
8. **CirculationDesk** (`circulation-desk.ts`): Async job queue with auto-escalation. Dual thresholds trigger optimization with a powerful model.
9. **LibraryServices** (`library-services.ts`): Middleware integrating library with the agentic loop.

### Formatting

- **Biome** (not ESLint/Prettier): tabs for indentation, single quotes, semicolons
- Organize imports is enabled (biome handles this)
