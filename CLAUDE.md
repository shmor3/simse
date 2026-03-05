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
bun run build:sandbox-engine # cd simse-sandbox && cargo build --release

# Rust tests
cd simse-vector && cargo test  # Rust vector engine tests
cd simse-vfs && cargo test     # Rust VFS engine tests
cd simse-acp && cargo test     # Rust ACP engine tests
cd simse-mcp && cargo test     # Rust MCP engine tests
cd simse-vsh && cargo test     # Rust VSH engine tests
cd simse-vnet && cargo test    # Rust vnet engine tests
cd simse-core && cargo test    # Rust core orchestration tests
cd simse-ui-core && cargo test # Rust UI core tests
cd simse-sandbox && cargo test # Rust sandbox engine tests
cd simse-tui && cargo test     # Rust TUI tests (unit + integration)

# TypeScript lint (all TS services use Biome)
cd simse-api && npm run lint       # API gateway lint
cd simse-auth && npm run lint      # Auth service lint
cd simse-payments && npm run lint  # Payments service lint
cd simse-cdn && npm run lint       # CDN worker lint

# TypeScript tests
cd simse-cdn && npm run test   # CDN worker tests (Vitest + @cloudflare/vitest-pool-workers)
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
simse-sandbox/              # Pure Rust crate — unified sandbox engine (JSON-RPC over stdio)
simse-ui-core/              # Pure Rust crate — platform-agnostic UI logic (no I/O)
simse-tui/                  # Pure Rust crate — terminal UI (ratatui, Elm Architecture, depends on simse-core directly)
simse-engine/               # Pure Rust crate — core engine
simse-cdn/                  # TypeScript — CDN worker (R2 + KV, Cloudflare Worker)
simse-cloud/                # TypeScript — SaaS web app (React Router + Cloudflare Pages)
simse-api/                  # TypeScript — API gateway (Cloudflare Worker, proxies to backend services)
simse-auth/                 # TypeScript — Auth service (Cloudflare Worker, D1, users/sessions/teams/API keys)
simse-payments/             # TypeScript — Payments service (Cloudflare Worker, Stripe)
simse-landing/              # TypeScript — Landing page (React Router + Cloudflare)
simse-mailer/               # TypeScript — Email templates + notifications
simse-brand/                # Brand assets (logos, screenshots, guidelines, copy)
simse-predictive-coding/    # Pure Rust crate — predictive coding engine (JSON-RPC over stdio)
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
    prompts/                # SystemPromptBuilder, environment, provider
    agentic_loop.rs         # run_agentic_loop: generate→parse→execute→repeat
    agent.rs                # Agent executor (dispatch steps)
    hooks.rs                # HookSystem: 6 hook types with chaining/blocking
    rpc_server.rs           # JSON-RPC dispatcher (48 methods across 9 domains)
    rpc_protocol.rs         # JSON-RPC 2.0 framing types
    rpc_transport.rs        # NDJSON stdio transport
    chain/                  # Chain execution (run_chain, ChainStep)
    tools/                  # ToolRegistry, builtin/host/subagent/delegation tools
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

simse-sandbox/              # Pure Rust crate — unified sandbox engine
  src/
    lib.rs                  # Module declarations, re-exports
    main.rs                 # Binary entry point (simse-sandbox-engine)
    error.rs                # SandboxError enum (SANDBOX_ prefix)
    protocol.rs             # JSON-RPC param/result types
    transport.rs            # NdjsonTransport (same pattern as other crates)
    server.rs               # SandboxServer: JSON-RPC dispatcher (63 methods across 7 domains)
    sandbox.rs              # Sandbox: unified orchestrator, backend switching
    config.rs               # BackendConfig, SshConfig, SshAuth
    ssh/
      mod.rs                # SSH module root
      pool.rs               # SshPool: multiplexed russh connection manager
      channel.rs            # ExecOutput, channel read with timeout
      fs_backend.rs         # FsBackend impl over SFTP
      shell_backend.rs      # ShellBackend impl over exec channel
      net_backend.rs        # NetBackend impl over exec channel (curl/getent)
  tests/
    integration.rs          # 10 integration tests (local backend)
    ssh_integration.rs      # SSH integration tests (feature-gated)
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

simse-tui/                  # Terminal UI (ratatui + crossterm + tokio, depends on simse-core directly)
  src/
    app.rs                  # App model (Elm Architecture: Model/Update/View)
    event_loop.rs           # TuiRuntime: bridges App to simse-core (ACP, tools, config, sessions)
    cli_args.rs             # CLI argument parsing
    config.rs               # Config loading (8 files, agents, skills, SIMSE.md)
    session_store.rs        # JSONL session persistence
    json_io.rs              # JSON/JSONL utilities
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
```

### CDN Worker

```tree
simse-cdn/                  # TypeScript — Cloudflare Worker at cdn.simse.dev
  src/
    index.ts                # Worker fetch handler (media, versioned downloads, latest redirect)
    types.ts                # Env interface (R2Bucket, KVNamespace)
    index.test.ts           # Integration tests (8 tests, @cloudflare/vitest-pool-workers)
    test-setup.ts           # R2/KV mock seeding for tests
  wrangler.toml             # R2 bucket (CDN_BUCKET) + KV namespace (VERSION_STORE) bindings
  vitest.config.ts          # Vitest workers pool config
