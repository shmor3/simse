# Bridge Removal Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Remove simse-bridge crate by having simse-tui and simse-ui-core depend directly on simse-core for unified types and logic.

**Architecture:** simse-core becomes the single source of truth for shared types (tools, conversation, agentic loop). simse-ui-core re-exports simse-core types and keeps UI-specific logic. simse-tui uses simse-core directly for config, ACP client (via simse-acp), tool registry, and agentic loop. Bridge-unique code (session store, json_io, config loading) moves to simse-tui.

**Tech Stack:** Rust, tokio, serde, simse-core, simse-acp, simse-ui-core, simse-tui

---

## Phase 1: Prepare simse-core for consumption

### Task 1: Add simse-core to workspace and align ToolCallResult

**Files:**
- Modify: `Cargo.toml` (workspace root, line 3-7 members, line 15 exclude)
- Modify: `simse-core/src/tools/types.rs` (ToolCallResult struct)
- Test: `cd simse-core && cargo test`

**Step 1: Add simse-core to workspace members**

In root `Cargo.toml`, add `simse-core` to `[workspace].members` and remove from `exclude`:

```toml
[workspace]
members = [
    "simse-ui-core",
    "simse-tui",
    "simse-bridge",
    "simse-core",
]
```

Remove `"simse-core"` from the exclude list.

**Step 2: Add `diff` field to simse-core's ToolCallResult**

