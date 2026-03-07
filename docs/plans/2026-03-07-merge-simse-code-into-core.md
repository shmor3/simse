# Merge simse-code into simse-core Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Merge all 4 simse-code subcrates (engine, adaptive, sandbox, remote) into simse-core as a pure library with feature-gated subsystems.

**Architecture:** Each subcrate becomes a top-level module inside simse-core (`engine/`, `adaptive/`, `sandbox/`, `remote/`). JSON-RPC transport layers are dropped. Dependencies are gated behind Cargo features. Internal imports change from `use crate::X` to `use crate::<subcrate>::X` in moved files.

**Tech Stack:** Rust, Cargo workspaces, feature flags

---

### Task 1: Verify baseline

**Step 1: Run existing tests**

```bash
cd /home/dev/simse && cargo test --workspace 2>&1 | tail -20
```

Expected: All simse-core, simse-ui-core, simse-tui tests pass.

**Step 2: Check subcrate tests individually**

```bash
cd /home/dev/simse/simse-code/adaptive && cargo test 2>&1 | tail -5
cd /home/dev/simse/simse-code/engine && cargo test 2>&1 | tail -5
cd /home/dev/simse/simse-code/sandbox && cargo test 2>&1 | tail -5
cd /home/dev/simse/simse-code/remote && cargo test 2>&1 | tail -5
```

Note any pre-existing failures. The `simse-resilience` path dep in engine may cause a build failure — that's expected and will be resolved during the merge.

---

### Task 2: Update Cargo.toml files

**Files:**
- Modify: `/home/dev/simse/Cargo.toml`
- Modify: `/home/dev/simse/simse-core/Cargo.toml`

**Step 1: Update root workspace Cargo.toml**

Remove the `exclude` block. Add new workspace dependencies that were previously local to subcrates:

```toml
[workspace]
resolver = "2"
members = [
    "simse-ui-core",
    "simse-tui",
    "simse-core",
]

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "MIT"
rust-version = "1.85"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
regex = "1"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
flate2 = "1"
dirs = "6"
ratatui = "0.30"
crossterm = { version = "0.29", features = ["event-stream"] }
futures = "0.3"
tui-textarea = "0.7"
clap = { version = "4", features = ["derive"] }
```

**Step 2: Rewrite simse-core/Cargo.toml**

Remove the `[[bin]]` section. Remove `simse-engine` and `simse-adaptive-engine` path deps. Add all subcrate dependencies with `optional = true` where feature-gated.

```toml
[package]
name = "simse-core"
version.workspace = true
edition.workspace = true

[lib]
name = "simse_core"
path = "src/lib.rs"

[features]
default = ["engine", "adaptive", "sandbox", "remote"]

engine = [
    "dep:candle-core", "dep:candle-nn", "dep:candle-transformers",
    "dep:hf-hub", "dep:tokenizers", "dep:agent-client-protocol",
    "dep:ureq", "dep:anyhow",
]
adaptive = ["dep:hnsw_rs", "dep:rayon", "dep:base64", "dep:flate2"]
sandbox = ["dep:russh", "dep:russh-sftp", "dep:sha2", "dep:reqwest"]
remote = ["dep:tokio-tungstenite"]

cuda = ["engine", "candle-core/cuda", "candle-nn/cuda", "candle-transformers/cuda"]
metal = ["engine", "candle-core/metal", "candle-nn/metal", "candle-transformers/metal"]
mkl = ["engine", "candle-core/mkl", "candle-nn/mkl", "candle-transformers/mkl"]
accelerate = ["engine", "candle-core/accelerate", "candle-nn/accelerate", "candle-transformers/accelerate"]
embed-weights = []

[dependencies]
# Always-on deps (used by core + multiple subcrates)
im = { version = "15", features = ["serde"] }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { version = "1", features = ["raw_value"] }
thiserror.workspace = true
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
uuid = { workspace = true }
futures.workspace = true
async-trait = "0.1"
tokio-util = { version = "0.7", features = ["compat"] }
regex.workspace = true
chrono.workspace = true
rand = "0.10"
glob = "0.3"

# Engine-gated deps
candle-core = { version = "0.9", optional = true }
candle-nn = { version = "0.9", optional = true }
candle-transformers = { version = "0.9", optional = true }
hf-hub = { version = "0.5", features = ["tokio"], optional = true }
tokenizers = { version = "0.22", default-features = false, features = ["onig"], optional = true }
agent-client-protocol = { git = "https://github.com/agentclientprotocol/rust-sdk", features = ["unstable"], optional = true }
ureq = { version = "3", features = ["json"], optional = true }
anyhow = { version = "1", optional = true }
clap = { version = "4", features = ["derive"] }

# Adaptive-gated deps
hnsw_rs = { version = "0.3", optional = true }
rayon = { version = "1", optional = true }
base64 = { version = "0.22", optional = true }
flate2 = { version = "1", optional = true }

# Sandbox-gated deps
russh = { version = "0.57", default-features = false, features = ["ring", "rsa", "async-trait"], optional = true }
russh-sftp = { version = "2.1", optional = true }
sha2 = { version = "0.10", optional = true }
reqwest = { version = "0.13", features = ["json"], optional = true }

# Remote-gated deps
tokio-tungstenite = { version = "0.28", features = ["native-tls"], optional = true }

[dev-dependencies]
tempfile = "3"
tokio-test = "0.4"
criterion = { version = "0.8", features = ["html_reports"] }
```

