# TUI Production Wiring — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Replace all 26 placeholder command handlers and 6 stub tool handlers with real implementations that call the bridge, session store, config, and JSON-RPC engines.

**Architecture:** Commands that need only sync data (session store, config, tool defs) receive a `CommandContext` with references to runtime state. Commands that need async I/O (ACP reconnect, chain execution, JSON-RPC to engines) return a `BridgeAction` variant that the event loop dispatches asynchronously via `TuiRuntime`. Tool handlers are replaced with real `ToolHandler` impls that spawn/reuse JSON-RPC subprocesses.

**Tech Stack:** Rust, tokio, serde_json, simse-bridge (JSON-RPC client, SessionStore, AcpClient, config), simse-ui-core (tool types, commands), simse-tui (app, dispatch, event_loop)

---

### Task 0: Add BridgeAction enum and CommandContext to commands/mod.rs

**Files:**
- Modify: `simse-tui/src/commands/mod.rs`

**Step 1: Write failing test**

Add test in `simse-tui/src/commands/mod.rs`:

```rust
#[test]
fn bridge_action_debug() {
    let a = BridgeAction::ListSessions;
    let _ = format!("{:?}", a);
}

#[test]
fn command_context_default_has_empty_state() {
    let ctx = CommandContext::default();
    assert!(ctx.sessions.is_empty());
    assert!(ctx.tool_defs.is_empty());
    assert!(ctx.agents.is_empty());
    assert!(ctx.skills.is_empty());
    assert!(ctx.prompts.is_empty());
    assert!(ctx.server_name.is_none());
    assert!(ctx.model_name.is_none());
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-tui && cargo +1.89.0 test --lib commands::tests::bridge_action_debug -- --exact`
Expected: FAIL — `BridgeAction` not defined

**Step 3: Write implementation**

Add to `simse-tui/src/commands/mod.rs`:

```rust
use simse_ui_core::tools::ToolDefinition;

/// Actions that require async bridge operations.
/// Returned by command handlers that cannot resolve synchronously.
/// The event loop dispatches these via TuiRuntime.
#[derive(Debug, Clone, PartialEq)]
pub enum BridgeAction {
    // Library (async — requires JSON-RPC to simse-vector)
    LibraryAdd { topic: String, text: String },
    LibrarySearch { query: String },
    LibraryRecommend { query: String },
    LibraryTopics,
    LibraryVolumes { topic: Option<String> },
    LibraryGet { id: String },
    LibraryDelete { id: String },
    // Session (async — ACP reconnect, session swap)
    ResumeSession { id: String },
    SwitchServer { name: String },
    SwitchModel { name: String },
    McpRestart,
    AcpRestart,
    // Files (async — requires JSON-RPC to simse-vfs)
    ListFiles { path: Option<String> },
    SaveFiles { path: Option<String> },
    ValidateFiles { path: Option<String> },
    DiscardFile { path: String },
    DiffFiles { path: Option<String> },
    // Config (async — file I/O)
    InitConfig { force: bool },
    FactoryReset,
    FactoryResetProject,
    // AI (async — chain execution)
    RunChain { name: String, args: String },
}

/// Read-only context available to command handlers for sync data access.
/// Built from TuiRuntime state before each command dispatch.
#[derive(Debug, Clone, Default)]
pub struct CommandContext {
    /// Session list (from SessionStore::list()).
    pub sessions: Vec<SessionInfo>,
    /// Tool definitions (from ToolRegistry::get_tool_definitions()).
    pub tool_defs: Vec<ToolDefinition>,
    /// Configured agent names.
    pub agents: Vec<AgentInfo>,
    /// Configured skill names.
    pub skills: Vec<SkillInfo>,
    /// Configured prompt chain names.
    pub prompts: Vec<PromptInfo>,
    /// Current ACP server name.
    pub server_name: Option<String>,
    /// Current model name.
    pub model_name: Option<String>,
    /// Current session ID.
    pub session_id: Option<String>,
    /// Current ACP connection status.
    pub acp_connected: bool,
    /// Data directory path.
    pub data_dir: Option<String>,
    /// Work directory path.
    pub work_dir: Option<String>,
    /// Loaded config key-value pairs for /config display.
    pub config_values: Vec<(String, String)>,
}

/// Summary info for a session (for /sessions listing).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub work_dir: String,
}

/// Summary info for an agent.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AgentInfo {
    pub name: String,
    pub description: Option<String>,
}

/// Summary info for a skill.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SkillInfo {
    pub name: String,
    pub description: Option<String>,
}

/// Summary info for a prompt chain.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PromptInfo {
    pub name: String,
    pub description: Option<String>,
    pub step_count: usize,
}
```

