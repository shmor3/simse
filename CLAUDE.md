# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Rust crate builds
bun run build:adaptive-engine # cd simse-adaptive && cargo build --release
bun run build:acp-engine     # cd simse-acp && cargo build --release
bun run build:mcp-engine     # cd simse-mcp && cargo build --release
bun run build:core           # cd simse-core && cargo build --release
bun run build:tui            # cd simse-tui && cargo build --release
bun run build:sandbox-engine # cd simse-sandbox && cargo build --release
bun run build:remote-engine  # cd simse-remote && cargo build --release

# Rust tests
cd simse-adaptive && cargo test # Rust adaptive engine tests
cd simse-acp && cargo test     # Rust ACP engine tests
cd simse-mcp && cargo test     # Rust MCP engine tests
cd simse-core && cargo test    # Rust core orchestration tests
cd simse-ui-core && cargo test # Rust UI core tests
cd simse-sandbox && cargo test # Rust sandbox engine tests
cd simse-remote && cargo test  # Rust remote engine tests
cd simse-tui && cargo test     # Rust TUI tests (unit + integration)

# TypeScript lint (all TS services use Biome)
cd simse-api && npm run lint       # API gateway lint
cd simse-auth && npm run lint      # Auth service lint
cd simse-payments && npm run lint  # Payments service lint
cd simse-cdn && npm run lint       # CDN worker lint

# TypeScript tests
cd simse-cdn && npm run test    # CDN worker tests (Vitest + @cloudflare/vitest-pool-workers)
cd simse-mailer && npm run test # Mailer tests (Vitest + @cloudflare/vitest-pool-workers)
```

## Architecture

simse is a modular pipeline framework for orchestrating multi-step AI workflows. It connects to AI backends via **ACP** (Agent Client Protocol), exposes tools via **MCP** (Model Context Protocol), and provides a file-backed **adaptive store** (vector store + PCN) with compression, cataloging, deduplication, recommendation, and summarization.

The entire core is implemented in **Rust**. Each crate is a standalone binary communicating over JSON-RPC 2.0 / NDJSON stdio.

### Repository Layout

```tree
simse-core/                 # Pure Rust crate — orchestration library + JSON-RPC binary server
simse-adaptive/             # Pure Rust crate — adaptive engine (vector store + PCN, JSON-RPC over stdio)
simse-acp/                  # Pure Rust crate — ACP engine (JSON-RPC over stdio)
simse-mcp/                  # Pure Rust crate — MCP engine (JSON-RPC over stdio)
simse-sandbox/              # Pure Rust crate — unified sandbox engine (VFS + VSH + VNet, JSON-RPC over stdio)
simse-ui-core/              # Pure Rust crate — platform-agnostic UI logic (no I/O)
simse-tui/                  # Pure Rust crate — terminal UI (ratatui, Elm Architecture)
simse-remote/               # Pure Rust crate — remote access engine (JSON-RPC over stdio)
simse-engine/               # Pure Rust crate — core engine
simse-analytics/            # TypeScript — Analytics + audit service (Cloudflare Worker, D1, Queues, Analytics Engine)
simse-cdn/                  # TypeScript — CDN worker (R2 + KV, Cloudflare Worker)
simse-app/                  # TypeScript — SaaS web app + relay (React Router + Cloudflare Pages + Durable Objects)
simse-api/                  # TypeScript — API gateway (Cloudflare Worker, proxies to backend services)
simse-auth/                 # TypeScript — Auth service (Cloudflare Worker, D1, users/sessions/teams/API keys)
simse-payments/             # TypeScript — Payments service (Cloudflare Worker, Stripe)
simse-landing/              # TypeScript — Landing page (React Router + Cloudflare)
simse-mailer/               # TypeScript — Email templates + notifications
simse-status/               # TypeScript — Status page (React Router v7 + Cloudflare Pages + D1 + Cron)
simse-brand/                # Brand assets (logos, screenshots, guidelines, copy)
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

### simse-remote JSON-RPC Methods

| Domain | Methods |
|--------|---------|
| `auth/` | `login`, `logout`, `status` |
| `tunnel/` | `connect`, `disconnect`, `status` |
| `remote/` | `health` |

### simse-adaptive JSON-RPC Methods

| Domain | Methods |
|--------|---------|
| `store/` | `add`, `get`, `remove`, `search`, `searchWithOptions`, `list`, `clear`, `save`, `load`, `setIndexStrategy`, `setQuantization`, `getIndexStats` |

### Other Rust Crates

