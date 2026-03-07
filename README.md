# simse

A modular pipeline framework for orchestrating multi-step AI workflows. Connects to AI backends via [ACP](https://agentclientprotocol.com), exposes tools via [MCP](https://modelcontextprotocol.io), runs local inference with [Candle](https://github.com/huggingface/candle), and provides file-backed adaptive memory with vector search, a predictive coding network, and graph indexing.

The entire core is implemented in **Rust** in a single `simse-core` crate with feature-gated modules. The cloud platform runs on **Cloudflare Workers** in TypeScript.

## Features

- **Unified Engine** — ACP client/server, MCP client/server, and local ML inference (embeddings + text generation) in a single crate. CUDA, Metal, MKL, and Accelerate backends for hardware-accelerated inference.
- **Orchestration** — Agentic loop (generate → parse → execute → repeat), composable chain pipelines, subagent spawning, dependency-aware task tracking, and structured auto-compaction.
- **Adaptive Memory** — File-backed vector store with SIMD-accelerated distance metrics (AVX2/NEON), HNSW approximate nearest neighbor indexing, scalar/binary vector quantization, MMR diversity reranking, RRF hybrid fusion, BM25 text search, topic classification, deduplication, recommendation scoring, graph indexing, and a predictive coding network.
- **Sandbox** — Virtual filesystem (in-memory + disk-backed with history), virtual shell (command execution with filtering/timeouts), and virtual network (mock HTTP, allowlists, session tracking). Local and SSH backends via enum dispatch.
- **Remote Access** — Token-based auth, WebSocket tunnel with reconnect/multiplexing, and local request routing.
- **Terminal UI** — Elm Architecture TUI built with ratatui. Markdown rendering with syntax highlighting, tool call display with diffs, /command autocomplete, @file mentions, permission dialogs, and settings overlays.
- **Cloud Platform** — SaaS web app with relay (React Router + Durable Objects), API gateway, auth (users/sessions/teams/API keys), payments (Stripe), analytics + audit, CDN (R2 + KV), landing page, email notifications, and status page.

## Repository Layout

This is a monorepo using git submodules:

| Submodule | Description |
|-----------|-------------|
| `simse-core` | Rust — orchestration library with feature-gated modules (engine, adaptive, sandbox, remote) |
| `simse-cli` | Rust — terminal UI + UI core (ratatui, Elm Architecture) |
| `simse-cloud` | TypeScript — all Cloudflare Workers (nested submodules: app, api, auth, payments, bi, cdn, landing, mailer, status) |
| `simse-brand` | Brand assets — logos, screenshots, design system, guidelines, copy |

### simse-core Features

| Feature | Default | Description |
|---------|---------|-------------|
| `engine` | yes | ACP + MCP + ML inference (Candle) |
| `adaptive` | yes | Vector store + PCN (SIMD, HNSW, quantization, MMR/RRF) |
| `sandbox` | yes | VFS + VSH + VNet (local + SSH backends) |
| `remote` | yes | Remote access tunneling (WebSocket) |
| `cuda`/`metal`/`mkl`/`accelerate` | no | Hardware-accelerated inference |

### simse-cloud Services

| Service | Description |
|---------|-------------|
| `simse-app` | SaaS web app + relay (React Router + Cloudflare Pages + Durable Objects) |
| `simse-api` | API gateway — route proxying, auth validation, secrets middleware |
| `simse-auth` | Auth service — users, sessions, teams, API keys (D1) |
| `simse-payments` | Payments service — Stripe subscriptions, credits, usage (D1) |
| `simse-bi` | Analytics + audit — centralized queue consumer (D1 + Analytics Engine) |
| `simse-cdn` | CDN worker — media and versioned downloads (R2 + KV) |
| `simse-landing` | Landing page (React Router + Cloudflare) |
| `simse-mailer` | Email templates + notifications |
| `simse-status` | Status page (React Router + D1 + Cron) |

## Architecture

```
                          ┌─────────────┐
                          │  simse-cli  │
                          │  (ratatui)  │
                          └──────┬──────┘
                                 │
                          ┌──────┴──────┐
                          │ simse-core  │
                          │             │
                          ├─────────────┤
                          │ orchestration│
                          │ engine (ACP │
                          │  MCP, ML)   │
                          │ adaptive    │
                          │ sandbox     │
                          │ remote      │
                          └──────┬──────┘
                                 │
                          ┌──────┴──────┐
                          │ simse-cloud │
                          │  (TS/CF)    │
                          └─────────────┘
```

## Requirements

- **Rust** >= 1.85 (2024 edition)
- **Bun** >= 1.0 (for TypeScript services)
- **Node.js** >= 20 (for TypeScript services)

## Building

### Rust

```bash
cd simse-core && cargo build --release                    # All features
cd simse-core && cargo build --release --features cuda    # NVIDIA GPU
cd simse-core && cargo build --release --features metal   # Apple GPU
cd simse-core && cargo build --release --features mkl     # Intel CPU
cd simse-cli && cargo build --release                     # CLI
```

### TypeScript Services

```bash
cd simse-cloud/simse-app && npm install && npm run build
cd simse-cloud/simse-api && npm install && npm run build
cd simse-cloud/simse-auth && npm install && npm run build
cd simse-cloud/simse-payments && npm install && npm run build
cd simse-cloud/simse-bi && npm install && npm run build
cd simse-cloud/simse-cdn && npm install && npm run build
cd simse-cloud/simse-landing && npm install && npm run build
cd simse-cloud/simse-mailer && npm install && npm run build
cd simse-cloud/simse-status && npm install && npm run build
```

## Testing

### Rust

```bash
cd simse-core && cargo test                          # All tests
cd simse-core && cargo test --features engine        # Engine module only
cd simse-core && cargo test --features adaptive      # Adaptive module only
cd simse-core && cargo test --features sandbox       # Sandbox module only
cd simse-core && cargo test --features remote        # Remote module only
cd simse-cli && cargo test                           # CLI tests
```

### TypeScript

```bash
cd simse-cloud/simse-cdn && npm run test
cd simse-cloud/simse-mailer && npm run test
```

### Linting

All TypeScript services use [Biome](https://biomejs.dev) (tabs, single quotes, semicolons):

```bash
cd simse-cloud/simse-api && npm run lint
cd simse-cloud/simse-auth && npm run lint
cd simse-cloud/simse-payments && npm run lint
cd simse-cloud/simse-cdn && npm run lint
```

Rust crates use standard `rustfmt` and `clippy -D warnings`.

## Key Patterns

- **Rust-first** — All core logic is in Rust. TypeScript is only used for application/service layers on Cloudflare Workers.
- **Single-crate core** — `simse-core` uses feature flags (`engine`, `adaptive`, `sandbox`, `remote`) instead of separate crates.
- **Backend enum dispatch** — Sandbox uses enum dispatch (`FsImpl`, `ShellImpl`, `NetImpl`) with `Local` and `Ssh` variants instead of trait objects.
- **Callback pattern** — Tools, hooks, chains, and loops use oneshot channels for async callback execution.
- **Centralized analytics** — All services produce events to per-service queues consumed by simse-bi, the sole writer to Analytics Engine and D1 audit store.

## License

[Elastic License 2.0 (ELv2)](LICENSE)