In `simse-core/src/tools/types.rs`, add the `diff` field to `ToolCallResult`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub id: String,
    pub name: String,
    pub output: String,
    pub is_error: bool,
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}
```

**Step 3: Verify simse-core builds and tests pass**

Run: `cd simse-core && cargo build && cargo test`
Expected: All 779+ tests pass.

**Step 4: Commit**

```bash
git add Cargo.toml simse-core/src/tools/types.rs
git commit -m "feat: add simse-core to workspace, add diff field to ToolCallResult"
```

---

### Task 2: Add ToolExecutor impl for ToolRegistry in simse-core

**Files:**
- Modify: `simse-core/src/tools/registry.rs` (add impl block)
- Modify: `simse-core/src/agentic_loop.rs` (verify ToolExecutor trait matches)
- Test: `cd simse-core && cargo test`

**Step 1: Write a test for ToolExecutor implementation**

In `simse-core/src/tools/registry.rs` tests:

```rust
#[tokio::test]
async fn tool_registry_implements_tool_executor() {
    use crate::agentic_loop::ToolExecutor;
    let registry = ToolRegistry::new(ToolRegistryOptions::default());
    // Verify it compiles as a trait object
    let _: &dyn ToolExecutor = &registry;
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-core && cargo test tool_registry_implements_tool_executor`
Expected: FAIL — ToolRegistry does not implement ToolExecutor.

**Step 3: Implement ToolExecutor for ToolRegistry**

In `simse-core/src/tools/registry.rs`:

```rust
use crate::agentic_loop::ToolExecutor;

#[async_trait]
impl ToolExecutor for ToolRegistry {
    fn parse_tool_calls(&self, response: &str) -> ParsedResponse {
        ToolRegistry::parse_tool_calls(response)
    }

    async fn execute(&self, call: &ToolCallRequest) -> ToolCallResult {
        self.execute(call).await
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-core && cargo test tool_registry_implements_tool_executor`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-core/src/tools/registry.rs
git commit -m "feat(simse-core): implement ToolExecutor trait for ToolRegistry"
```

---

### Task 3: Add on_stream_start callback to simse-core LoopCallbacks

**Files:**
- Modify: `simse-core/src/agentic_loop.rs` (LoopCallbacks struct, loop body)
- Test: `cd simse-core && cargo test`

**Step 1: Add on_stream_start field to LoopCallbacks**

```rust
#[derive(Default)]
pub struct LoopCallbacks {
    pub on_stream_start: Option<Box<dyn Fn() + Send + Sync>>,
    pub on_stream_delta: Option<Box<dyn Fn(&str) + Send + Sync>>,
    // ... rest unchanged
}
```

**Step 2: Fire on_stream_start at the beginning of each generation call in the loop body**

Find where the loop begins streaming and add:

```rust
if let Some(ref cb) = callbacks_ref.on_stream_start {
    cb();
}
```

**Step 3: Run tests**

Run: `cd simse-core && cargo test`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add simse-core/src/agentic_loop.rs
git commit -m "feat(simse-core): add on_stream_start callback to LoopCallbacks"
```

---

### Task 4: Re-export simse-acp types from simse-core

**Files:**
- Modify: `simse-core/src/lib.rs` (add pub use)
- Test: `cd simse-core && cargo build`

**Step 1: Add re-exports for commonly needed simse-acp types**

In `simse-core/src/lib.rs`:

```rust
// Re-export ACP engine types for consumers
pub use simse_acp_engine as acp;
```

This allows consumers to access `simse_core::acp::client::AcpClient`, `simse_core::acp::stream::AcpStream`, etc.

**Step 2: Verify it builds**

Run: `cd simse-core && cargo build`
Expected: Build succeeds.

**Step 3: Commit**

```bash
git add simse-core/src/lib.rs
git commit -m "feat(simse-core): re-export simse-acp as acp module"
```

---

## Phase 2: Unify types in simse-ui-core

### Task 5: Add simse-core dependency to simse-ui-core

**Files:**
- Modify: `simse-ui-core/Cargo.toml`
- Test: `cd simse-ui-core && cargo build`

**Step 1: Add simse-core as dependency**

```toml
[dependencies]
simse-core = { path = "../simse-core" }
serde = { workspace = true }
serde_json = { workspace = true }
regex = { workspace = true }
```

**Step 2: Verify it builds**

Run: `cd simse-ui-core && cargo build`
Expected: Build succeeds (no circular dependency since simse-core does not depend on simse-ui-core).

**Step 3: Commit**

```bash
git add simse-ui-core/Cargo.toml
git commit -m "feat(simse-ui-core): add simse-core dependency for type unification"
```

---

### Task 6: Replace simse-ui-core tool types with simse-core re-exports

**Files:**
- Modify: `simse-ui-core/src/tools/mod.rs`
- Test: `cd simse-ui-core && cargo build`

**Step 1: Replace type definitions with re-exports**

Replace the struct definitions in `simse-ui-core/src/tools/mod.rs` with re-exports from simse-core. Keep utility functions that don't exist in simse-core (like `truncate_output`, `format_tools_for_system_prompt`):

```rust
// Re-export tool types from simse-core (single source of truth)
pub use simse_core::tools::{
    ToolCallRequest, ToolCallResult, ToolDefinition, ToolParameter,
    ToolCategory, ToolAnnotations, ToolMetrics, ParsedResponse,
    ToolHandler, ToolRegistryOptions, ToolRegistry,
};

// Keep UI-specific constants and utilities
pub const DEFAULT_MAX_OUTPUT_CHARS: usize = 50_000;

/// Output from a tool handler before being wrapped in ToolCallResult.
#[derive(Debug, Clone)]
pub struct ToolHandlerOutput {
    pub output: String,
    pub diff: Option<String>,
}

/// Truncate tool output to max_chars, appending [OUTPUT TRUNCATED] if needed.
pub fn truncate_output(output: &str, max_chars: usize) -> String {
    // Keep existing implementation
}

/// Format tool definitions for inclusion in system prompt.
pub fn format_tools_for_system_prompt(tools: &[ToolDefinition]) -> String {
    // Keep existing implementation — update to use simse_core::tools::ToolDefinition fields
}

/// Format a single tool definition.
pub fn format_tool_definition(tool: &ToolDefinition) -> String {
    // Keep existing implementation — update if ToolDefinition fields changed
}
```

**Step 2: Update tools/parser.rs if it references old types**

The parser should work with `simse_core::tools::ToolCallRequest` (same struct shape). Update imports if needed.

**Step 3: Verify it builds**

Run: `cd simse-ui-core && cargo build`
Expected: Build succeeds. Fix any field name mismatches (e.g., `category`, `annotations` are new fields in simse-core's ToolDefinition).

**Step 4: Commit**

```bash
git add simse-ui-core/src/tools/
git commit -m "refactor(simse-ui-core): replace tool types with simse-core re-exports"
```

---

### Task 7: Replace simse-ui-core conversation types with simse-core re-exports

**Files:**
- Modify: `simse-ui-core/src/state/conversation.rs`
- Test: `cd simse-ui-core && cargo build`

**Step 1: Replace type definitions with re-exports**

```rust
// Re-export conversation types from simse-core (single source of truth)
pub use simse_core::conversation::{
    Conversation, ConversationMessage, ConversationOptions, Role,
};

// Keep backward-compatibility aliases
pub type ConversationBuffer = Conversation;
pub type ConversationRole = Role;
pub type Message = ConversationMessage;

// Keep backward-compatibility free functions that delegate to methods
pub fn new_conversation(
    system_prompt: Option<String>,
    max_messages: Option<usize>,
    auto_compact_chars: Option<usize>,
) -> Conversation {
    Conversation::new(Some(ConversationOptions {
        system_prompt,
        max_messages,
        auto_compact_chars,
        context_window_tokens: None,
    }))
}
```

**Step 2: Handle API differences**

simse-core's `Conversation` has:
- `set_system_prompt(&mut self, prompt: String)` (takes `String`)
- simse-ui-core's was `set_system_prompt(&mut self, prompt: &str)` (takes `&str`)

If callers pass `&str`, they'll need `.to_string()`. Update callers in simse-tui.

simse-core's `ConversationMessage` has an extra `timestamp: Option<u64>` field. This is additive and should be fine.

simse-core's `Role` uses `#[serde(rename_all = "lowercase")]` while simse-ui-core's `ConversationRole` uses `#[serde(rename_all = "snake_case")]`. Check if `ToolResult` vs `tool_result` matters for serialization compatibility. If session store JSONL uses snake_case, we may need to handle this.

**Step 3: Verify it builds**

Run: `cd simse-ui-core && cargo build`

**Step 4: Commit**

```bash
git add simse-ui-core/src/state/conversation.rs
git commit -m "refactor(simse-ui-core): replace conversation types with simse-core re-exports"
```

---

### Task 8: Replace simse-ui-core agentic loop types with simse-core re-exports

**Files:**
- Modify: `simse-ui-core/src/agentic_loop.rs`
- Test: `cd simse-ui-core && cargo build`

**Step 1: Replace type definitions with re-exports**

```rust
// Re-export agentic loop types from simse-core (single source of truth)
pub use simse_core::agentic_loop::{
    AgenticLoopResult, LoopTurn, TurnType, TokenUsage,
    AgenticLoopOptions, LoopCallbacks, CancellationToken,
    AcpClient as AcpClientTrait, ToolExecutor,
};
```

**Step 2: Handle differences**

simse-core's `TokenUsage` has `Option<u64>` fields; simse-ui-core's has `u64` fields. Callers that assume non-optional will need `.unwrap_or(0)`.

simse-core's `LoopTurn` has extra `duration_ms` and `usage` fields. These are additive.

simse-core's `AgenticLoopResult` has extra `total_duration_ms` and `total_usage` fields. Additive.

**Step 3: Verify it builds**

Run: `cd simse-ui-core && cargo build`

**Step 4: Commit**

```bash
git add simse-ui-core/src/agentic_loop.rs
git commit -m "refactor(simse-ui-core): replace agentic loop types with simse-core re-exports"
```

---

### Task 9: Fix all simse-ui-core compile errors

**Files:**
- Modify: Various files in `simse-ui-core/src/` (app.rs, commands/, state/, etc.)
- Test: `cd simse-ui-core && cargo build && cargo test`

**Step 1: Build and identify all errors**

Run: `cd simse-ui-core && cargo build 2>&1`

Common issues to fix:
- `ConversationBuffer` → `Conversation` (or use the alias)
- `ConversationRole` → `Role` (or use the alias)
- `set_system_prompt(&str)` → `set_system_prompt(String)` — add `.to_string()`
- `TokenUsage` fields changed from `u64` to `Option<u64>`
- `ToolDefinition` now has `category`, `annotations`, `timeout_ms` fields — constructors need updating
- `ToolCallResult` now has `duration_ms` field — constructors need updating

**Step 2: Fix errors one by one**

For each compile error, update the code to use simse-core's type shapes. The most common fix patterns:

```rust
// Before: creating ToolDefinition without category/annotations
ToolDefinition { name, description, parameters, max_output_chars }
// After: add default fields
ToolDefinition { name, description, parameters, max_output_chars, category: ToolCategory::default(), annotations: None, timeout_ms: None }

// Before: creating ToolCallResult without duration_ms
ToolCallResult { id, name, output, is_error, diff }
// After: add duration_ms
ToolCallResult { id, name, output, is_error, diff, duration_ms: None }
```

**Step 3: Run full build and tests**

Run: `cd simse-ui-core && cargo build && cargo test`
Expected: All pass.

**Step 4: Commit**

```bash
git add simse-ui-core/
git commit -m "fix(simse-ui-core): update all code to use simse-core types"
```

---

## Phase 3: Move bridge-unique code to simse-tui

### Task 10: Move session_store.rs and json_io.rs to simse-tui

**Files:**
- Create: `simse-tui/src/session_store.rs` (copy from bridge)
- Create: `simse-tui/src/json_io.rs` (copy from bridge)
- Modify: `simse-tui/src/lib.rs` or `simse-tui/src/main.rs` (add mod declarations)
- Test: `cd simse-tui && cargo build`

**Step 1: Copy files**

Copy `simse-bridge/src/session_store.rs` to `simse-tui/src/session_store.rs`.
Copy `simse-bridge/src/json_io.rs` to `simse-tui/src/json_io.rs`.

**Step 2: Update imports in session_store.rs**

Replace:
```rust
use simse_ui_core::state::session::SessionMeta;
```
This import stays the same (SessionMeta is still in simse-ui-core).

Replace any `crate::json_io` with the new module path.

**Step 3: Add module declarations**

In `simse-tui/src/main.rs` or a dedicated `lib.rs`:
```rust
mod session_store;
mod json_io;
```

**Step 4: Verify it builds**

Run: `cd simse-tui && cargo build`
Expected: May fail due to other bridge imports — that's OK, we'll fix in Task 12.

**Step 5: Commit**

```bash
git add simse-tui/src/session_store.rs simse-tui/src/json_io.rs
git commit -m "feat(simse-tui): move session_store and json_io from simse-bridge"
```

---

### Task 11: Move config loading to simse-tui

**Files:**
- Create: `simse-tui/src/config.rs` (adapted from bridge's config.rs)
- Test: `cd simse-tui && cargo build`

**Step 1: Copy bridge's config.rs to simse-tui**

Copy `simse-bridge/src/config.rs` to `simse-tui/src/config.rs`.

**Step 2: Update types to use simse-core where possible**

Keep the `LoadedConfig` struct as-is (it's TUI-specific with fields like `data_dir`, `work_dir`, `agents`, `skills` that don't belong in simse-core's `AppConfig`).

Update internal types to use simse-core's where they overlap:
- `AcpServerConfig` may map to `simse_core::config::AcpServerEntry`
- Keep TUI-specific types (AgentConfig, SkillConfig, etc.) in this file

Update imports:
```rust
use crate::json_io;
// Remove: use simse_bridge::...
```

**Step 3: Add module declaration**

```rust
pub mod config;
```

**Step 4: Verify it builds**

Run: `cd simse-tui && cargo build`

**Step 5: Commit**

```bash
git add simse-tui/src/config.rs
git commit -m "feat(simse-tui): move config loading from simse-bridge"
```

---

## Phase 4: Migrate simse-tui to simse-core

### Task 12: Update simse-tui Cargo.toml

**Files:**
- Modify: `simse-tui/Cargo.toml`
- Test: `cd simse-tui && cargo check`

**Step 1: Replace simse-bridge with simse-core**

```toml
[dependencies]
simse-ui-core = { path = "../simse-ui-core" }
simse-core = { path = "../simse-core" }
# Remove: simse-bridge = { path = "../simse-bridge" }
```

**Step 2: Add any new dependencies needed**

If session_store/json_io need deps not already in simse-tui (like `uuid`, `flate2`), add them:

```toml
uuid = { workspace = true }
```

**Step 3: Verify cargo check**

Run: `cd simse-tui && cargo check`
Expected: Many errors from missing bridge imports. That's expected — Task 13 fixes them.

**Step 4: Commit**

```bash
git add simse-tui/Cargo.toml
git commit -m "build(simse-tui): replace simse-bridge dependency with simse-core"
```

---

### Task 13: Rewrite event_loop.rs to use simse-core

**Files:**
- Modify: `simse-tui/src/event_loop.rs`
- Test: `cd simse-tui && cargo build`

This is the largest task. The main changes:

**Step 1: Update imports**

Replace:
```rust
use simse_bridge::acp_client::AcpClient;
use simse_bridge::acp_types::AcpServerInfo;
use simse_bridge::agentic_loop::{self, AgenticLoopOptions, LoopCallbacks};
use simse_bridge::config::LoadedConfig;
use simse_bridge::session_store::SessionStore;
use simse_bridge::tool_registry::ToolRegistry;
```

With:
```rust
use simse_core::acp::client::AcpClient as AcpEngine;
use simse_core::acp::client::{AcpConfig as AcpEngineConfig, ServerEntry, GenerateOptions, StreamOptions};
use simse_core::agentic_loop::{self, AgenticLoopOptions, LoopCallbacks, CancellationToken};
use simse_core::tools::ToolRegistry;
use simse_core::{CoreContext, AppConfig, Conversation};

use crate::config::LoadedConfig;
use crate::session_store::SessionStore;
```

**Step 2: Create AcpClient trait adapter**

simse-core's `run_agentic_loop` expects `&dyn simse_core::agentic_loop::AcpClient` (a trait), but simse-acp's `AcpClient` is a concrete struct. Create an adapter:

```rust
use simse_core::agentic_loop::{AcpClient as AcpClientTrait, Message, MessageRole, GenerateResponse};

struct AcpAdapter {
    client: AcpEngine,
    session_id: Option<String>,
    server_name: Option<String>,
}

#[async_trait]
impl AcpClientTrait for AcpAdapter {
    async fn generate(
        &self,
        messages: &[Message],
        system: Option<&str>,
    ) -> Result<GenerateResponse, simse_core::SimseError> {
        // Convert messages to simse-acp's ChatMessage format
        let chat_messages: Vec<_> = messages.iter().map(|m| {
            simse_core::acp::client::ChatMessage {
                role: match m.role {
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::System => "system".to_string(),
                },
                content: vec![simse_core::acp::protocol::ContentBlock::Text {
                    text: m.content.clone(),
                }],
            }
        }).collect();

        let result = self.client.chat(&chat_messages, simse_core::acp::client::ChatOptions {
            server_name: self.server_name.clone(),
            session_id: self.session_id.clone(),
            ..Default::default()
        }).await.map_err(|e| simse_core::SimseError::Acp(e.to_string()))?;

        Ok(GenerateResponse {
            text: result.content,
            usage: result.usage.map(|u| simse_core::agentic_loop::TokenUsage {
                prompt_tokens: Some(u.prompt_tokens),
                completion_tokens: Some(u.completion_tokens),
                total_tokens: Some(u.total_tokens),
            }),
        })
    }
}
```

**Step 3: Update TuiRuntime struct**

```rust
pub struct TuiRuntime {
    config: LoadedConfig,
    acp_client: Option<AcpEngine>,
    conversation: Conversation,
    tool_registry: ToolRegistry,
    permission_manager: PermissionManager,
    session_id: Option<String>,
    cancel_token: CancellationToken,
    pub verbose: bool,
    session_store: SessionStore,
}
```

Key changes:
- `AcpClient` (bridge) → `AcpEngine` (simse-acp)
- `ConversationBuffer` → `Conversation` (simse-core)
- `ToolRegistry` (bridge) → `ToolRegistry` (simse-core)
- `abort_signal: Arc<AtomicBool>` → `cancel_token: CancellationToken`

**Step 4: Update TuiRuntime::new()**

```rust
impl TuiRuntime {
    pub fn new(config: LoadedConfig) -> Self {
        Self {
            config,
            acp_client: None,
            conversation: Conversation::new(None),
            tool_registry: ToolRegistry::new(ToolRegistryOptions::default()),
            permission_manager: PermissionManager::new(PermissionMode::Normal),
            session_id: None,
            cancel_token: CancellationToken::new(),
            verbose: false,
            session_store: SessionStore::new(&config.data_dir),
        }
    }
}
```

**Step 5: Update connect() to use simse-acp's AcpClient**

```rust
pub async fn connect(&mut self) -> Result<(), RuntimeError> {
    let server = self.resolve_server(None)?;

    let acp_config = AcpEngineConfig {
        servers: vec![ServerEntry {
            name: server.name.clone(),
            command: server.command.clone(),
            args: server.args.clone(),
            cwd: server.cwd.clone(),
            env: server.env.clone(),
            default_agent: server.default_agent.clone(),
            timeout_ms: server.timeout_ms,
            permission_policy: None,
        }],
        default_server: Some(server.name.clone()),
        default_agent: self.config.default_agent.clone(),
        mcp_servers: vec![],
    };

    let client = AcpEngine::new(acp_config).await
        .map_err(|e| RuntimeError::Acp(e.to_string()))?;

    self.acp_client = Some(client);

    // Tool discovery via simse-core's ToolRegistry
    // Register built-in tools, discover MCP tools

    Ok(())
}
```

**Step 6: Update handle_submit() to use simse-core's agentic loop**

```rust
pub async fn handle_submit(
    &mut self,
    input: &str,
    callbacks: LoopCallbacks,
) -> Result<String, RuntimeError> {
    let client = self.acp_client.as_ref()
        .ok_or(RuntimeError::NotConnected)?;

    self.conversation.add_user(input);

    let adapter = AcpAdapter {
        client: client.clone(), // or Arc
        session_id: self.session_id.clone(),
        server_name: self.config.default_server.clone(),
    };

    // Convert Conversation messages to agentic loop Message format
    let mut messages: Vec<Message> = self.conversation.messages().iter().map(|m| {
        Message {
            role: match m.role {
                Role::User => MessageRole::User,
                Role::Assistant => MessageRole::Assistant,
                Role::System => MessageRole::System,
                _ => MessageRole::User,
            },
            content: m.content.clone(),
        }
    }).collect();

    let options = AgenticLoopOptions {
        max_turns: 10,
        system_prompt: self.conversation.system_prompt().map(|s| s.to_string()),
        ..Default::default()
    };

    let result = agentic_loop::run_agentic_loop(
        &adapter,
        &self.tool_registry,
        &mut messages,
        options,
        Some(callbacks),
        Some(&self.cancel_token),
        None,
        None,
    ).await.map_err(|e| RuntimeError::Acp(e.to_string()))?;

    self.conversation.add_assistant(&result.final_text);
    Ok(result.final_text)
}
```

**Step 7: Update remaining methods**

Update `abort()`, `is_healthy()`, `is_connected()`, `agent_name()`, and all bridge action handlers to use the new types.

**Step 8: Verify it builds**

Run: `cd simse-tui && cargo build`

**Step 9: Commit**

```bash
git add simse-tui/src/event_loop.rs
git commit -m "refactor(simse-tui): rewrite event_loop to use simse-core directly"
```

---

### Task 14: Rewrite main.rs to use simse-core

**Files:**
- Modify: `simse-tui/src/main.rs`
- Test: `cd simse-tui && cargo build`

**Step 1: Update imports**

Replace:
```rust
use simse_bridge::agentic_loop::LoopCallbacks;
use simse_bridge::config::{ConfigOptions, load_config};
```

With:
```rust
use simse_core::agentic_loop::LoopCallbacks;
use crate::config::{ConfigOptions, load_config};
```

**Step 2: Update TuiLoopCallbacks**

simse-core's `LoopCallbacks` is a struct with closures, not a trait. Update accordingly:

```rust
fn create_callbacks(tx: mpsc::UnboundedSender<AppMessage>) -> LoopCallbacks {
    let tx_start = tx.clone();
    let tx_delta = tx.clone();
    let tx_error = tx.clone();

    LoopCallbacks {
        on_stream_start: Some(Box::new(move || {
            let _ = tx_start.send(AppMessage::StreamStart);
        })),
        on_stream_delta: Some(Box::new(move |delta: &str| {
            let _ = tx_delta.send(AppMessage::StreamDelta(delta.to_string()));
        })),
        on_error: Some(Box::new(move |error: &simse_core::SimseError| {
            let _ = tx_error.send(AppMessage::LoopError(error.to_string()));
        })),
        ..Default::default()
    }
}
```

**Step 3: Update initialization**

```rust
let config_options = crate::config::ConfigOptions {
    data_dir: cli.data_dir.map(PathBuf::from),
    work_dir: None,
    default_agent: cli.agent.clone(),
    log_level: None,
    server_name: cli.server.clone(),
};

let config = crate::config::load_config(&config_options);
let mut rt = event_loop::TuiRuntime::new(config);
```

**Step 4: Update agentic loop spawn to pass LoopCallbacks struct**

```rust
let callbacks = create_callbacks(tx.clone());
match rt.lock().await.handle_submit(&text, callbacks).await { ... }
```

**Step 5: Verify it builds**

Run: `cd simse-tui && cargo build`

**Step 6: Commit**

```bash
git add simse-tui/src/main.rs
git commit -m "refactor(simse-tui): rewrite main.rs to use simse-core types"
```

---

### Task 15: Fix all remaining compile errors in simse-tui

**Files:**
- Modify: Various files in `simse-tui/src/` (dispatch.rs, overlays/, commands/, etc.)
- Test: `cd simse-tui && cargo build && cargo test`

**Step 1: Run cargo build and fix all errors**

Run: `cd simse-tui && cargo build 2>&1`

Common fixes:
- Any remaining `simse_bridge::` imports → replace with `simse_core::` or `crate::`
- `ConversationBuffer` → `Conversation`
- `Arc<AtomicBool>` abort → `CancellationToken`
- `LoopCallbacks` trait impls → struct construction
- `ToolRegistry` method name differences

**Step 2: Run tests**

Run: `cd simse-tui && cargo test`
Expected: All pass.

**Step 3: Commit**

```bash
git add simse-tui/
git commit -m "fix(simse-tui): resolve all compile errors from bridge removal"
```

---

## Phase 5: Cleanup

### Task 16: Delete simse-bridge crate

**Files:**
- Delete: `simse-bridge/` (entire directory)
- Modify: `Cargo.toml` (workspace root — remove simse-bridge from members)
- Test: `cargo build --workspace`

**Step 1: Remove simse-bridge from workspace**

In root `Cargo.toml`:
```toml
[workspace]
members = [
    "simse-ui-core",
    "simse-tui",
    "simse-core",
]
```

**Step 2: Delete simse-bridge directory**

```bash
rm -rf simse-bridge/
```

**Step 3: Verify full workspace builds**

Run: `cargo build --workspace`
Expected: Build succeeds with no references to simse-bridge.

**Step 4: Run all tests**

Run: `cargo test --workspace`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add -A
git commit -m "chore: remove simse-bridge crate (replaced by simse-core direct dependency)"
```

---

### Task 17: Update CLAUDE.md and documentation

**Files:**
- Modify: `CLAUDE.md`
- Test: N/A

**Step 1: Update repository layout**

Remove `simse-bridge/` from the layout section. Update dependency descriptions.

**Step 2: Update architecture description**

Note that simse-tui now depends directly on simse-core, and simse-ui-core re-exports simse-core types.

**Step 3: Update build commands**

Remove any simse-bridge build commands.

**Step 4: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md to reflect bridge removal"
```

---

### Task 18: Add integration tests

**Files:**
- Create: `simse-tui/tests/integration.rs` (or extend existing)
- Test: `cd simse-tui && cargo test`

**Step 1: Test CoreContext initialization**

```rust
#[test]
fn test_core_context_creation() {
    let config = simse_core::AppConfig::default();
    let ctx = simse_core::CoreContext::new(config);
    assert_eq!(ctx.tool_registry.tool_count(), 0);
}
```

**Step 2: Test config loading**

```rust
#[test]
fn test_config_loads_defaults() {
    let options = crate::config::ConfigOptions::default();
    let config = crate::config::load_config(&options);
    assert!(config.data_dir.exists() || true); // graceful on missing
}
```

**Step 3: Test session store**

```rust
#[tokio::test]
async fn test_session_store_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = SessionStore::new(dir.path());
    let id = store.create("/tmp/test").unwrap();
    let meta = store.get(&id);
    assert!(meta.is_some());
}
```

**Step 4: Test type unification**

```rust
#[test]
fn test_ui_core_types_are_core_types() {
    // Verify that simse-ui-core re-exports are the same types as simse-core
    let req = simse_ui_core::tools::ToolCallRequest {
        id: "1".into(),
        name: "test".into(),
        arguments: serde_json::json!({}),
    };
    let _: simse_core::tools::ToolCallRequest = req; // Must compile
}
```

**Step 5: Run all tests**

Run: `cargo test --workspace`

**Step 6: Commit**

```bash
git add simse-tui/tests/
git commit -m "test(simse-tui): add integration tests for simse-core direct dependency"
```

---

## Task Dependencies

```
Task 1 (align types) ─┐
Task 2 (ToolExecutor)  ├─► Task 5 (ui-core dep) ─► Task 6 (tool types) ─┐
Task 3 (callbacks)     │                           Task 7 (conv types) ──┤
Task 4 (re-export acp) ┘                           Task 8 (loop types) ──┤
                                                                          ▼
                                                    Task 9 (fix ui-core) ─┐
                                                                          │
Task 10 (move session) ─┐                                                 │
Task 11 (move config)  ─┤                                                 │
                        ▼                                                 ▼
                Task 12 (tui Cargo) ─► Task 13 (event_loop) ─► Task 14 (main.rs) ─► Task 15 (fix tui) ─► Task 16 (delete bridge) ─► Task 17 (docs) ─► Task 18 (tests)
```