```tree
simse-adaptive/             # Pure Rust crate — adaptive engine (vector store + PCN, JSON-RPC over stdio)
  src/                      # Key deps: rayon (parallel search), hnsw_rs (approximate NN)
    store.rs                # Store: core state manager (CRUD, search, indexing, persistence)
    distance.rs             # Distance metrics (Cosine, Euclidean, DotProduct, Manhattan) with SIMD acceleration (AVX2, NEON)
    vector_storage.rs       # SoA contiguous embedding storage for cache-friendly scans
    index.rs                # IndexBackend trait, FlatIndex (brute force), HnswIndex (approximate NN via hnsw_rs)
    quantization.rs         # Scalar (f32→u8, 4x) and Binary (sign-bit, 32x) vector quantization
    fusion.rs               # MMR diversity reranking, Reciprocal Rank Fusion for hybrid search
    persistence.rs          # Binary codec + gzip compression for entries/learning/graph state
    cataloging.rs           # TopicIndex, MetadataIndex, MagnitudeCache
    deduplication.rs        # Duplicate detection & clustering
    recommendation.rs       # Scoring with recency/frequency
    text_search.rs          # Exact/substring/fuzzy/regex/token search
    inverted_index.rs       # BM25 text search indexing
    topic_catalog.rs        # Hierarchical topic classification
    learning.rs             # Adaptive learning engine
    query_dsl.rs            # Query DSL parsing
    context_format.rs       # Context formatting for LLM prompts (XML/natural)
    graph.rs                # Graph index with explicit/similarity/correlation edges
    text_cache.rs           # LRU text cache with entry-count + byte-budget limits
    pcn/                    # Predictive coding network subsystem
      config.rs             # PCN model configuration
      encoder.rs            # Input encoding (embeddings → PCN input)
      vocabulary.rs         # Token vocabulary management
      network.rs            # Predictive coding network layers
      layer.rs              # Layer implementation
      predictor.rs          # Prediction engine (read-only, concurrent)
      trainer.rs            # Model training (async background worker)
      snapshot.rs           # Model snapshots (serializable weights)

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

simse-sandbox/              # Pure Rust crate — unified sandbox engine (VFS + VSH + VNet merged)
  src/
    lib.rs                  # Module declarations, re-exports
    main.rs                 # Binary entry point (simse-sandbox-engine)
    error.rs                # SandboxError enum (SANDBOX_ prefix, all VFS/VSH/VNet variants)
    protocol.rs             # JSON-RPC param/result types
    transport.rs            # NdjsonTransport (same pattern as other crates)
    server.rs               # SandboxServer: JSON-RPC dispatcher (63 methods across 7 domains)
    sandbox.rs              # Sandbox: unified orchestrator, backend switching
    config.rs               # BackendConfig, SshConfig, SshAuth
    vfs_store.rs            # VirtualFs: in-memory filesystem (vfs:// backend)
    vfs_disk.rs             # DiskFs: real filesystem with shadow history (file:// backend)
    vfs_diff.rs             # Myers diff algorithm
    vfs_glob.rs             # Glob pattern matching
    vfs_search.rs           # File search implementation
    vfs_path.rs             # Path utilities, VfsLimits
    vfs_types.rs            # Shared VFS types: DirEntry, HistoryEntry, ReadFileResult, etc.
    vfs_backend.rs          # FsImpl enum { Local(DiskFs), Ssh(SshFs) }
    vsh_shell.rs            # VirtualShell: session management, env, aliases, history
    vsh_executor.rs         # Command execution via tokio::process with timeouts
    vsh_sandbox.rs          # SandboxConfig: path validation, command filtering
    vsh_backend.rs          # ShellImpl enum { Local(LocalShell), Ssh(SshShell) }
    vnet_network.rs         # VirtualNetwork: core logic, mock HTTP, sandbox, metrics
    vnet_sandbox.rs         # NetSandboxConfig: host/port/protocol allowlist validation
    vnet_mock_store.rs      # MockStore: mock registry + glob pattern matching
    vnet_session.rs         # SessionManager: persistent connection tracking (WS, TCP)
    vnet_types.rs           # Shared VNet types: HttpResponseResult, MetricsResult, etc.
    vnet_local.rs           # LocalNet: reqwest HTTP + DNS resolution
    vnet_backend.rs         # NetImpl enum { Local(LocalNet), Ssh(SshNet) }
    ssh/
      mod.rs                # SSH module root
      pool.rs               # SshPool: multiplexed russh connection manager
      channel.rs            # ExecOutput, channel read with timeout
      fs.rs                 # SshFs: SFTP-backed filesystem
      shell.rs              # SshShell: exec channel command execution
      net.rs                # SshNet: exec channel network operations (curl/getent)
  tests/
    integration.rs          # 10 integration tests (local backend)
    ssh_integration.rs      # SSH integration tests (feature-gated)
    vfs.rs                  # 60 VFS unit tests (VirtualFs + DiskFs)
    vsh.rs                  # 24 VSH unit tests (VirtualShell)
    vnet.rs                 # 42 VNet unit tests (VirtualNetwork)

simse-remote/              # Pure Rust crate — remote access engine
  src/
    error.rs               # RemoteError enum with REMOTE_ code prefixes
    protocol.rs            # JSON-RPC request/response types (7 methods)
    transport.rs           # NdjsonTransport for JSON-RPC over stdio
    auth.rs                # Auth client (login/logout, token validation via simse-api)
    tunnel.rs              # WebSocket tunnel client (connect, reconnect, multiplex)
    router.rs              # Local router (forward relayed requests to simse-core)
    heartbeat.rs           # Backoff config, keepalive ping interval
    server.rs              # RemoteServer: 7-method JSON-RPC dispatch
  tests/
    integration.rs         # 8 integration tests (JSON-RPC over stdio)
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
    event_loop.rs           # TuiRuntime: main event loop
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
simse-analytics/            # Analytics + audit — centralized queue consumer
  src/
    index.ts                # Hono app, queue handler (datapoint + audit), HTTP routes (/health, /audit/:userId)
    types.ts                # Env (D1, AnalyticsEngine), DatapointMessage, AuditMessage
  migrations/
    0001_initial.sql        # audit_events table + indexes
  wrangler.toml             # D1, Analytics Engine, 8 queue consumers (one per service)

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

simse-cloud/                # SaaS web app + relay (React Router + Cloudflare Pages + Durable Objects)
  worker.ts                 # CF Pages worker entry (React Router + relay routing)
  app/
    relay/
      tunnel.ts             # TunnelSession Durable Object (WebSocket pair management)
      handler.ts            # Relay request handler (/ws/tunnel, /ws/client, /tunnels)
  wrangler.toml             # Pages + Durable Object + Analytics Engine bindings
```

