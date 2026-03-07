# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Rust crate builds
bun run build:core           # cd simse-core && cargo build --release
bun run build:cli            # cd simse-cli && cargo build --release

# Rust tests
cd simse-core && cargo test                          # All tests (default features: engine, adaptive, sandbox, remote)
cd simse-core && cargo test --features engine         # Engine module tests only
cd simse-core && cargo test --features adaptive       # Adaptive module tests only
cd simse-core && cargo test --features sandbox        # Sandbox module tests only
cd simse-core && cargo test --features remote         # Remote module tests only
cd simse-cli && cargo test                            # CLI tests (TUI + UI core)

# TypeScript lint (all TS services use Biome, paths inside simse-cloud/)
cd simse-cloud/simse-api && npm run lint       # API gateway lint
cd simse-cloud/simse-auth && npm run lint      # Auth service lint
cd simse-cloud/simse-payments && npm run lint  # Payments service lint
cd simse-cloud/simse-cdn && npm run lint       # CDN worker lint

# TypeScript tests
cd simse-cloud/simse-cdn && npm run test    # CDN worker tests (Vitest + @cloudflare/vitest-pool-workers)
cd simse-cloud/simse-mailer && npm run test # Mailer tests (Vitest + @cloudflare/vitest-pool-workers)
```

## Architecture

simse is a modular pipeline framework for orchestrating multi-step AI workflows. It connects to AI backends via **ACP** (Agent Client Protocol), exposes tools via **MCP** (Model Context Protocol), and provides a file-backed **adaptive store** (vector store + PCN) with SIMD-accelerated distance metrics, HNSW indexing, vector quantization, MMR/RRF fusion, cataloging, deduplication, and recommendation.

The entire core is implemented in **Rust** in a single `simse-core` crate with feature-gated modules (`engine`, `adaptive`, `sandbox`, `remote`).

### Repository Layout

```tree
simse-core/                 # [submodule] Pure Rust crate — orchestration + feature-gated engine/adaptive/sandbox/remote
simse-cli/                  # [submodule] Pure Rust crate — TUI + UI core (ratatui, Elm Architecture)
simse-cloud/                # [submodule] TypeScript — all Cloudflare Workers (nested submodules)
  simse-app/                #   SaaS web app + relay (React Router + Cloudflare Pages + Durable Objects)
  simse-api/                #   API gateway (Cloudflare Worker, proxies to backend services)
  simse-auth/               #   Auth service (Cloudflare Worker, D1, users/sessions/teams/API keys)
  simse-payments/           #   Payments service (Cloudflare Worker, Stripe)
  simse-bi/                 #   Analytics + audit (Cloudflare Worker, D1, Queues, Analytics Engine)
  simse-cdn/                #   CDN worker (R2 + KV, Cloudflare Worker)
  simse-landing/            #   Landing page (React Router + Cloudflare)
  simse-mailer/             #   Email templates + notifications
  simse-status/             #   Status page (React Router v7 + Cloudflare Pages + D1 + Cron)