Also add `BridgeRequest(BridgeAction)` variant to `CommandOutput` enum.

**Step 4: Run tests**

Run: `cd simse-tui && cargo +1.89.0 test --lib commands::tests -- --exact`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-tui/src/commands/mod.rs
git commit -m "feat(simse-tui): add BridgeAction enum and CommandContext for real command wiring"
```

---

### Task 1: Implement session commands with real data

**Files:**
- Modify: `simse-tui/src/commands/session.rs`

**Step 1: Update handler signatures to accept CommandContext**

Replace all "Would call bridge" placeholders with real implementations:

- `handle_sessions(args, ctx)` → format `ctx.sessions` as Table
- `handle_resume(args)` → return `BridgeAction::ResumeSession { id }`
- `handle_rename(args, ctx)` → return `BridgeAction` or sync rename
- `handle_server(args, ctx)` → show `ctx.server_name` or return `BridgeAction::SwitchServer`
- `handle_model(args, ctx)` → show `ctx.model_name` or return `BridgeAction::SwitchModel`
- `handle_mcp(args, ctx)` → show MCP status from ctx or return `BridgeAction::McpRestart`
- `handle_acp(args, ctx)` → show ACP status from ctx or return `BridgeAction::AcpRestart`

**Step 2: Update tests**

Replace `"Would call bridge"` assertions with real data assertions. Tests should verify:
- `/sessions` with empty ctx returns "No sessions" info
- `/sessions` with sessions returns Table
- `/server` with no args shows current server name
- `/model` with no args shows current model
- `/mcp status` returns formatted status
- `/resume id` returns BridgeRequest

**Step 3: Run tests**

Run: `cd simse-tui && cargo +1.89.0 test --lib commands::session`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-tui/src/commands/session.rs
git commit -m "feat(simse-tui): implement session commands with real data from CommandContext"
```

---

### Task 2: Implement config commands with real data

**Files:**
- Modify: `simse-tui/src/commands/config.rs`

- `handle_init(args)` → return `BridgeAction::InitConfig { force }`
- `handle_config(args, ctx)` → show `ctx.config_values` as Table or filtered key
- `handle_factory_reset(args)` → return `BridgeAction::FactoryReset`
- `handle_factory_reset_project(args)` → return `BridgeAction::FactoryResetProject`
- `handle_settings` and `handle_setup` already work (open overlays)

**Step 1-4:** Same TDD pattern as Task 1.

**Step 5: Commit**

```bash
git add simse-tui/src/commands/config.rs
git commit -m "feat(simse-tui): implement config commands with real data"
```

---

### Task 3: Implement tools/AI commands with real data

**Files:**
- Modify: `simse-tui/src/commands/tools.rs`
- Modify: `simse-tui/src/commands/ai.rs`

- `handle_tools(args, ctx)` → format `ctx.tool_defs` as Table, filter by name
- `handle_agents(args, ctx)` → format `ctx.agents` as Table
- `handle_skills(args, ctx)` → format `ctx.skills` as Table
- `handle_chain(args)` → return `BridgeAction::RunChain { name, args }`
- `handle_prompts(args, ctx)` → format `ctx.prompts` as Table

**Step 1-4:** Same TDD pattern.

**Step 5: Commit**

```bash
git add simse-tui/src/commands/tools.rs simse-tui/src/commands/ai.rs
git commit -m "feat(simse-tui): implement tools and AI commands with real data"
```

---

### Task 4: Implement library commands with BridgeAction

**Files:**
- Modify: `simse-tui/src/commands/library.rs`

All library commands return `BridgeAction` variants (library operations require async JSON-RPC to simse-vector):

- `handle_add(args)` → `BridgeAction::LibraryAdd { topic, text }`
- `handle_search(args)` → `BridgeAction::LibrarySearch { query }`
- `handle_recommend(args)` → `BridgeAction::LibraryRecommend { query }`
- `handle_topics(args)` → `BridgeAction::LibraryTopics`
- `handle_volumes(args)` → `BridgeAction::LibraryVolumes { topic }`
- `handle_get(args)` → `BridgeAction::LibraryGet { id }`
- `handle_delete(args)` → `BridgeAction::LibraryDelete { id }`
- `handle_librarians` stays as-is (opens overlay)

**Step 1-4:** Same TDD pattern.

**Step 5: Commit**

```bash
git add simse-tui/src/commands/library.rs
git commit -m "feat(simse-tui): implement library commands with BridgeAction dispatch"
```

