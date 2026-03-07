# simse

A modular pipeline framework for orchestrating multi-step AI workflows.

The core is implemented in **Rust** with feature-gated modules. Cloud services run on **Cloudflare Workers** in TypeScript. The repository is organized as git submodules — each has its own README, CLAUDE.md, and LICENSE.

## Submodules

| Submodule | Language | Description |
|-----------|----------|-------------|
| [`simse-core`](https://github.com/shmor3/simse-core) | Rust | Orchestration, ACP/MCP engine, adaptive memory, sandbox, remote access |
| [`simse-cli`](https://github.com/shmor3/simse-cli) | Rust | Terminal interface (ratatui, Elm Architecture) |
| [`simse-cloud`](https://github.com/shmor3/simse-cloud) | TypeScript | Cloud services (app, api, auth, payments, bi, cdn, landing, mailer, status) |
| [`simse-brand`](https://github.com/shmor3/simse-brand) | — | Brand assets (logos, design system, guidelines, copy) |

## Getting Started

```bash
git clone --recurse-submodules https://github.com/shmor3/simse.git
cd simse
cargo build --release          # Build Rust crates
cd simse-cloud && npm install  # Install TS dependencies
```

See each submodule's README for detailed build, test, and development instructions.

## License

[Elastic License 2.0 (ELv2)](LICENSE)