**Step 3: Verify Cargo.toml parses**

```bash
cd /home/dev/simse/simse-core && cargo metadata --no-deps 2>&1 | head -5
```

Expected: No parse errors.

**Step 4: Commit**

```bash
git add Cargo.toml simse-core/Cargo.toml
git commit -m "chore: prepare Cargo.toml for simse-code merge"
```

---

### Task 3: Move adaptive into simse-core

**Step 1: Create directory and copy source files**

```bash
mkdir -p /home/dev/simse/simse-core/src/adaptive/pcn

# Copy all source files except main.rs, server.rs, transport.rs
for f in store distance vector_storage index quantization fusion persistence \
         cataloging deduplication recommendation text_search inverted_index \
         topic_catalog learning query_dsl context_format graph text_cache \
         types error; do
    cp simse-code/adaptive/src/${f}.rs simse-core/src/adaptive/
done

# Copy protocol.rs (has PCN param types — some may be used internally)
# Actually drop it — only used by server.rs
# cp simse-code/adaptive/src/protocol.rs simse-core/src/adaptive/

# Copy PCN submodule
for f in mod config encoder vocabulary network layer predictor trainer snapshot; do
    cp simse-code/adaptive/src/pcn/${f}.rs simse-core/src/adaptive/pcn/
done
```

**Step 2: Create adaptive/mod.rs**

Write `/home/dev/simse/simse-core/src/adaptive/mod.rs`:

```rust
pub mod error;
pub mod types;
pub mod distance;
pub mod fusion;
pub mod text_search;
pub mod inverted_index;
pub mod cataloging;
pub mod deduplication;
pub mod recommendation;
pub mod learning;
pub mod topic_catalog;
pub mod query_dsl;
pub mod text_cache;
pub mod persistence;
pub mod quantization;
pub mod context_format;
pub mod graph;
pub mod vector_storage;
pub mod index;
pub mod store;
pub mod pcn;
```

**Step 3: Fix internal imports in all moved files**

Every `use crate::X` in adaptive files must become `use crate::adaptive::X`:

```bash
find simse-core/src/adaptive -name "*.rs" -exec sed -i 's/use crate::/use crate::adaptive::/g' {} +
```

Also fix any `crate::` references in type paths (not just `use` statements):

```bash
find simse-core/src/adaptive -name "*.rs" -exec sed -i 's/crate::adaptive::adaptive::/crate::adaptive::/g' {} +
```

The second command fixes any double-nesting that might occur if a file already had `crate::adaptive::` (unlikely but safe).

**Step 4: Copy tests and benchmarks**

```bash
# Integration tests
cp simse-code/adaptive/tests/integration.rs simse-core/tests/adaptive_integration.rs
cp simse-code/adaptive/tests/pcn_integration.rs simse-core/tests/adaptive_pcn_integration.rs

# Benchmarks
mkdir -p simse-core/benches
cp simse-code/adaptive/benches/index_benchmarks.rs simse-core/benches/
cp simse-code/adaptive/benches/pcn_benchmarks.rs simse-core/benches/
```