---

### Task 5: Implement file commands with BridgeAction

**Files:**
- Modify: `simse-tui/src/commands/files.rs`

All file commands return `BridgeAction` variants (VFS operations require async JSON-RPC to simse-vfs):

- `handle_files(args)` → `BridgeAction::ListFiles { path }`
- `handle_save(args)` → `BridgeAction::SaveFiles { path }`
- `handle_validate(args)` → `BridgeAction::ValidateFiles { path }`
- `handle_discard(args)` → `BridgeAction::DiscardFile { path }`
- `handle_diff(args)` → `BridgeAction::DiffFiles { path }`

**Step 1-4:** Same TDD pattern.

**Step 5: Commit**

```bash
git add simse-tui/src/commands/files.rs
git commit -m "feat(simse-tui): implement file commands with BridgeAction dispatch"
```

---

### Task 6: Update dispatch.rs to pass CommandContext

**Files:**
- Modify: `simse-tui/src/dispatch.rs`

**Step 1:** Add `CommandContext` field to `DispatchContext`:

```rust
pub struct DispatchContext {
    pub verbose: bool,
    pub plan: bool,
    pub total_tokens: u64,
    pub context_percent: u8,
    pub commands: Vec<CommandDefinition>,
    pub cmd_ctx: CommandContext,  // NEW
}
```

**Step 2:** Update `dispatch_inner` to pass `&ctx.cmd_ctx` to handlers that need it:

```rust
"sessions" => commands::session::handle_sessions(args, &ctx.cmd_ctx),
"server" => commands::session::handle_server(args, &ctx.cmd_ctx),
"model" => commands::session::handle_model(args, &ctx.cmd_ctx),
"tools" => commands::tools::handle_tools(args, &ctx.cmd_ctx),
// ... etc
```

**Step 3:** Update tests.

**Step 4: Commit**

```bash
git add simse-tui/src/dispatch.rs
git commit -m "feat(simse-tui): wire CommandContext through dispatch layer"
```

---

### Task 7: Wire app.rs to build CommandContext from TuiRuntime and handle BridgeAction

**Files:**
- Modify: `simse-tui/src/app.rs`

**Step 1:** In `dispatch_command()`, build `CommandContext` from `App` state:

The `App` struct needs access to runtime state. Add fields for the data that `CommandContext` needs, populated by the event loop after connecting:

```rust
pub struct App {
    // ... existing fields ...
    pub sessions: Vec<SessionInfo>,
    pub tool_defs: Vec<ToolDefinition>,
    pub agents: Vec<AgentInfo>,
    pub skills: Vec<SkillInfo>,
    pub prompts: Vec<PromptInfo>,
}
```

**Step 2:** Build `CommandContext` in `dispatch_command()` from these fields.

**Step 3:** Handle `CommandOutput::BridgeRequest(action)` in the output conversion:

```rust
CommandOutput::BridgeRequest(action) => {
    app.output.push(OutputItem::Info {
        text: format!("Processing: {:?}...", action),
    });
    // Store pending action for the event loop to pick up
    app.pending_bridge_action = Some(action);
}
```

**Step 4:** Add `AppMessage::BridgeResult { action, result }` for async results:

```rust
AppMessage::BridgeResult { text, is_error } => {
    if is_error {
        app.output.push(OutputItem::Error { message: text });
    } else {
        app.output.push(OutputItem::CommandResult { text });
    }
}
```

**Step 5: Commit**

```bash
git add simse-tui/src/app.rs
git commit -m "feat(simse-tui): wire CommandContext and BridgeAction into app update loop"
```

---

### Task 8: Add TuiRuntime methods for bridge actions

**Files:**
- Modify: `simse-tui/src/event_loop.rs`

**Step 1:** Add `SessionStore` to `TuiRuntime`:

```rust
pub struct TuiRuntime {
    // ... existing fields ...
    session_store: SessionStore,
}
```

**Step 2:** Add method to build `CommandContext`:

```rust
pub fn build_command_context(&self) -> CommandContext {
    CommandContext {
        sessions: self.session_store.list().into_iter().map(|m| SessionInfo {
            id: m.id, title: m.title, created_at: m.created_at,
            updated_at: m.updated_at, message_count: m.message_count,
            work_dir: m.work_dir,
        }).collect(),
        tool_defs: self.tool_registry.get_tool_definitions(),
        agents: self.config.agents.iter().map(|a| AgentInfo {
            name: a.name.clone(), description: a.description.clone(),
        }).collect(),
        skills: self.config.skills.iter().map(|s| SkillInfo {
            name: s.name.clone(), description: s.description.clone(),
        }).collect(),
        prompts: self.config.prompts.iter().map(|(name, p)| PromptInfo {
            name: name.clone(), description: p.description.clone(),
            step_count: p.steps.len(),
        }).collect(),
        server_name: self.config.default_server.clone(),
        model_name: self.config.default_agent.clone(),
        session_id: self.session_id.clone(),
        acp_connected: self.is_connected(),
        data_dir: Some(self.config.data_dir.display().to_string()),
        work_dir: Some(self.config.work_dir.display().to_string()),
        config_values: self.build_config_display(),
    }
}
```