### Key Patterns

- **Rust-first architecture**: All core logic is in Rust. TS packages are application/service layers (simse-app (includes relay), simse-api, simse-auth, simse-payments, simse-cdn, simse-landing, simse-mailer).
- **JSON-RPC 2.0 / NDJSON stdio**: All Rust crates expose their APIs via JSON-RPC over newline-delimited JSON on stdin/stdout. Tracing/logs go to stderr.
- **Callback pattern**: Tools, hooks, chains, and loops registered from external callers use oneshot channels + JSON-RPC notifications for async callback execution.
- **CoreContext wiring**: `CoreContext` ties together EventBus, Logger, AppConfig, TaskList, HookSystem, SessionManager, ToolRegistry, and optional Library.
- **Error format**: `{ code: -32000, message: "...", data: { coreCode: "NOT_INITIALIZED" | "SESSION_NOT_FOUND" | ... } }`
- **Doom loop detection**: The agentic loop tracks consecutive identical tool calls. After `maxIdenticalToolCalls` (default 3), it fires callbacks and injects a system warning.
- **Tool output truncation**: `ToolRegistryOptions.maxOutputChars` (default 50,000) caps tool output. Per-tool override via `ToolDefinition.maxOutputChars`.
- **Session forking**: `SessionManager.fork(id)` clones conversation state, creates fresh event bus and new ID.
- **Structured compaction**: Auto-compaction requests 6 sections (Goal, Progress, Current State, Key Decisions, Relevant Files, Next Steps).
- **Arc<AtomicBool> for health flags**: Connection health shared between spawned reader tasks and main struct.
- **Backend enum dispatch**: simse-sandbox uses enum dispatch (`FsImpl`, `ShellImpl`, `NetImpl`) instead of trait objects. Each enum has `Local` and `Ssh` variants. Local wraps in-crate logic, Ssh uses russh multiplexed SSH connections.
- **Centralized Analytics**: All 8 services produce analytics/audit events to per-service `ANALYTICS_QUEUE` queues consumed by `simse-analytics`, which is the sole writer to the Analytics Engine dataset (`simse-analytics`) and D1 audit store. Data points include method, path, status, latency, userId, geo (country/city/continent), userAgent, and cfRay. Audit events are persisted in D1 `audit_events` table and also written to Analytics Engine with `indexes: ['audit']`.

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

### Adaptive Store System

The adaptive store has two layers: the storage engine in Rust (`simse-adaptive/`), with orchestration in `simse-core/src/library/`.

**Rust engine** (`simse-adaptive/src/`) — all vector + PCN operations via JSON-RPC:
- Store (entries, CRUD, search), distance metrics (SIMD-accelerated), SoA vector storage, index backends (Flat/HNSW), quantization (Scalar/Binary), fusion (MMR/RRF), persistence, cataloging, deduplication, recommendation, text search, BM25, topic classification, adaptive learning, context formatting, graph index, predictive coding network (`pcn/`)

**simse-core orchestration layer** (`simse-core/src/library/`) — high-level operations:
- Library, Stacks, Shelf, Librarian, LibrarianRegistry, CirculationDesk

### Formatting

- **Biome** (not ESLint/Prettier): tabs for indentation, single quotes, semicolons (for TS packages)
- **Rust**: standard `rustfmt` defaults, clippy with `-D warnings`