Fix test imports:

```bash
sed -i 's/use simse_adaptive_engine::/use simse_core::adaptive::/g' simse-core/tests/adaptive_integration.rs
sed -i 's/use simse_adaptive_engine::/use simse_core::adaptive::/g' simse-core/tests/adaptive_pcn_integration.rs
sed -i 's/use simse_adaptive_engine::/use simse_core::adaptive::/g' simse-core/benches/index_benchmarks.rs
sed -i 's/use simse_adaptive_engine::/use simse_core::adaptive::/g' simse-core/benches/pcn_benchmarks.rs
```

Add bench targets to Cargo.toml if needed:
```toml
[[bench]]
name = "pcn_benchmarks"
harness = false

[[bench]]
name = "index_benchmarks"
harness = false
```

**Step 5: Commit**

```bash
git add simse-core/src/adaptive/ simse-core/tests/adaptive_*.rs simse-core/benches/
git commit -m "refactor: move adaptive engine into simse-core"
```

---

### Task 4: Move engine into simse-core

**Step 1: Create directories and copy source files**

```bash
mkdir -p /home/dev/simse/simse-core/src/engine/{acp,mcp,inference,models}

# Root engine files (skip main.rs, server.rs, transport.rs)
for f in config error protocol; do
    cp simse-code/engine/src/${f}.rs simse-core/src/engine/
done

# ACP module (all files)
for f in mod client connection error permission resilience rpc_types server stream; do
    cp simse-code/engine/src/acp/${f}.rs simse-core/src/engine/acp/
done

# MCP module (all files — mcp has its own rpc_server/transport for MCP protocol, not top-level binary)
for f in mod client error http_transport mcp_server protocol rpc_server rpc_transport stdio_transport; do
    cp simse-code/engine/src/mcp/${f}.rs simse-core/src/engine/mcp/
done

# Inference module
for f in mod embedding generation; do
    cp simse-code/engine/src/inference/${f}.rs simse-core/src/engine/inference/
done

# Models module
for f in mod bert llama nomic_bert sampling tei tokenizer weights; do
    cp simse-code/engine/src/models/${f}.rs simse-core/src/engine/models/
done
```

**Step 2: Create engine/mod.rs**

Write `/home/dev/simse/simse-core/src/engine/mod.rs`:

```rust
pub mod acp;
pub mod config;
pub mod error;
pub mod inference;
pub mod mcp;
pub mod models;
pub mod protocol;
```

**Step 3: Fix internal imports**

```bash
find simse-core/src/engine -name "*.rs" -exec sed -i 's/use crate::/use crate::engine::/g' {} +
# Fix any double-nesting
find simse-core/src/engine -name "*.rs" -exec sed -i 's/crate::engine::engine::/crate::engine::/g' {} +
```

Also remove any `use simse_resilience::` imports and inline the functionality. Check `acp/resilience.rs` — it likely already contains the full implementation. If it imports from `simse_resilience`, remove those imports (the dep no longer exists).

```bash
grep -rn "simse_resilience" simse-core/src/engine/
```

If found, replace with local implementations or remove.

**Step 4: Copy tests**

```bash
cp simse-code/engine/tests/acp_integration.rs simse-core/tests/engine_acp_integration.rs
cp simse-code/engine/tests/mcp_integration.rs simse-core/tests/engine_mcp_integration.rs

sed -i 's/use simse_engine::/use simse_core::engine::/g' simse-core/tests/engine_acp_integration.rs
sed -i 's/use simse_engine::/use simse_core::engine::/g' simse-core/tests/engine_mcp_integration.rs
```

**Step 5: Commit**

```bash
git add simse-core/src/engine/ simse-core/tests/engine_*.rs
git commit -m "refactor: move engine into simse-core"
```

---

### Task 5: Move sandbox into simse-core

**Step 1: Create directories and copy source files**