**Step 3:** Add `execute_bridge_action` method:

```rust
pub async fn execute_bridge_action(&mut self, action: BridgeAction) -> Result<String, RuntimeError> {
    match action {
        BridgeAction::ResumeSession { id } => { ... },
        BridgeAction::SwitchServer { name } => { self.connect_to(&name).await?; Ok(format!("Switched to server: {name}")) },
        BridgeAction::SwitchModel { name } => { ... },
        BridgeAction::McpRestart => { ... },
        BridgeAction::AcpRestart => { self.reconnect().await?; Ok("ACP connection restarted.".into()) },
        BridgeAction::InitConfig { force } => { ... },
        BridgeAction::FactoryReset => { ... },
        BridgeAction::FactoryResetProject => { ... },
        BridgeAction::RunChain { name, args } => { ... },
        // Library actions
        BridgeAction::LibrarySearch { query } => { ... },
        // ... etc
    }
}
```

**Step 4: Run tests, Commit**

```bash
git add simse-tui/src/event_loop.rs
git commit -m "feat(simse-tui): add TuiRuntime methods for bridge action dispatch"
```

---

### Task 9: Replace StubHandlers with real tool handlers

**Files:**
- Modify: `simse-bridge/src/tool_registry.rs`

**Step 1:** Create real handler structs that use `BridgeProcess` to call JSON-RPC:

```rust
/// Handler that calls the simse-vector engine via JSON-RPC.
struct LibrarySearchHandler {
    /// Shared bridge process for the vector engine.
    bridge: Arc<Mutex<Option<BridgeProcess>>>,
    config: LibraryToolConfig,
}

impl ToolHandler for LibrarySearchHandler {
    async fn execute(&self, args: serde_json::Value) -> Result<ToolHandlerOutput, ToolExecutionError> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolExecutionError::HandlerError("Missing 'query' parameter".into()))?;
        let max_results = args.get("maxResults")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        // Call the vector engine via JSON-RPC
        let params = serde_json::json!({
            "query": query,
            "maxResults": max_results,
        });

        let mut bridge_guard = self.bridge.lock().await;
        let bridge = bridge_guard.as_mut()
            .ok_or_else(|| ToolExecutionError::HandlerError("Vector engine not connected".into()))?;

        let resp = crate::client::request(bridge, "store/search", Some(params))
            .await
            .map_err(|e| ToolExecutionError::HandlerError(e.to_string()))?;

        match resp.result {
            Some(result) => Ok(ToolHandlerOutput {
                output: serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|_| result.to_string()),
                diff: None,
            }),
            None => {
                let msg = resp.error
                    .map(|e| e.message)
                    .unwrap_or_else(|| "Unknown error".into());
                Err(ToolExecutionError::HandlerError(msg))
            }
        }
    }
}
```

**Step 2:** Same pattern for `LibraryShelveHandler`, `VfsReadHandler`, `VfsWriteHandler`, `VfsListHandler`, `VfsTreeHandler`.

**Step 3:** Update `register_builtins()` to accept engine config and register real handlers instead of `StubHandler`.

**Step 4:** Update `discover()` to accept engine paths/configs for spawning subprocess connections.

**Step 5: Run tests, Commit**

```bash
git add simse-bridge/src/tool_registry.rs
git commit -m "feat(simse-bridge): replace StubHandlers with real JSON-RPC tool handlers"
```

---

### Task 10: Wire MCP tool discovery

**Files:**
- Modify: `simse-bridge/src/tool_registry.rs`

**Step 1:** Implement `discover_mcp_tools()`:

```rust
async fn discover_mcp_tools(&mut self, mcp_servers: &[McpServerConfig]) {
    for server_config in mcp_servers {
        // Spawn MCP server subprocess
        let config = BridgeConfig {
            command: server_config.command.clone(),
            args: server_config.args.clone(),
            ..Default::default()
        };
        match spawn_bridge(&config).await {
            Ok(mut bridge) => {
                // Initialize MCP connection
                let init_resp = request(&mut bridge, "initialize", Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "simse", "version": "0.1.0" }
                }))).await;

                if let Ok(resp) = init_resp {
                    // List tools
                    if let Ok(tools_resp) = request(&mut bridge, "tools/list", None).await {
                        if let Some(result) = tools_resp.result {
                            if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
                                for tool in tools {
                                    // Register each tool as mcp:{server}/{name}
                                    let name = tool.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                                    let desc = tool.get("description").and_then(|d| d.as_str()).unwrap_or("");
                                    let full_name = format!("mcp:{}/{}", server_config.name, name);
                                    // Register with McpToolHandler
                                    self.register(
                                        ToolDefinition {
                                            name: full_name,
                                            description: desc.into(),
                                            parameters: HashMap::new(),
                                            max_output_chars: None,
                                        },
                                        McpToolHandler { bridge: Arc::new(Mutex::new(bridge)), server_name: server_config.name.clone(), tool_name: name.into() },
                                    );
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => { /* Log and skip unavailable servers */ }
        }
    }
}
```

**Step 2: Run tests, Commit**

```bash
git add simse-bridge/src/tool_registry.rs
git commit -m "feat(simse-bridge): wire MCP tool discovery from connected servers"
```

---

### Task 11: Wire VFS @-mention completion

**Files:**
- Modify: `simse-tui/src/at_mention.rs`

**Step 1:** Replace `complete_vfs()` placeholder:

The VFS completion needs to call the VFS engine, but `at_mention` is synchronous. Since the VFS engine is a subprocess, we need a cached file list that's refreshed periodically. Add a `VfsCache` that the event loop populates:

```rust
/// Cached VFS entries for @-mention completion.
/// Populated by the event loop when VFS is connected.
static VFS_CACHE: std::sync::Mutex<Vec<MentionEntry>> = std::sync::Mutex::new(Vec::new());

pub fn set_vfs_cache(entries: Vec<MentionEntry>) {
    *VFS_CACHE.lock().unwrap() = entries;
}

fn complete_vfs(prefix: &str) -> Vec<MentionEntry> {
    let cache = VFS_CACHE.lock().unwrap();
    cache.iter()
        .filter(|e| e.display.starts_with(prefix) || e.insert.starts_with(prefix))
        .cloned()
        .collect()
}
```

**Step 2: Run tests, Commit**

```bash
git add simse-tui/src/at_mention.rs
git commit -m "feat(simse-tui): wire VFS @-mention completion from cached entries"
```

---

### Task 12: Wire overlay screens (Librarians, Setup)

**Files:**
- Modify: `simse-tui/src/app.rs`

**Step 1:** Add `Screen::Librarians` and `Screen::Setup { preset: Option<String> }` variants.

**Step 2:** In `dispatch_command`, change overlay handling:

```rust
OverlayAction::Librarians => {
    app.screen = Screen::Librarians;
}
OverlayAction::Setup(preset) => {
    app.screen = Screen::Setup { preset };
}
```

**Step 3:** In `view()`, add rendering branches for these screens (use existing overlay code from `simse-tui/src/overlays/`).

**Step 4: Run tests, Commit**

```bash
git add simse-tui/src/app.rs
git commit -m "feat(simse-tui): wire Librarians and Setup overlay screens"
```

---

### Task 13: Integration tests and build verification

**Files:**
- Modify: `simse-tui/tests/integration.rs`

**Step 1:** Update integration tests that assert on placeholder strings to assert on real behavior (Tables, BridgeRequests, real data).

**Step 2:** Run full test suite:

```bash
cd simse-tui && cargo +1.89.0 test
cd simse-ui-core && cargo test
cd simse-bridge && cargo test
```

**Step 3:** Build binary:

```bash
cd simse-tui && cargo +1.89.0 build --release
```

**Step 4: Commit**

```bash
git add -A
git commit -m "test: update integration tests for production command wiring"
```

---

### Task 14: Final audit — verify zero stubs remain

**Step 1:** Grep for remaining stubs:

```bash
grep -rn "Would call bridge" simse-tui/src/
grep -rn "StubHandler" simse-bridge/src/
grep -rn "stub result" simse-bridge/src/
grep -rn "not yet connected" simse-bridge/src/
```

All should return zero results.

**Step 2:** Run all tests:

```bash
cd simse-tui && cargo +1.89.0 test
cd simse-bridge && cargo test
cd simse-ui-core && cargo test
```

**Step 3: Final commit and push**

```bash
git add -A
git commit -m "chore: production audit complete — zero stubs remaining"
git push
```
