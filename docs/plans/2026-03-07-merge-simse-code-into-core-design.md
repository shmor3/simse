# Merge simse-code into simse-core as a pure library

**Date:** 2026-03-07
**Status:** Approved

## Goal

Merge all 4 simse-code subcrates (engine, adaptive, sandbox, remote) into simse-core. Make simse-core a pure library with no binaries. Drop all JSON-RPC transport layers — consumers call Rust APIs directly.

## Module Structure

```
simse-core/src/
  lib.rs                    # Crate root — module declarations, re-exports. No main.rs.

  # Existing simse-core modules (unchanged)
  context.rs, error.rs, config.rs, logger.rs, events.rs,
  conversation.rs, tasks.rs, agentic_loop.rs, agent.rs, hooks.rs,
  prompts/, chain/, tools/, library/, server/, utils/

  # Absorbed from simse-code/engine
  engine/
    mod.rs
    acp/                    # ACP client/server
    mcp/                    # MCP client/server
    inference/              # Local ML inference
    models/                 # BERT, Llama, NomicBERT, etc.

  # Absorbed from simse-code/adaptive
  adaptive/
    mod.rs
    store.rs, distance.rs, vector_storage.rs, index.rs,
    quantization.rs, fusion.rs, persistence.rs, cataloging.rs,
    deduplication.rs, recommendation.rs, text_search.rs,
    inverted_index.rs, topic_catalog.rs, learning.rs,
    query_dsl.rs, context_format.rs, graph.rs, text_cache.rs,
    pcn/

  # Absorbed from simse-code/sandbox
  sandbox/
    mod.rs
    vfs_store.rs, vfs_disk.rs, vfs_diff.rs, vfs_glob.rs, ...
    vsh_shell.rs, vsh_executor.rs, vsh_sandbox.rs, ...
    vnet_network.rs, vnet_sandbox.rs, ...
    ssh/

  # Absorbed from simse-code/remote
  remote/
    mod.rs
    auth.rs, tunnel.rs, router.rs, heartbeat.rs
```

## Dropped Files

All JSON-RPC transport layers are removed:
- `main.rs`, `rpc_server.rs`, `rpc_protocol.rs`, `rpc_transport.rs` from simse-core
- `main.rs`, `server.rs`, `protocol.rs`, `transport.rs` from each simse-code subcrate
- `bin/acp.rs`, `bin/mcp.rs` from simse-code/engine

## Feature Flags

```toml
[features]
default = ["engine", "adaptive", "sandbox", "remote"]

engine   = ["dep:candle-core", "dep:candle-nn", "dep:candle-transformers",
            "dep:hf-hub", "dep:tokenizers", "dep:agent-client-protocol",
            "dep:ureq", "dep:anyhow"]
adaptive = ["dep:hnsw_rs", "dep:rayon", "dep:base64", "dep:flate2"]
sandbox  = ["dep:russh", "dep:russh-sftp", "dep:sha2"]
remote   = ["dep:tokio-tungstenite"]

cuda       = ["engine", "candle-core/cuda", "candle-nn/cuda", "candle-transformers/cuda"]
metal      = ["engine", "candle-core/metal", "candle-nn/metal", "candle-transformers/metal"]
mkl        = ["engine", "candle-core/mkl", "candle-nn/mkl", "candle-transformers/mkl"]
accelerate = ["engine", "candle-core/accelerate", "candle-nn/accelerate", "candle-transformers/accelerate"]
```

Modules are conditionally compiled with `#[cfg(feature = "...")]`.

Shared deps (serde, tokio, uuid, tracing, reqwest) stay unconditional. Only heavy/specialized deps get gated.

Existing simse-core code that references engine/adaptive types (`error.rs`, `library/`) gets `#[cfg]` guards.

## Consumer Impact

**simse-tui / simse-ui-core:**
- Dependency line unchanged (default features include everything)
- Can opt into specific features: `default-features = false, features = ["engine", "adaptive"]`

**Import path changes:**
```rust
// Before
use simse_core::simse_engine::acp::...;
use simse_adaptive_engine::store::Store;

// After
use simse_core::engine::acp::...;
use simse_core::adaptive::store::Store;
```

## Deletions

- `simse-code/` directory (all 4 subcrates)
- `simse-resilience/` (only used by engine — inline or drop)
- `exclude` block in root `Cargo.toml`

## Workspace Cargo.toml

Shared deps like `hnsw_rs`, `rayon`, `candle-*` move into `[workspace.dependencies]`. No more `exclude` for simse-code subcrates.