```bash
mkdir -p /home/dev/simse/simse-core/src/sandbox/ssh

# All source files except main.rs, server.rs, transport.rs, protocol.rs
for f in error config sandbox \
         vfs_store vfs_disk vfs_diff vfs_glob vfs_search vfs_path vfs_types vfs_backend \
         vsh_shell vsh_executor vsh_sandbox vsh_backend \
         vnet_network vnet_sandbox vnet_mock_store vnet_session vnet_types vnet_local vnet_backend; do
    cp simse-code/sandbox/src/${f}.rs simse-core/src/sandbox/
done

# SSH submodule
for f in mod pool channel fs shell net; do
    cp simse-code/sandbox/src/ssh/${f}.rs simse-core/src/sandbox/ssh/
done
```

**Step 2: Create sandbox/mod.rs**

Write `/home/dev/simse/simse-core/src/sandbox/mod.rs`:

```rust
pub mod config;
pub mod error;
pub mod sandbox;
pub mod ssh;
pub mod vfs_diff;
pub mod vfs_disk;
pub mod vfs_glob;
pub mod vfs_path;
pub mod vfs_search;
pub mod vfs_store;
pub mod vfs_types;
pub mod vnet_mock_store;
pub mod vnet_network;
pub mod vnet_sandbox;
pub mod vnet_session;
pub mod vnet_local;
pub mod vnet_types;
pub mod vsh_executor;
pub mod vsh_sandbox;
pub mod vsh_shell;
pub mod vfs_backend;
pub mod vsh_backend;
pub mod vnet_backend;
```

**Step 3: Fix internal imports**

```bash
find simse-core/src/sandbox -name "*.rs" -exec sed -i 's/use crate::/use crate::sandbox::/g' {} +
find simse-core/src/sandbox -name "*.rs" -exec sed -i 's/crate::sandbox::sandbox::/crate::sandbox::/g' {} +
```

**Step 4: Copy tests**

```bash
cp simse-code/sandbox/tests/integration.rs simse-core/tests/sandbox_integration.rs
cp simse-code/sandbox/tests/vfs.rs simse-core/tests/sandbox_vfs.rs
cp simse-code/sandbox/tests/vsh.rs simse-core/tests/sandbox_vsh.rs
cp simse-code/sandbox/tests/vnet.rs simse-core/tests/sandbox_vnet.rs
cp simse-code/sandbox/tests/ssh_integration.rs simse-core/tests/sandbox_ssh_integration.rs

for f in simse-core/tests/sandbox_*.rs; do
    sed -i 's/use simse_sandbox_engine::/use simse_core::sandbox::/g' "$f"
done
```

**Step 5: Commit**

```bash
git add simse-core/src/sandbox/ simse-core/tests/sandbox_*.rs
git commit -m "refactor: move sandbox into simse-core"
```

---

### Task 6: Move remote into simse-core

**Step 1: Create directory and copy source files**

```bash
mkdir -p /home/dev/simse/simse-core/src/remote

# All source files except main.rs, server.rs, transport.rs
for f in error auth tunnel router heartbeat; do
    cp simse-code/remote/src/${f}.rs simse-core/src/remote/
done

# Keep protocol.rs — auth.rs and tunnel.rs likely use its param/result types
cp simse-code/remote/src/protocol.rs simse-core/src/remote/
```

**Step 2: Create remote/mod.rs**

Write `/home/dev/simse/simse-core/src/remote/mod.rs`:

```rust
pub mod error;
pub mod protocol;
pub mod auth;
pub mod tunnel;
pub mod router;
pub mod heartbeat;
```

**Step 3: Fix internal imports**

```bash
find simse-core/src/remote -name "*.rs" -exec sed -i 's/use crate::/use crate::remote::/g' {} +
find simse-core/src/remote -name "*.rs" -exec sed -i 's/crate::remote::remote::/crate::remote::/g' {} +
```

**Step 4: Copy tests**

```bash
cp simse-code/remote/tests/integration.rs simse-core/tests/remote_integration.rs
sed -i 's/use simse_remote_engine::/use simse_core::remote::/g' simse-core/tests/remote_integration.rs
```

**Step 5: Commit**

```bash
git add simse-core/src/remote/ simse-core/tests/remote_integration.rs
git commit -m "refactor: move remote into simse-core"
```

---

### Task 7: Update simse-core internals