simse-brand/                # [submodule] Brand assets (logos, screenshots, guidelines, copy)
```

### simse-core Module Layout

```tree
simse-core/
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
    prompts/                # SystemPromptBuilder, environment, provider
    agentic_loop.rs         # run_agentic_loop: generate->parse->execute->repeat
    agent.rs                # Agent executor (dispatch steps)
    hooks.rs                # HookSystem: 6 hook types with chaining/blocking
    chain/                  # Chain execution (run_chain, ChainStep)
    tools/                  # ToolRegistry, builtin/host/subagent/delegation tools
    library/                # Library, Stacks, Shelf, Librarian, Registry, CirculationDesk (requires adaptive feature)
    server/                 # SessionManager with fork support
    utils/                  # retry, circuit_breaker, timeout

    # Feature-gated modules:
    engine/                 # [feature = "engine"] ACP + MCP + ML inference
      acp/                  # ACP client/server (uses agent-client-protocol SDK)
      mcp/                  # MCP client/server (stdio + HTTP transports)
      inference/            # Local ML inference (embeddings + text generation)
      models/               # Model implementations (BERT, Llama, NomicBERT, TEI, sampling)

    adaptive/               # [feature = "adaptive"] Vector store + PCN
      store.rs              # Store: core state manager (CRUD, search, indexing, persistence)
      distance.rs           # Distance metrics (Cosine, Euclidean, DotProduct, Manhattan) with SIMD acceleration
      vector_storage.rs     # SoA contiguous embedding storage for cache-friendly scans
      index.rs              # IndexBackend trait, FlatIndex (brute force), HnswIndex (approximate NN)
      quantization.rs       # Scalar (f32->u8, 4x) and Binary (sign-bit, 32x) vector quantization
      fusion.rs             # MMR diversity reranking, Reciprocal Rank Fusion for hybrid search
      persistence.rs        # Binary codec + gzip compression for entries/learning/graph state
      cataloging.rs         # TopicIndex, MetadataIndex, MagnitudeCache
      deduplication.rs      # Duplicate detection & clustering
      recommendation.rs     # Scoring with recency/frequency
      text_search.rs        # Exact/substring/fuzzy/regex/token search
      inverted_index.rs     # BM25 text search indexing
      topic_catalog.rs      # Hierarchical topic classification
      learning.rs           # Adaptive learning engine
      graph.rs              # Graph index with explicit/similarity/correlation edges
      pcn/                  # Predictive coding network subsystem

    sandbox/                # [feature = "sandbox"] VFS + VSH + VNet
      vfs_store.rs          # VirtualFs: in-memory filesystem (vfs:// backend)
      vfs_disk.rs           # DiskFs: real filesystem with shadow history (file:// backend)
      vfs_backend.rs        # FsImpl enum { Local(DiskFs), Ssh(SshFs) }
      vsh_shell.rs          # VirtualShell: session management, env, aliases, history
      vsh_executor.rs       # Command execution via tokio::process with timeouts
      vsh_backend.rs        # ShellImpl enum { Local(LocalShell), Ssh(SshShell) }
      vnet_network.rs       # VirtualNetwork: core logic, mock HTTP, sandbox, metrics
      vnet_backend.rs       # NetImpl enum { Local(LocalNet), Ssh(SshNet) }
      ssh/                  # SSH module: SshPool, SshFs, SshShell, SshNet

    remote/                 # [feature = "remote"] Remote access
      auth.rs               # Auth client (login/logout, token validation via simse-api)
      tunnel.rs             # WebSocket tunnel client (connect, reconnect, multiplex)
      router.rs             # Local router (forward relayed requests to simse-core)
      heartbeat.rs          # Backoff config, keepalive ping interval

  tests/                    # Integration tests
