# simse

A modular pipeline framework for orchestrating multi-step AI workflows. Connects to AI backends via [ACP](https://agentclientprotocol.com), exposes tools via [MCP](https://modelcontextprotocol.io), runs local inference with [Candle](https://github.com/huggingface/candle), and provides file-backed adaptive memory with vector search, a predictive coding network, and graph indexing.

The entire core is implemented in **Rust**. Each crate is a standalone binary communicating over **JSON-RPC 2.0 / NDJSON stdio**. The cloud platform runs on **Cloudflare Workers** in TypeScript.

## Features

- **Unified Engine** — ACP client/server, MCP client/server, and local ML inference (embeddings + text generation) in a single crate. CUDA, Metal, MKL, and Accelerate backends for hardware-accelerated inference.
- **Orchestration** — Agentic loop (generate → parse → execute → repeat), composable chain pipelines, subagent spawning, dependency-aware task tracking, and structured auto-compaction.
- **Adaptive Memory** — File-backed vector store with cosine similarity, BM25 text search, topic classification, deduplication, recommendation scoring, graph indexing, and a predictive coding network.
- **Sandbox** — Virtual filesystem (in-memory + disk-backed with history), virtual shell (command execution with filtering/timeouts), and virtual network (mock HTTP, allowlists, session tracking). Local and SSH backends via enum dispatch.
- **Remote Access** — Token-based auth, WebSocket tunnel with reconnect/multiplexing, and local request routing.
- **Terminal UI** — Elm Architecture TUI built with ratatui. Markdown rendering with syntax highlighting, tool call display with diffs, /command autocomplete, @file mentions, permission dialogs, and settings overlays.
- **Cloud Platform** — SaaS web app with relay (React Router + Durable Objects), API gateway, auth (users/sessions/teams/API keys), payments (Stripe), analytics + audit, CDN (R2 + KV), landing page, email notifications, and status page.

## Repository Layout

### Rust Crates

| Crate | Path | Description |
|-------|------|-------------|
| `simse-engine` | `simse-code/engine` | Unified engine — ACP, MCP, and ML inference over JSON-RPC 2.0 / NDJSON stdio |
| `simse-core` | `simse-core` | Orchestration library — agentic loop, chains, tools, hooks, sessions, library |
| `simse-adaptive` | `simse-code/adaptive` | Adaptive engine — vector store, PCN, cataloging, deduplication, graph index |
| `simse-sandbox` | `simse-code/sandbox` | Sandbox engine — VFS + VSH + VNet, local and SSH backends |
| `simse-remote` | `simse-code/remote` | Remote access engine — auth, WebSocket tunnel, request routing |
| `simse-ui-core` | `simse-ui-core` | Platform-agnostic UI logic — state, input, commands, config (no I/O) |
| `simse-tui` | `simse-tui` | Terminal UI — ratatui + crossterm, Elm Architecture |

### TypeScript Services (Cloudflare Workers)

| Package | Description |
|---------|-------------|
| `simse-app` | SaaS web app + relay (React Router + Cloudflare Pages + Durable Objects) |
| `simse-api` | API gateway — route proxying, auth validation, secrets middleware |
| `simse-auth` | Auth service — users, sessions, teams, API keys (D1) |
| `simse-payments` | Payments service — Stripe subscriptions, credits, usage (D1) |
| `simse-analytics` | Analytics + audit — centralized queue consumer (D1 + Analytics Engine) |
| `simse-cdn` | CDN worker — media and versioned downloads (R2 + KV) |
| `simse-landing` | Landing page (React Router + Cloudflare) |
| `simse-mailer` | Email templates + notifications |
| `simse-status` | Status page (React Router + D1 + Cron) |

### Other

| Directory | Description |
|-----------|-------------|
| `simse-brand` | Brand assets — logos, screenshots, design system, guidelines, copy |

## Architecture

```
                          ┌─────────────┐
                          │  simse-tui  │
                          │  (ratatui)  │
                          └──────┬──────┘
                                 │
                          ┌──────┴──────┐
                          │ simse-ui-core│
                          └──────┬──────┘
                                 │
                          ┌──────┴──────┐
                          │ simse-core  │
                          │(orchestration)│
                          └──┬───┬───┬──┘
                 ┌───────────┤   │   ├───────────┐
                 │           │   │   │           │
          ┌──────┴──────┐   │   │   │   ┌───────┴───────┐
          │simse-engine │   │   │   │   │simse-adaptive │
          │ ACP+MCP+ML  │   │   │   │   │ vector+PCN    │
          └─────────────┘   │   │   │   └───────────────┘
                            │   │   │
                 ┌──────────┘   │   └──────────┐
                 │              │              │
          ┌──────┴──────┐ ┌────┴─────┐ ┌──────┴──────┐
          │simse-sandbox│ │simse-    │ │  (cloud     │
          │ VFS+VSH+VNet│ │ remote   │ │  services)  │
          └─────────────┘ └──────────┘ └─────────────┘
```

All Rust crates communicate over JSON-RPC 2.0 / NDJSON stdio. Tracing and logs go to stderr.

## Requirements

- **Rust** >= 1.85 (2024 edition)
- **Bun** >= 1.0 (for TypeScript services)
- **Node.js** >= 20 (for TypeScript services)

## Building

### Rust Crates

```bash
# Via bun scripts
bun run build:core             # simse-core
bun run build:adaptive-engine  # simse-adaptive
bun run build:acp-engine       # simse-engine (ACP binary)
bun run build:mcp-engine       # simse-engine (MCP binary)
bun run build:sandbox-engine   # simse-sandbox
bun run build:remote-engine    # simse-remote
bun run build:tui              # simse-tui

# Or directly with cargo
cd simse-code/engine && cargo build --release
cd simse-core && cargo build --release

# Hardware-accelerated inference
cd simse-code/engine && cargo build --release --features cuda    # NVIDIA GPU
cd simse-code/engine && cargo build --release --features metal   # Apple GPU
cd simse-code/engine && cargo build --release --features mkl     # Intel CPU
```

### TypeScript Services

```bash
cd simse-app && npm install && npm run build
cd simse-api && npm install && npm run build
cd simse-auth && npm install && npm run build
cd simse-payments && npm install && npm run build
cd simse-cdn && npm install && npm run build
cd simse-analytics && npm install && npm run build
cd simse-landing && npm install && npm run build
cd simse-mailer && npm install && npm run build
cd simse-status && npm install && npm run build
```

## Testing

### Rust

```bash
cd simse-core && cargo test
cd simse-code/engine && cargo test
cd simse-code/adaptive && cargo test
cd simse-code/sandbox && cargo test
cd simse-code/remote && cargo test
cd simse-ui-core && cargo test
cd simse-tui && cargo test
```

### TypeScript

```bash
cd simse-cdn && npm run test
cd simse-mailer && npm run test
```

### Linting

All TypeScript services use [Biome](https://biomejs.dev) (tabs, single quotes, semicolons):

```bash
cd simse-api && npm run lint
cd simse-auth && npm run lint
cd simse-payments && npm run lint
cd simse-cdn && npm run lint
```

Rust crates use standard `rustfmt` and `clippy -D warnings`.

## Key Patterns

- **Rust-first** — All core logic is in Rust. TypeScript is only used for application/service layers on Cloudflare Workers.
- **JSON-RPC 2.0 / NDJSON stdio** — Every Rust crate exposes its API as JSON-RPC methods over newline-delimited JSON on stdin/stdout.
- **Backend enum dispatch** — simse-sandbox uses enum dispatch (`FsImpl`, `ShellImpl`, `NetImpl`) with `Local` and `Ssh` variants instead of trait objects.
- **Callback pattern** — Tools, hooks, chains, and loops use oneshot channels + JSON-RPC notifications for async callback execution.
- **Centralized analytics** — All services produce events to per-service queues consumed by simse-analytics, the sole writer to Analytics Engine and D1 audit store.

## License

[Elastic License 2.0 (ELv2)](LICENSE)