**Files:**
- Modify: `simse-core/src/lib.rs`
- Modify: `simse-core/src/error.rs`
- Modify: `simse-core/src/library/mod.rs`
- Modify: `simse-core/src/library/library.rs`
- Modify: `simse-core/src/library/shelf.rs`
- Modify: `simse-core/src/library/services.rs`
- Modify: `simse-core/src/library/query_dsl.rs`
- Modify: `simse-core/src/library/prompt_inject.rs`
- Delete: `simse-core/src/main.rs`
- Delete: `simse-core/src/rpc_server.rs`
- Delete: `simse-core/src/rpc_protocol.rs`
- Delete: `simse-core/src/rpc_transport.rs`

**Step 1: Rewrite lib.rs**

```rust
pub mod agent;
pub mod agentic_loop;
pub mod chain;
pub mod config;
pub mod context;
pub mod conversation;
pub mod error;
pub mod events;
pub mod hooks;
pub mod logger;
pub mod prompts;
pub mod server;
pub mod tasks;
pub mod tools;
pub mod utils;

#[cfg(feature = "engine")]
pub mod engine;

#[cfg(feature = "adaptive")]
pub mod adaptive;

#[cfg(feature = "sandbox")]
pub mod sandbox;

#[cfg(feature = "remote")]
pub mod remote;

#[cfg(feature = "adaptive")]
pub mod library;

// Re-export key types at the crate root for convenience
pub use config::AppConfig;
pub use context::CoreContext;
pub use conversation::Conversation;
pub use error::SimseError;
pub use events::EventBus;
pub use logger::Logger;
pub use tasks::TaskList;
```

Note: `library` module depends on `adaptive` types, so it's gated behind the `adaptive` feature. The old `pub use simse_engine;` re-export is removed.

**Step 2: Update error.rs**

Find the three engine/adaptive error variants (around lines 355-363) and wrap them:

```rust
#[cfg(feature = "engine")]
#[error("ACP error: {0}")]
Acp(#[from] crate::engine::acp::error::AcpError),

#[cfg(feature = "engine")]
#[error("MCP engine error: {0}")]
McpEngine(#[from] crate::engine::mcp::error::McpError),

#[cfg(feature = "adaptive")]
#[error("Adaptive error: {0}")]
Adaptive(#[from] crate::adaptive::error::AdaptiveError),
```

Also wrap the corresponding `code()` and `is_retriable()` match arms with `#[cfg]`.

**Step 3: Update library imports**

In `library/library.rs`, change:
```rust
// Before
use simse_adaptive_engine::store::{StoreConfig, Store};
use simse_adaptive_engine::types::{...};

// After
use crate::adaptive::store::{StoreConfig, Store};
use crate::adaptive::types::{...};
```

In `library/shelf.rs`:
```rust
// Before
use simse_adaptive_engine::types::{Entry, Lookup};
// After
use crate::adaptive::types::{Entry, Lookup};
```

In `library/services.rs`:
```rust
// Before
use simse_adaptive_engine::context_format::{format_context, ContextFormatOptions};
// After
use crate::adaptive::context_format::{format_context, ContextFormatOptions};
```

In `library/query_dsl.rs`:
```rust
// Before
pub use simse_adaptive_engine::query_dsl::{parse_query, ParsedQuery, TextSearchParsed};
// After
pub use crate::adaptive::query_dsl::{parse_query, ParsedQuery, TextSearchParsed};
```

In `library/prompt_inject.rs`:
```rust
// Before
pub use simse_adaptive_engine::context_format::{format_age, format_context, ContextFormatOptions};
// After
pub use crate::adaptive::context_format::{format_age, format_context, ContextFormatOptions};
```

In `library/mod.rs`, update the doc comment:
```rust
//! Library orchestration layer wrapping `crate::adaptive::store::Store`.
```

**Step 4: Delete JSON-RPC transport files**

```bash
rm simse-core/src/main.rs
rm simse-core/src/rpc_server.rs
rm simse-core/src/rpc_protocol.rs
rm simse-core/src/rpc_transport.rs
```

**Step 5: Update existing simse-core tests**

Fix imports in existing test files:

```bash
sed -i 's/simse_engine::acp::error::AcpError/simse_core::engine::acp::error::AcpError/g' simse-core/tests/error.rs
sed -i 's/simse_engine::mcp::error::McpError/simse_core::engine::mcp::error::McpError/g' simse-core/tests/error.rs
sed -i 's/simse_adaptive_engine::/simse_core::adaptive::/g' simse-core/tests/library.rs
sed -i 's/simse_adaptive_engine::/simse_core::adaptive::/g' simse-core/tests/librarian.rs
sed -i 's/simse_engine::/simse_core::engine::/g' simse-core/tests/error.rs
```

**Step 6: Commit**

```bash
git add -A simse-core/src/ simse-core/tests/
git commit -m "refactor: update simse-core internals for merged crates"
```

---

### Task 8: Build and fix

**Step 1: Attempt full build**

```bash
cd /home/dev/simse/simse-core && cargo check --all-features 2>&1 | head -80
```

**Step 2: Fix compiler errors iteratively**

Common issues to expect:
- **Double-nested paths**: `crate::adaptive::adaptive::X` from the sed replacement — fix to `crate::adaptive::X`
- **Missing `#[cfg]` guards**: code that references engine/adaptive types without feature gates
- **Visibility**: items that were `pub` at the crate root but now need `pub` at the module level
- **`crate::` vs `super::` confusion** in deeply nested modules (e.g., `pcn/network.rs` using `crate::adaptive::distance` — correct)
- **Protocol types**: if any non-transport code referenced types from the dropped protocol.rs files, add them back or move them to the appropriate module
- **`simse_resilience` references** in engine/acp/ — remove and use local acp/resilience.rs

Run `cargo check --all-features` repeatedly until clean.

**Step 3: Attempt tests**

```bash
cd /home/dev/simse && cargo test --workspace 2>&1 | tail -40
```

Fix any remaining test compilation errors. Integration tests that tested JSON-RPC server behavior (spawning binaries, stdin/stdout) should be removed or rewritten to call Rust APIs directly.

**Step 4: Commit**

```bash
git add -A
git commit -m "fix: resolve compiler errors from crate merge"
```

---

### Task 9: Delete simse-code and clean up

**Step 1: Delete simse-code directory**

```bash
rm -rf /home/dev/simse/simse-code
```

**Step 2: Check for any remaining references**

```bash
grep -r "simse.code\|simse_engine\b\|simse_adaptive_engine\|simse_sandbox_engine\|simse_remote_engine\|simse.resilience" \
    --include="*.rs" --include="*.toml" --include="*.md" \
    /home/dev/simse/simse-core/ /home/dev/simse/simse-tui/ /home/dev/simse/simse-ui-core/ /home/dev/simse/Cargo.toml
```

Fix any remaining references found.

**Step 3: Final build + test**

```bash
cd /home/dev/simse && cargo test --workspace 2>&1 | tail -20
```

All tests must pass.

**Step 4: Test feature subsets**

```bash
# Core only (no subcrate features)
cargo check -p simse-core --no-default-features 2>&1 | tail -5

# Individual features
cargo check -p simse-core --no-default-features --features engine 2>&1 | tail -5
cargo check -p simse-core --no-default-features --features adaptive 2>&1 | tail -5
cargo check -p simse-core --no-default-features --features sandbox 2>&1 | tail -5
cargo check -p simse-core --no-default-features --features remote 2>&1 | tail -5
```

All must compile cleanly.

**Step 5: Commit**

```bash
git add -A
git commit -m "refactor: delete simse-code, merge complete"
```

---

### Task 10: Update CLAUDE.md

**Files:**
- Modify: `/home/dev/simse/CLAUDE.md`

Update the architecture section, repository layout, build commands, and test commands to reflect the merged structure. Key changes:

- Remove `simse-code/` from repository layout
- Add `simse-core/src/engine/`, `simse-core/src/adaptive/`, `simse-core/src/sandbox/`, `simse-core/src/remote/` to the simse-core module layout
- Update build commands (no more separate `build:adaptive-engine`, `build:acp-engine`, etc.)
- Update test commands to use `cd simse-core && cargo test --features engine`, etc.
- Remove JSON-RPC method tables for subcrates (no longer exposed as servers)
- Update the "Key Patterns" section

**Step 1: Make the edits**

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md for merged crate structure"
```