```

### simse-core Features

| Feature | Default | Deps | Description |
|---------|---------|------|-------------|
| `engine` | yes | candle-*, hf-hub, tokenizers, agent-client-protocol, reqwest | ACP + MCP + ML inference |
| `adaptive` | yes | hnsw_rs, rayon, base64, flate2 | Vector store + PCN + library |
| `sandbox` | yes | russh, russh-sftp, sha2, reqwest, base64 | VFS + VSH + VNet |
| `remote` | yes | tokio-tungstenite, reqwest | Remote access tunneling |
| `cuda`/`metal`/`mkl`/`accelerate` | no | (implies engine) | Hardware-accelerated inference |

### CLI Crate (simse-cli)

`simse-cli/` is a submodule containing the merged TUI + UI core (formerly `simse-tui` + `simse-ui-core`). Elm Architecture with ratatui + crossterm + tokio. Includes platform-agnostic UI logic, state management, keybindings, command registry, markdown rendering, tool call display, permission dialogs, and settings overlays.

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
| `GET /download/latest/{os}/{arch}` | KV lookup -> 301 redirect to versioned URL |
| `GET /health` | 200 OK |

**R2 key layout:** `media/{file}` and `releases/{os}/{arch}/{version}/{filename}`
**KV keys:** `latest:{os}-{arch}` -> version string

### TypeScript Services (Cloudflare Workers)

All TS services live in the `simse-cloud/` submodule as nested submodules. Each is a separate repo:

| Service | Description |
|---------|-------------|
| `simse-cloud/simse-app` | SaaS web app + relay (React Router + Cloudflare Pages + Durable Objects) |
| `simse-cloud/simse-api` | API gateway — route proxying, auth validation, secrets middleware |
| `simse-cloud/simse-auth` | Auth service — users, sessions, teams, API keys (D1) |
| `simse-cloud/simse-payments` | Payments service — Stripe subscriptions, credits, usage (D1) |
| `simse-cloud/simse-bi` | Analytics + audit — centralized queue consumer (D1 + Analytics Engine) |
| `simse-cloud/simse-cdn` | CDN worker — media and versioned downloads (R2 + KV) |
| `simse-cloud/simse-landing` | Landing page (React Router + Cloudflare) |
| `simse-cloud/simse-mailer` | Email templates + notifications |
| `simse-cloud/simse-status` | Status page (React Router v7 + Cloudflare Pages + D1 + Cron) |

### Key Patterns

- **Rust-first architecture**: All core logic is in Rust (`simse-core` submodule). TS services are in `simse-cloud/` submodule. CLI is `simse-cli/` submodule.
- **Single-crate Rust core**: `simse-core` has four feature-gated modules (`engine`, `adaptive`, `sandbox`, `remote`). Consumers link `simse-core` as a library dependency.
- **Callback pattern**: Tools, hooks, chains, and loops registered from external callers use oneshot channels for async callback execution.
- **CoreContext wiring**: `CoreContext` ties together EventBus, Logger, AppConfig, TaskList, HookSystem, SessionManager, ToolRegistry, and optional Library (requires `adaptive` feature).
- **Error format**: `{ code: -32000, message: "...", data: { coreCode: "NOT_INITIALIZED" | "SESSION_NOT_FOUND" | ... } }`
- **Doom loop detection**: The agentic loop tracks consecutive identical tool calls. After `maxIdenticalToolCalls` (default 3), it fires callbacks and injects a system warning.
- **Tool output truncation**: `ToolRegistryOptions.maxOutputChars` (default 50,000) caps tool output. Per-tool override via `ToolDefinition.maxOutputChars`.
- **Session forking**: `SessionManager.fork(id)` clones conversation state, creates fresh event bus and new ID.
- **Structured compaction**: Auto-compaction requests 6 sections (Goal, Progress, Current State, Key Decisions, Relevant Files, Next Steps).
- **Arc<AtomicBool> for health flags**: Connection health shared between spawned reader tasks and main struct.
- **Backend enum dispatch**: The sandbox module uses enum dispatch (`FsImpl`, `ShellImpl`, `NetImpl`) instead of trait objects. Each enum has `Local` and `Ssh` variants. Local wraps in-crate logic, Ssh uses russh multiplexed SSH connections.
- **Centralized Analytics**: All services produce analytics/audit events to per-service `ANALYTICS_QUEUE` queues consumed by `simse-bi`, which is the sole writer to the Analytics Engine dataset and D1 audit store. Data points include method, path, status, latency, userId, geo (country/city/continent), userAgent, and cfRay.

### ACP Protocol

The ACP engine (`simse-core/src/engine/acp/`) exposes the [Agent Client Protocol](https://agentclientprotocol.com).

**Protocol details:**
- **Protocol version**: 1
- **Field naming**: camelCase throughout (`sessionId`, `stopReason`, `agentInfo`)
- **Session lifecycle**: `session/new` -> `session/prompt` -> `session/update` notifications -> response
- **Permission flow**: Agent sends `session/request_permission`; client responds with `allow_once`/`allow_always`/`reject_once`/`reject_always`
- **Tool call lifecycle**: `tool_call` -> `tool_call_update` (in_progress) -> `tool_call_update` (completed)
- **Timeout defaults**: `timeoutMs` = 60s, `initTimeoutMs` = 30s (both overridable)

### MCP Protocol

The MCP engine (`simse-core/src/engine/mcp/`) implements the [Model Context Protocol](https://modelcontextprotocol.io).

**Protocol details:**
- **Client**: Connects to external MCP servers via stdio or HTTP transport
- **Server**: Exposes simse capabilities as MCP tools
- **Features**: Logging, list-changed notifications, completions, roots, resource templates

### Adaptive Store System

The adaptive store is implemented entirely within `simse-core` (requires `adaptive` feature):

**Storage layer** (`simse-core/src/adaptive/`) — all vector + PCN operations:
- Store (entries, CRUD, search), distance metrics (SIMD-accelerated), SoA vector storage, index backends (Flat/HNSW), quantization (Scalar/Binary), fusion (MMR/RRF), persistence, cataloging, deduplication, recommendation, text search, BM25, topic classification, adaptive learning, context formatting, graph index, predictive coding network (`pcn/`)

**Orchestration layer** (`simse-core/src/library/`) — high-level operations:
- Library, Stacks, Shelf, Librarian, LibrarianRegistry, CirculationDesk

### Formatting

- **Biome** (not ESLint/Prettier): tabs for indentation, single quotes, semicolons (for TS packages)
- **Rust**: standard `rustfmt` defaults, clippy with `-D warnings`