```

**Routes:**
| Path | Behavior |
|------|----------|
| `GET /media/{file}` | Stream from R2, immutable cache |
| `GET /download/{version}/{os}/{arch}` | Stream binary from R2, immutable cache, Content-Disposition |
| `GET /download/latest/{os}/{arch}` | KV lookup → 301 redirect to versioned URL |
| `GET /health` | 200 OK |

**R2 key layout:** `media/{file}` and `releases/{os}/{arch}/{version}/{filename}`
**KV keys:** `latest:{os}-{arch}` → version string

### TypeScript Services (Cloudflare Workers)

```tree
simse-api/                  # API gateway — proxies to backend services
  src/
    index.ts                # Hono app, health + secrets middleware + gateway routes
    routes/gateway.ts       # Route map, auth validation, proxy helpers
    middleware/secrets.ts    # Cloudflare Secrets Store middleware
    types.ts                # Env (Queue, SecretsStore), ApiSecrets, ValidateResponse

simse-auth/                 # Auth service — users, sessions, teams, API keys
  src/
    index.ts                # Hono app entry
    schemas.ts              # Zod validation schemas
    types.ts                # Env (D1, Queue, SecretsStore)
    routes/
      auth.ts               # Register, login, 2FA, reset, verify
      users.ts              # Profile update, password change, delete
      teams.ts              # Team CRUD, invites, member roles
      api-keys.ts           # API key CRUD
    lib/
      db.ts                 # D1 query helpers
      password.ts           # Argon2 hashing
      session.ts            # Session token management
      token.ts              # Verification/reset token management
      api-key.ts            # API key generation + validation
      comms.ts              # Queue message helpers (emails, notifications)

simse-payments/             # Payments service — Stripe subscriptions, credits, usage
  src/
    index.ts                # Hono app entry
    types.ts                # Env (D1, SecretsStore)
    routes/
      checkout.ts           # Stripe checkout session creation
      subscriptions.ts      # Plan management
      credits.ts            # Credit balance + top-ups
      customers.ts          # Stripe customer sync
      portal.ts             # Stripe billing portal
      webhooks.ts           # Stripe webhook handler
    lib/
      stripe.ts             # Stripe client wrapper
      db.ts                 # D1 query helpers
      mailer.ts             # Email trigger helpers
    middleware/
      auth.ts               # Service-to-service auth (X-User-Id)
```

### Predictive Coding Engine

```tree
simse-predictive-coding/    # Pure Rust crate — predictive coding engine
  src/
    lib.rs                  # Module declarations
    main.rs                 # Binary entry point (JSON-RPC server)
    config.rs               # Model configuration
    encoder.rs              # Input encoding
    vocabulary.rs           # Token vocabulary management
    network.rs              # Neural network layers
    layer.rs                # Layer implementation
    predictor.rs            # Prediction engine
    trainer.rs              # Model training
    snapshot.rs             # Model snapshots
    persistence.rs          # Save/load persistence
    error.rs                # Error types
    protocol.rs             # JSON-RPC protocol types
    transport.rs            # NDJSON transport
    server.rs               # JSON-RPC dispatcher
```

### Key Patterns

- **Rust-first architecture**: All core logic is in Rust. TS packages are application/service layers (simse-cloud, simse-api, simse-auth, simse-payments, simse-cdn, simse-landing, simse-mailer).
- **JSON-RPC 2.0 / NDJSON stdio**: All Rust crates expose their APIs via JSON-RPC over newline-delimited JSON on stdin/stdout. Tracing/logs go to stderr.
- **Callback pattern**: Tools, hooks, chains, and loops registered from external callers use oneshot channels + JSON-RPC notifications for async callback execution.
- **CoreContext wiring**: `CoreContext` ties together EventBus, Logger, AppConfig, TaskList, HookSystem, SessionManager, ToolRegistry, and optional Library/VFS.
- **Error format**: `{ code: -32000, message: "...", data: { coreCode: "NOT_INITIALIZED" | "SESSION_NOT_FOUND" | ... } }`
- **Doom loop detection**: The agentic loop tracks consecutive identical tool calls. After `maxIdenticalToolCalls` (default 3), it fires callbacks and injects a system warning.
- **Tool output truncation**: `ToolRegistryOptions.maxOutputChars` (default 50,000) caps tool output. Per-tool override via `ToolDefinition.maxOutputChars`.
- **Session forking**: `SessionManager.fork(id)` clones conversation state, creates fresh event bus and new ID.
- **Structured compaction**: Auto-compaction requests 6 sections (Goal, Progress, Current State, Key Decisions, Relevant Files, Next Steps).
- **Arc<AtomicBool> for health flags**: Connection health shared between spawned reader tasks and main struct.
- **Backend trait abstraction**: Each engine crate (VFS, VSH, VNet) defines a backend trait (`FsBackend`, `ShellBackend`, `NetBackend`). LocalBackend wraps existing logic, SshBackend in simse-sandbox uses russh multiplexed SSH connections.

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
