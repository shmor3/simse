# simse-core Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a pure Rust library crate (`simse-core/`) that replaces all remaining TypeScript orchestration code (~20k lines), linking the 4 existing engine crates as library dependencies.

**Architecture:** Single library crate with direct Rust calls to simse-acp, simse-mcp, simse-vector, simse-vfs. No JSON-RPC between internal components. CoreContext struct holds all engine references.

**Tech Stack:** Rust, tokio, serde/serde_json, thiserror v2, tracing, tokio-util (CancellationToken), uuid, regex, futures, async-trait

**Design Doc:** `docs/plans/2026-03-01-simse-core-rust-design.md`

---

## Phase 1: Foundation (error, logger, events, config)

### Task 1: Scaffold simse-core crate

**Files:**
- Create: `simse-core/Cargo.toml`
- Create: `simse-core/src/lib.rs`
- Create: `simse-core/.gitignore`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "simse-core"
version = "0.1.0"
edition = "2021"

[lib]
name = "simse_core"
path = "src/lib.rs"

[dependencies]
simse-acp = { path = "../simse-acp" }
simse-mcp = { path = "../simse-mcp" }
simse-vector = { path = "../simse-vector" }
simse-vfs = { path = "../simse-vfs" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt", "env-filter"] }
uuid = { version = "1", features = ["v4"] }
futures = "0.3"
async-trait = "0.1"
tokio-util = "0.7"
regex = "1"
chrono = "0.4"

[dev-dependencies]
tokio-test = "0.4"
tempfile = "3"
```

**Step 2: Create lib.rs with module declarations**

```rust
pub mod error;

// More modules added in subsequent tasks
```

**Step 3: Create .gitignore**

```
target/
```

**Step 4: Verify it compiles**

Run: `cd simse-core && cargo check`
Expected: Compilation succeeds (with warning about missing error module)

**Step 5: Commit**

```bash
git add simse-core/
git commit -m "feat(simse-core): scaffold crate with engine dependencies"
```

---

### Task 2: Error system

Ports: `src/errors/base.ts` + all domain error files (~1,046 lines TS)

**Files:**
- Create: `simse-core/src/error.rs`
- Create: `simse-core/tests/error.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Create `simse-core/tests/error.rs`:

```rust
use simse_core::error::*;

#[test]
fn test_error_code_strings() {
    let err = SimseError::Config {
        code: ConfigErrorCode::InvalidField,
        message: "bad field".into(),
    };
    assert_eq!(err.code(), "CONFIG_INVALID_FIELD");

    let err = SimseError::Provider {
        code: ProviderErrorCode::Timeout,
        message: "timed out".into(),
        status: Some(504),
    };
    assert_eq!(err.code(), "PROVIDER_TIMEOUT");
}

#[test]
fn test_error_display() {
    let err = SimseError::Config {
        code: ConfigErrorCode::InvalidField,
        message: "missing name".into(),
    };
    assert_eq!(err.to_string(), "config error: missing name");
}

#[test]
fn test_error_from_engine_crates() {
    // AcpError → SimseError conversion
    let acp_err = simse_acp::error::AcpError::NotInitialized;
    let err: SimseError = acp_err.into();
    assert!(matches!(err, SimseError::Acp(_)));
}

#[test]
fn test_error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
    let err: SimseError = io_err.into();
    assert!(matches!(err, SimseError::Io(_)));
}

#[test]
fn test_all_error_codes() {
    // Verify every error code variant produces a unique string
    let codes = vec![
        SimseError::Config { code: ConfigErrorCode::InvalidField, message: String::new() }.code().to_string(),
        SimseError::Config { code: ConfigErrorCode::MissingRequired, message: String::new() }.code().to_string(),
        SimseError::Config { code: ConfigErrorCode::ValidationFailed, message: String::new() }.code().to_string(),
        SimseError::Provider { code: ProviderErrorCode::Timeout, message: String::new(), status: None }.code().to_string(),
        SimseError::Provider { code: ProviderErrorCode::Unavailable, message: String::new(), status: None }.code().to_string(),
        SimseError::Provider { code: ProviderErrorCode::AuthFailed, message: String::new(), status: None }.code().to_string(),
        SimseError::Provider { code: ProviderErrorCode::RateLimited, message: String::new(), status: None }.code().to_string(),
        SimseError::Provider { code: ProviderErrorCode::HttpError, message: String::new(), status: Some(500) }.code().to_string(),
        SimseError::Chain { code: ChainErrorCode::Empty, message: String::new() }.code().to_string(),
        SimseError::Chain { code: ChainErrorCode::StepFailed, message: String::new() }.code().to_string(),
        SimseError::Chain { code: ChainErrorCode::InvalidStep, message: String::new() }.code().to_string(),
        SimseError::Chain { code: ChainErrorCode::McpNotConfigured, message: String::new() }.code().to_string(),
        SimseError::Template { code: TemplateErrorCode::Empty, message: String::new() }.code().to_string(),
        SimseError::Template { code: TemplateErrorCode::MissingVariables, message: String::new() }.code().to_string(),
        SimseError::Mcp { code: McpErrorCode::ConnectionError, message: String::new() }.code().to_string(),
        SimseError::Mcp { code: McpErrorCode::ToolError, message: String::new() }.code().to_string(),
        SimseError::Library { code: LibraryErrorCode::EmptyText, message: String::new() }.code().to_string(),
        SimseError::Library { code: LibraryErrorCode::EmbeddingFailed, message: String::new() }.code().to_string(),
        SimseError::Library { code: LibraryErrorCode::NotInitialized, message: String::new() }.code().to_string(),
        SimseError::Loop { code: LoopErrorCode::DoomLoop, message: String::new() }.code().to_string(),
        SimseError::Loop { code: LoopErrorCode::TurnLimit, message: String::new() }.code().to_string(),
        SimseError::Resilience { code: ResilienceErrorCode::CircuitOpen, message: String::new() }.code().to_string(),
        SimseError::Resilience { code: ResilienceErrorCode::Timeout, message: String::new() }.code().to_string(),
        SimseError::Resilience { code: ResilienceErrorCode::RetryExhausted, message: String::new() }.code().to_string(),
        SimseError::Task { code: TaskErrorCode::NotFound, message: String::new() }.code().to_string(),
        SimseError::Task { code: TaskErrorCode::LimitReached, message: String::new() }.code().to_string(),
        SimseError::Task { code: TaskErrorCode::CircularDependency, message: String::new() }.code().to_string(),
        SimseError::Tool { code: ToolErrorCode::NotFound, message: String::new() }.code().to_string(),
        SimseError::Tool { code: ToolErrorCode::ExecutionFailed, message: String::new() }.code().to_string(),
        SimseError::Tool { code: ToolErrorCode::PermissionDenied, message: String::new() }.code().to_string(),
        SimseError::Tool { code: ToolErrorCode::Timeout, message: String::new() }.code().to_string(),
        SimseError::Vfs { code: VfsErrorCode::InvalidPath, message: String::new() }.code().to_string(),
        SimseError::Vfs { code: VfsErrorCode::NotFound, message: String::new() }.code().to_string(),
    ];
    // All codes should be unique
    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(unique.len(), codes.len(), "Duplicate error codes found");
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-core && cargo test --test error`
Expected: FAIL — `error` module doesn't exist yet

**Step 3: Implement error.rs**

Create `simse-core/src/error.rs` with the full `SimseError` enum, all `*ErrorCode` enums, and the `code()` method. Each error code enum:

- `ConfigErrorCode`: InvalidField, MissingRequired, ValidationFailed
- `ProviderErrorCode`: Timeout, Unavailable, AuthFailed, RateLimited, HttpError
- `ChainErrorCode`: Empty, StepFailed, InvalidStep, McpNotConfigured, ExecutionFailed, McpToolError, NotFound
- `TemplateErrorCode`: Empty, MissingVariables, InvalidValue
- `McpErrorCode`: ConnectionError, ToolError, ResourceError, ServerError
- `LibraryErrorCode`: EmptyText, EmbeddingFailed, NotInitialized, DuplicateDetected
- `LoopErrorCode`: DoomLoop, TurnLimit, Aborted, CompactionFailed
- `ResilienceErrorCode`: CircuitOpen, Timeout, RetryExhausted, RetryAborted
- `TaskErrorCode`: NotFound, LimitReached, CircularDependency, InvalidStatus
- `ToolErrorCode`: NotFound, ExecutionFailed, PermissionDenied, Timeout, ParseError
- `VfsErrorCode`: InvalidPath, NotFound, AlreadyExists, LimitExceeded

The `code()` method returns uppercase strings like `"CONFIG_INVALID_FIELD"`, `"PROVIDER_TIMEOUT"`, etc.

Add `#[from]` conversions for engine crate errors and `std::io::Error`.

**Step 4: Update lib.rs**

```rust
pub mod error;
```

**Step 5: Run tests to verify they pass**

Run: `cd simse-core && cargo test --test error`
Expected: All tests PASS

**Step 6: Commit**

```bash
git add simse-core/src/error.rs simse-core/src/lib.rs simse-core/tests/error.rs
git commit -m "feat(simse-core): unified error system with domain error codes"
```

---

### Task 3: Logger

Ports: `src/logger.ts` (~284 lines) + `src/ai/shared/logger.ts` (~54 lines)

**Files:**
- Create: `simse-core/src/logger.rs`
- Create: `simse-core/tests/logger.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

```rust
use simse_core::logger::*;

#[test]
fn test_logger_creation() {
    let logger = Logger::new("test");
    // Should not panic
    logger.info("hello");
}

#[test]
fn test_child_logger() {
    let parent = Logger::new("parent");
    let child = parent.child("child");
    // Child context should be "parent:child"
    child.info("from child");
}

#[test]
fn test_log_level_filtering() {
    let logger = Logger::new("test");
    logger.set_level(LogLevel::Warn);
    assert_eq!(logger.get_level(), LogLevel::Warn);
    // debug and info should be filtered (no-op)
    logger.debug("should not appear");
    logger.info("should not appear");
    // warn and error should pass through
    logger.warn("warning");
    logger.error("error");
}

#[test]
fn test_shared_level_between_parent_and_child() {
    let parent = Logger::new("parent");
    let child = parent.child("child");
    parent.set_level(LogLevel::Error);
    // Child should also be at Error level since they share state
    assert_eq!(child.get_level(), LogLevel::Error);
}

#[test]
fn test_noop_logger() {
    let logger = create_noop_logger();
    // All methods should be no-ops
    logger.debug("noop");
    logger.info("noop");
    logger.warn("noop");
    logger.error("noop");
    let child = logger.child("child");
    child.info("also noop");
}

#[test]
fn test_log_level_priority() {
    assert!(LogLevel::Debug < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Warn);
    assert!(LogLevel::Warn < LogLevel::Error);
    assert!(LogLevel::Error < LogLevel::None);
}
```

**Step 2: Run tests to verify they fail**

Run: `cd simse-core && cargo test --test logger`
Expected: FAIL

**Step 3: Implement logger.rs**

Thin wrapper around `tracing`:
- `LogLevel` enum: Debug, Info, Warn, Error, None (with Ord impl)
- `Logger` struct wrapping `Arc<Mutex<LogLevel>>` + context string
- `child()` creates new Logger with "parent:child" context and same Arc
- `set_level()` / `get_level()` mutate/read shared level
- Logging methods check level priority before emitting via `tracing::{debug,info,warn,error}!`
- `create_noop_logger()` returns Logger with level=None

**Step 4: Run tests**

Run: `cd simse-core && cargo test --test logger`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-core/src/logger.rs simse-core/tests/logger.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): logger wrapping tracing with shared parent/child level"
```

---

### Task 4: Event bus

Ports: `src/events/event-bus.ts` + `src/events/types.ts` (~236 lines)

**Files:**
- Create: `simse-core/src/events.rs`
- Create: `simse-core/tests/events.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

```rust
use simse_core::events::*;
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

#[test]
fn test_publish_subscribe() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicU32::new(0));
    let c = counter.clone();
    let _unsub = bus.subscribe("test.event", move |_payload| {
        c.fetch_add(1, Ordering::SeqCst);
    });
    bus.publish("test.event", serde_json::json!({"key": "value"}));
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn test_unsubscribe() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicU32::new(0));
    let c = counter.clone();
    let unsub = bus.subscribe("test.event", move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });
    bus.publish("test.event", serde_json::json!({}));
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    unsub();
    bus.publish("test.event", serde_json::json!({}));
    assert_eq!(counter.load(Ordering::SeqCst), 1); // no change
}

#[test]
fn test_subscribe_all() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicU32::new(0));
    let c = counter.clone();
    let _unsub = bus.subscribe_all(move |_event_type, _payload| {
        c.fetch_add(1, Ordering::SeqCst);
    });
    bus.publish("event.a", serde_json::json!({}));
    bus.publish("event.b", serde_json::json!({}));
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn test_clear() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicU32::new(0));
    let c = counter.clone();
    let _unsub = bus.subscribe("x", move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });
    bus.clear();
    bus.publish("x", serde_json::json!({}));
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[test]
fn test_handler_error_isolation() {
    let bus = EventBus::new();
    let counter = Arc::new(AtomicU32::new(0));
    let c = counter.clone();
    // First handler panics
    let _unsub1 = bus.subscribe("x", |_| {
        panic!("handler error");
    });
    // Second handler should still fire
    let _unsub2 = bus.subscribe("x", move |_| {
        c.fetch_add(1, Ordering::SeqCst);
    });
    bus.publish("x", serde_json::json!({}));
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
```

**Step 2: Run tests to fail**

Run: `cd simse-core && cargo test --test events`

**Step 3: Implement events.rs**

- `EventBus` struct with `HashMap<String, Vec<Box<dyn Fn(Value) + Send + Sync>>>` + global handlers
- Thread-safe via `Arc<Mutex<...>>`
- `publish()` catches panics via `std::panic::catch_unwind` (handler isolation)
- `subscribe()` returns closure that removes handler
- Event type constants for all known events (as string constants, not enum — allows extensibility)

**Step 4: Run tests**

Run: `cd simse-core && cargo test --test events`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-core/src/events.rs simse-core/tests/events.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): event bus with handler error isolation"
```

---

### Task 5: Config

Ports: `src/config/schema.ts` + `src/config/settings.ts` (~1,160 lines)

**Files:**
- Create: `simse-core/src/config.rs`
- Create: `simse-core/tests/config.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

```rust
use simse_core::config::*;

#[test]
fn test_default_config() {
    let config = AppConfig::default();
    assert!(config.acp.servers.is_empty());
    assert!(config.mcp.servers.is_empty());
    assert!(!config.library.enabled);
}

#[test]
fn test_define_config_minimal() {
    let raw = serde_json::json!({
        "acp": { "servers": [] },
    });
    let config = define_config(raw, None).unwrap();
    assert!(config.acp.servers.is_empty());
}

#[test]
fn test_define_config_with_acp_server() {
    let raw = serde_json::json!({
        "acp": {
            "servers": [{
                "name": "test",
                "command": "/usr/bin/agent",
                "args": ["--mode", "fast"]
            }]
        }
    });
    let config = define_config(raw, None).unwrap();
    assert_eq!(config.acp.servers.len(), 1);
    assert_eq!(config.acp.servers[0].name, "test");
}

#[test]
fn test_validation_rejects_invalid_timeout() {
    let raw = serde_json::json!({
        "acp": {
            "servers": [{
                "name": "test",
                "command": "agent",
                "timeoutMs": 100  // below minimum of 1000
            }]
        }
    });
    let result = define_config(raw, None);
    assert!(result.is_err());
}

#[test]
fn test_config_deserialization() {
    let json = r#"{"acp":{"servers":[]},"mcp":{"servers":[]}}"#;
    let config: AppConfig = serde_json::from_str(json).unwrap();
    assert!(config.acp.servers.is_empty());
}
```

**Step 2: Run tests to fail**

**Step 3: Implement config.rs**

- `AppConfig` struct with nested `AcpConfig`, `McpConfig`, `LibraryConfig`, `VfsConfig`, `ToolsConfig`, `LoopConfig`, `PromptsConfig`
- All derive `Serialize, Deserialize, Clone, Debug, Default`
- `define_config(raw: Value, opts: Option<DefineConfigOptions>) -> Result<AppConfig, SimseError>`
- Validation functions matching TS schema.ts constraints
- Lenient mode: reset invalid fields to defaults + call on_warn

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/config.rs simse-core/tests/config.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): config with validation and define_config"
```

---

## Phase 2: Core Data (conversation, tasks)

### Task 6: Conversation

Ports: `src/ai/conversation/conversation.ts` + `types.ts` (~438 lines)

**Files:**
- Create: `simse-core/src/conversation.rs`
- Create: `simse-core/tests/conversation.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

```rust
use simse_core::conversation::*;

#[test]
fn test_add_and_retrieve_messages() {
    let mut conv = Conversation::new(None);
    conv.add_user("hello");
    conv.add_assistant("hi there");
    let msgs = conv.messages();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, Role::User);
    assert_eq!(msgs[1].role, Role::Assistant);
}

#[test]
fn test_system_prompt() {
    let mut conv = Conversation::new(None);
    conv.set_system_prompt("you are helpful".into());
    let all = conv.to_messages();
    assert_eq!(all[0].role, Role::System);
    assert_eq!(all[0].content, "you are helpful");
}

#[test]
fn test_tool_result() {
    let mut conv = Conversation::new(None);
    conv.add_tool_result("call_1", "search", "found 3 results");
    let msgs = conv.messages();
    assert_eq!(msgs[0].role, Role::ToolResult);
    assert_eq!(msgs[0].tool_call_id.as_deref(), Some("call_1"));
}

#[test]
fn test_max_messages_trimming() {
    let mut conv = Conversation::new(Some(ConversationOptions {
        max_messages: Some(2),
        ..Default::default()
    }));
    conv.add_user("1");
    conv.add_assistant("2");
    conv.add_user("3");
    // Oldest non-system message should be trimmed
    assert_eq!(conv.message_count(), 2);
    assert_eq!(conv.messages()[0].content, "2");
}

#[test]
fn test_compact() {
    let mut conv = Conversation::new(None);
    conv.add_user("first");
    conv.add_assistant("response");
    conv.compact("summary of conversation");
    assert_eq!(conv.message_count(), 1);
    assert!(conv.messages()[0].content.contains("summary"));
}

#[test]
fn test_serialize_deserialize() {
    let mut conv = Conversation::new(None);
    conv.set_system_prompt("system".into());
    conv.add_user("hello");
    conv.add_assistant("world");
    let json = conv.to_json();
    let mut conv2 = Conversation::new(None);
    conv2.from_json(&json);
    assert_eq!(conv2.messages().len(), 2);
    assert_eq!(conv2.system_prompt().as_deref(), Some("system"));
}

#[test]
fn test_needs_compaction() {
    let mut conv = Conversation::new(Some(ConversationOptions {
        auto_compact_chars: Some(50),
        ..Default::default()
    }));
    conv.add_user(&"x".repeat(60));
    assert!(conv.needs_compaction());
}

#[test]
fn test_estimated_chars() {
    let mut conv = Conversation::new(None);
    conv.set_system_prompt("abc".into());
    conv.add_user("defgh");
    assert_eq!(conv.estimated_chars(), 8); // 3 + 5
}

#[test]
fn test_clear() {
    let mut conv = Conversation::new(None);
    conv.add_user("test");
    conv.clear();
    assert_eq!(conv.message_count(), 0);
}

#[test]
fn test_replace_messages() {
    let mut conv = Conversation::new(None);
    conv.add_user("old");
    let new_msgs = vec![
        ConversationMessage {
            role: Role::User,
            content: "new".into(),
            tool_call_id: None,
            tool_name: None,
            timestamp: None,
        },
    ];
    conv.replace_messages(&new_msgs);
    assert_eq!(conv.messages()[0].content, "new");
}

#[test]
fn test_load_messages() {
    let mut conv = Conversation::new(None);
    let msgs = vec![
        ConversationMessage {
            role: Role::User,
            content: "loaded".into(),
            tool_call_id: None,
            tool_name: None,
            timestamp: Some(12345),
        },
    ];
    conv.load_messages(msgs);
    assert_eq!(conv.messages()[0].content, "loaded");
}
```

**Step 2: Run tests to fail**

**Step 3: Implement conversation.rs**

Port all logic from TS `createConversation`:
- `Role` enum: System, User, Assistant, ToolResult
- `ConversationMessage` struct with optional fields
- `Conversation` struct with `messages: Vec<ConversationMessage>`, `system_prompt: Option<String>`, `max_messages`, `auto_compact_chars`
- Methods: `add_user`, `add_assistant`, `add_tool_result`, `to_messages` (prepends system), `serialize`, `clear`, `compact`, `to_json`, `from_json`, `replace_messages`, `load_messages`
- Getters: `message_count`, `estimated_chars`, `estimated_tokens`, `needs_compaction`, `context_usage_percent`
- `trim_if_needed()` internal method

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/conversation.rs simse-core/tests/conversation.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): conversation with compaction, serialization, trimming"
```

---

### Task 7: Task list

Ports: `src/ai/tasks/task-list.ts` + `types.ts` (~366 lines)

**Files:**
- Create: `simse-core/src/tasks.rs`
- Create: `simse-core/tests/tasks.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

```rust
use simse_core::tasks::*;
use simse_core::error::SimseError;

#[test]
fn test_create_task() {
    let mut list = TaskList::new(None);
    let task = list.create(TaskCreateInput {
        subject: "Fix bug".into(),
        description: "Fix the login bug".into(),
        active_form: Some("Fixing bug".into()),
        owner: None,
        metadata: None,
    });
    assert_eq!(task.id, "1");
    assert_eq!(task.status, TaskStatus::Pending);
}

#[test]
fn test_update_status() {
    let mut list = TaskList::new(None);
    list.create(TaskCreateInput {
        subject: "Task".into(),
        description: "Desc".into(),
        active_form: None, owner: None, metadata: None,
    });
    let updated = list.update("1", TaskUpdateInput {
        status: Some(TaskStatus::InProgress),
        ..Default::default()
    }).unwrap();
    assert_eq!(updated.status, TaskStatus::InProgress);
}

#[test]
fn test_dependency_tracking() {
    let mut list = TaskList::new(None);
    list.create(TaskCreateInput { subject: "A".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.create(TaskCreateInput { subject: "B".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.update("2", TaskUpdateInput {
        add_blocked_by: Some(vec!["1".into()]),
        ..Default::default()
    }).unwrap();
    let task2 = list.get("2").unwrap();
    assert_eq!(task2.blocked_by, vec!["1"]);
    let task1 = list.get("1").unwrap();
    assert_eq!(task1.blocks, vec!["2"]);
}

#[test]
fn test_circular_dependency_rejected() {
    let mut list = TaskList::new(None);
    list.create(TaskCreateInput { subject: "A".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.create(TaskCreateInput { subject: "B".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.update("2", TaskUpdateInput {
        add_blocked_by: Some(vec!["1".into()]),
        ..Default::default()
    }).unwrap();
    let result = list.update("1", TaskUpdateInput {
        add_blocked_by: Some(vec!["2".into()]),
        ..Default::default()
    });
    assert!(result.is_err()); // circular dependency
}

#[test]
fn test_completing_unblocks_dependents() {
    let mut list = TaskList::new(None);
    list.create(TaskCreateInput { subject: "A".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.create(TaskCreateInput { subject: "B".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.update("2", TaskUpdateInput {
        add_blocked_by: Some(vec!["1".into()]),
        ..Default::default()
    }).unwrap();
    list.update("1", TaskUpdateInput {
        status: Some(TaskStatus::Completed),
        ..Default::default()
    }).unwrap();
    let task2 = list.get("2").unwrap();
    assert!(task2.blocked_by.is_empty()); // unblocked
}

#[test]
fn test_delete_cleans_up_deps() {
    let mut list = TaskList::new(None);
    list.create(TaskCreateInput { subject: "A".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.create(TaskCreateInput { subject: "B".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.update("2", TaskUpdateInput {
        add_blocked_by: Some(vec!["1".into()]),
        ..Default::default()
    }).unwrap();
    list.delete("1");
    let task2 = list.get("2").unwrap();
    assert!(task2.blocked_by.is_empty());
}

#[test]
fn test_list_available() {
    let mut list = TaskList::new(None);
    list.create(TaskCreateInput { subject: "A".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.create(TaskCreateInput { subject: "B".into(), description: "".into(), active_form: None, owner: Some("alice".into()), metadata: None });
    let available = list.list_available();
    assert_eq!(available.len(), 1); // B has owner
    assert_eq!(available[0].subject, "A");
}

#[test]
fn test_task_limit() {
    let mut list = TaskList::new(Some(TaskListOptions { max_tasks: Some(2) }));
    list.create(TaskCreateInput { subject: "A".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    list.create(TaskCreateInput { subject: "B".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    let result = list.create_checked(TaskCreateInput { subject: "C".into(), description: "".into(), active_form: None, owner: None, metadata: None });
    assert!(result.is_err()); // limit reached
}

#[test]
fn test_metadata_merge() {
    let mut list = TaskList::new(None);
    let mut meta = std::collections::HashMap::new();
    meta.insert("key1".into(), serde_json::json!("val1"));
    list.create(TaskCreateInput {
        subject: "A".into(),
        description: "".into(),
        active_form: None,
        owner: None,
        metadata: Some(meta),
    });
    let mut new_meta = std::collections::HashMap::new();
    new_meta.insert("key2".into(), serde_json::json!("val2"));
    new_meta.insert("key1".into(), serde_json::Value::Null); // delete key1
    list.update("1", TaskUpdateInput {
        metadata: Some(new_meta),
        ..Default::default()
    }).unwrap();
    let task = list.get("1").unwrap();
    let meta = task.metadata.as_ref().unwrap();
    assert!(!meta.contains_key("key1")); // deleted
    assert_eq!(meta.get("key2").unwrap(), &serde_json::json!("val2"));
}
```

**Step 2: Run tests to fail**

**Step 3: Implement tasks.rs**

Port `createTaskList` logic:
- `TaskStatus` enum, `TaskItem` struct, `TaskCreateInput`, `TaskUpdateInput`, `TaskListOptions`
- `TaskList` struct with `HashMap<String, TaskItem>`, `next_id: u64`
- BFS cycle detection in `would_create_cycle()`
- Reciprocal dependency maintenance (blocks ↔ blocked_by)
- Completing task removes itself from dependents' blocked_by
- Delete cleans up all references
- Metadata merge with null filtering

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/tasks.rs simse-core/tests/tasks.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): task list with dependency tracking and cycle detection"
```

---

## Phase 3: Utils

### Task 8: Retry, circuit breaker, timeout

Ports: `src/utils/retry.ts` + `circuit-breaker.ts` + `health-monitor.ts` + `timeout.ts` (~612 lines)

**Files:**
- Create: `simse-core/src/utils/mod.rs`
- Create: `simse-core/src/utils/retry.rs`
- Create: `simse-core/src/utils/circuit_breaker.rs`
- Create: `simse-core/src/utils/health_monitor.rs`
- Create: `simse-core/src/utils/timeout.rs`
- Create: `simse-core/tests/utils.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Cover: retry with backoff, abort during retry, circuit breaker state transitions, health monitor thresholds, timeout cancellation.

Key tests:
- `retry_succeeds_on_third_attempt` — fn fails twice then succeeds
- `retry_respects_max_attempts` — fn always fails, returns last error
- `retry_aborts_on_cancellation` — CancellationToken fires mid-retry
- `circuit_breaker_opens_after_threshold` — 5 failures → Open state
- `circuit_breaker_resets_after_timeout` — Open → HalfOpen after duration
- `circuit_breaker_closes_on_success` — HalfOpen + success → Closed
- `health_monitor_status_transitions` — Healthy → Degraded → Unhealthy
- `with_timeout_completes` — fast fn completes before timeout
- `with_timeout_cancels` — slow fn hits timeout deadline

**Step 2: Run tests to fail**

**Step 3: Implement utils modules**

- **retry.rs**: `with_retry<F, T>(f, opts, cancel)` — exponential backoff with jitter formula: `base * mult^(attempt-1)`, capped at max_delay, jitter = ±factor * delay
- **circuit_breaker.rs**: `CircuitBreaker` with Closed/Open(Instant)/HalfOpen states, lazy transition check, `allow_request()` / `record_success()` / `record_failure()`
- **health_monitor.rs**: `HealthMonitor` with sliding window, consecutive failure tracking, Healthy/Degraded/Unhealthy thresholds
- **timeout.rs**: `with_timeout<F, T>(f, duration)` using `tokio::time::timeout`

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/utils/ simse-core/tests/utils.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): retry, circuit breaker, health monitor, timeout utils"
```

---

## Phase 4: Prompts

### Task 9: System prompt builder

Ports: `src/ai/prompts/` (~434 lines)

**Files:**
- Create: `simse-core/src/prompts/mod.rs`
- Create: `simse-core/src/prompts/builder.rs`
- Create: `simse-core/src/prompts/environment.rs`
- Create: `simse-core/src/prompts/provider.rs`
- Create: `simse-core/tests/prompts.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Cover: builder section ordering, environment context injection, instruction discovery, provider-specific templates, mode switching.

**Step 2: Run tests to fail**

**Step 3: Implement prompts module**

- `SystemPromptBuilder` with ordered sections (identity, mode, tool guidelines, environment, instructions, custom, tool defs, memory)
- `EnvironmentInfo` struct: platform, shell, cwd, date, git info
- `discover_instructions(dir)` — scan for .simse, CLAUDE.md files
- `provider_prompt(provider)` — static provider-specific templates
- Three modes: build, plan, explore

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/prompts/ simse-core/tests/prompts.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): system prompt builder with mode switching"
```

---

## Phase 5: Chain & Agent

### Task 10: Prompt template

Ports: `src/ai/chain/prompt-template.ts` (~85 lines)

**Files:**
- Create: `simse-core/src/chain/mod.rs`
- Create: `simse-core/src/chain/template.rs`
- Create: `simse-core/tests/chain_template.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

```rust
use simse_core::chain::template::*;

#[test]
fn test_create_template() {
    let t = PromptTemplate::new("Hello {name}!").unwrap();
    assert!(t.has_variables());
    assert_eq!(t.variables(), vec!["name"]);
}

#[test]
fn test_format() {
    let t = PromptTemplate::new("Hello {name}, welcome to {place}!").unwrap();
    let mut values = std::collections::HashMap::new();
    values.insert("name".into(), "Alice".into());
    values.insert("place".into(), "Rust".into());
    assert_eq!(t.format(&values).unwrap(), "Hello Alice, welcome to Rust!");
}

#[test]
fn test_missing_variable() {
    let t = PromptTemplate::new("Hello {name}!").unwrap();
    let values = std::collections::HashMap::new();
    assert!(t.format(&values).is_err());
}

#[test]
fn test_empty_template_rejected() {
    assert!(PromptTemplate::new("").is_err());
}

#[test]
fn test_no_variables() {
    let t = PromptTemplate::new("static text").unwrap();
    assert!(!t.has_variables());
    assert_eq!(t.format(&Default::default()).unwrap(), "static text");
}

#[test]
fn test_duplicate_variables_deduped() {
    let t = PromptTemplate::new("{a} and {a}").unwrap();
    assert_eq!(t.variables(), vec!["a"]);
}

#[test]
fn test_hyphenated_variables() {
    let t = PromptTemplate::new("{my-var}").unwrap();
    assert_eq!(t.variables(), vec!["my-var"]);
}
```

**Step 2: Run tests to fail**

**Step 3: Implement template.rs**

- `PromptTemplate` struct with `raw: String`, `variables: Vec<String>`
- `new(template)` — extract `{varname}` via regex, deduplicate
- `format(values)` — validate all vars present, replace all occurrences
- `has_variables()`, `variables()`, `raw()`

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/chain/ simse-core/tests/chain_template.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): prompt template with variable extraction and formatting"
```

---

### Task 11: Agent executor

Ports: `src/ai/agent/agent-executor.ts` + `types.ts` (~261 lines)

**Files:**
- Create: `simse-core/src/agent.rs`
- Create: `simse-core/tests/agent.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Test the dispatcher logic with mock/stub providers. Since the real ACP/MCP clients are complex, tests focus on dispatch routing and error handling.

**Step 2: Run tests to fail**

**Step 3: Implement agent.rs**

- `AgentResult` struct: output, model, usage, tool_metrics
- `AgentStepConfig` struct: name, agent_id, server_name, etc.
- `ProviderRef` enum: Acp, Mcp, Library
- `AgentExecutor::execute(step, provider, prompt, ctx)` — dispatch based on ProviderRef

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/agent.rs simse-core/tests/agent.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): agent executor with ACP/MCP/library dispatch"
```

---

### Task 12: Chain execution

Ports: `src/ai/chain/chain.ts` + `types.ts` (~1,087 lines)

**Files:**
- Create: `simse-core/src/chain/chain.rs`
- Create: `simse-core/src/chain/types.rs`
- Modify: `simse-core/src/chain/mod.rs`
- Create: `simse-core/tests/chain.rs`

**Step 1: Write failing tests**

Cover: sequential step execution, parallel steps, merge strategies, input mapping, output transforms, chain from definition, named chain.

**Step 2: Run tests to fail**

**Step 3: Implement chain.rs**

- `Chain` struct with `steps: Vec<ChainStep>`, `callbacks: Option<ChainCallbacks>`
- `ChainBuilder` for fluent construction
- `run(initial_values)` — sequential execution loop
- `run_parallel_step()` — fan-out/fan-in with merge strategies
- Parallel strategies: Concat, Keyed, Custom
- Error handling: wrap in ChainStepError, fire callbacks
- `create_chain_from_definition()` — construct from declarative definition
- `run_named_chain()` — look up and run by name

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/chain/ simse-core/tests/chain.rs
git commit -m "feat(simse-core): chain execution with parallel steps and merge strategies"
```

---

## Phase 6: Tools

### Task 13: Tool registry

Ports: `src/ai/tools/tool-registry.ts` + `types.ts` + `permissions.ts` (~680 lines)

**Files:**
- Create: `simse-core/src/tools/mod.rs`
- Create: `simse-core/src/tools/registry.rs`
- Create: `simse-core/src/tools/types.rs`
- Create: `simse-core/src/tools/permissions.rs`
- Create: `simse-core/tests/tool_registry.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Cover: register/unregister, execute with output truncation, batch execute with concurrency, tool call parsing from `<tool_use>` XML tags, system prompt formatting, metrics tracking, permission resolver.

**Step 2: Run tests to fail**

**Step 3: Implement**

- `ToolDefinition`, `ToolCallRequest`, `ToolCallResult`, `ToolHandler` type alias
- `ToolRegistry` struct: `HashMap<String, RegisteredTool>`, metrics map
- `execute()` — permission check, timeout, handler invocation, output truncation, metrics
- `batch_execute()` — bounded concurrency via tokio semaphore
- `parse_tool_calls()` — regex for `<tool_use>` XML blocks, extract JSON
- `format_for_system_prompt()` — formatted tool list string
- `ToolPermissionResolver` — glob-based rule matching

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/tools/ simse-core/tests/tool_registry.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): tool registry with parsing, permissions, truncation"
```

---

### Task 14: Builtin tools

Ports: `src/ai/tools/builtin-tools.ts` (~508 lines)

**Files:**
- Create: `simse-core/src/tools/builtin.rs`
- Create: `simse-core/tests/builtin_tools.rs`

**Step 1: Write failing tests**

Cover: library_search, library_shelve, library_withdraw, library_catalog, library_compact, vfs_read, vfs_write, vfs_list, vfs_tree, task_create, task_get, task_update, task_delete, task_list.

**Step 2: Run tests to fail**

**Step 3: Implement builtin.rs**

Three registration functions calling directly into engine crate library APIs:
- `register_library_tools(registry, store)` — 5 tools
- `register_vfs_tools(registry, vfs)` — 4 tools
- `register_task_tools(registry, task_list)` — 5 tools

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/tools/builtin.rs simse-core/tests/builtin_tools.rs
git commit -m "feat(simse-core): builtin library, VFS, and task tools"
```

---

### Task 15: Host tools

Ports: `src/ai/tools/host/` (~1,445 lines)

**Files:**
- Create: `simse-core/src/tools/host/mod.rs`
- Create: `simse-core/src/tools/host/filesystem.rs`
- Create: `simse-core/src/tools/host/git.rs`
- Create: `simse-core/src/tools/host/bash.rs`
- Create: `simse-core/src/tools/host/fuzzy_edit.rs`
- Create: `simse-core/tests/host_tools.rs`

**Step 1: Write failing tests**

Cover: fs_read with line numbers, fs_write with parent creation, fs_edit with fuzzy match strategies (exact, line-trimmed, whitespace-normalized, indentation-flexible, block-anchor), fs_glob, fs_grep, fs_list, fs_stat, fs_delete, fs_move, git_status, git_diff, git_log, bash with timeout, bash with output truncation.

**Step 2: Run tests to fail**

**Step 3: Implement**

- `filesystem.rs`: `register_filesystem_tools()` — 10 tools using `tokio::fs` + path sandboxing
- `git.rs`: `register_git_tools()` — 9 tools using `tokio::process::Command`
- `bash.rs`: `register_bash_tools()` — 1 tool with timeout, output truncation
- `fuzzy_edit.rs`: 5-strategy fuzzy matching engine (exact → line-trimmed → whitespace-normalized → indentation-flexible → block-anchor+levenshtein)

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/tools/host/ simse-core/tests/host_tools.rs
git commit -m "feat(simse-core): host tools (filesystem, git, bash, fuzzy edit)"
```

---

### Task 16: Subagent and delegation tools

Ports: `src/ai/tools/subagent-tools.ts` + `delegation-tools.ts` (~525 lines)

**Files:**
- Create: `simse-core/src/tools/subagent.rs`
- Create: `simse-core/src/tools/delegation.rs`
- Create: `simse-core/tests/subagent_tools.rs`

**Step 1: Write failing tests**

Cover: subagent_spawn with shelf scoping, depth limiting, child registry construction, delegation tool per non-primary server.

**Step 2: Run tests to fail**

**Step 3: Implement**

- `subagent.rs`: `register_subagent_tools()` — spawn isolated sub-loops with shelf-scoped library, depth check, child registry
- `delegation.rs`: `register_delegation_tools()` — per-server delegation tools

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/tools/subagent.rs simse-core/src/tools/delegation.rs simse-core/tests/subagent_tools.rs
git commit -m "feat(simse-core): subagent spawning with shelf isolation and delegation"
```

---

## Phase 7: Library Orchestration

### Task 17: Library high-level API

Ports: `src/ai/library/library.ts` + `shelf.ts` + `query-dsl.ts` + `prompt-injection.ts` (~1,111 lines)

**Files:**
- Create: `simse-core/src/library/mod.rs`
- Create: `simse-core/src/library/library.rs`
- Create: `simse-core/src/library/shelf.rs`
- Create: `simse-core/src/library/query_dsl.rs`
- Create: `simse-core/src/library/prompt_inject.rs`
- Create: `simse-core/tests/library.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Cover: Library add/search/recommend/delete/compendium/find_duplicates, shelf scoping (metadata.shelf filtering), query DSL parsing, memory context formatting (structured and natural).

**Step 2: Run tests to fail**

**Step 3: Implement**

- `Library` struct wrapping `simse_vector::store::VolumeStore` directly
- All methods delegate to store with appropriate type conversion
- `Shelf` struct: thin wrapper adding `metadata.shelf = name` filter
- `parse_query()` — DSL parsing
- `format_memory_context()` — structured/natural formatting

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/library/ simse-core/tests/library.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): library orchestration wrapping simse-vector directly"
```

---

### Task 18: Librarian and registry

Ports: `src/ai/library/librarian.ts` + `librarian-definition.ts` + `librarian-registry.ts` + `circulation-desk.ts` + `library-services.ts` (~1,175 lines)

**Files:**
- Create: `simse-core/src/library/librarian.rs`
- Create: `simse-core/src/library/librarian_def.rs`
- Create: `simse-core/src/library/librarian_reg.rs`
- Create: `simse-core/src/library/circulation.rs`
- Create: `simse-core/src/library/services.rs`
- Create: `simse-core/tests/librarian.rs`

**Step 1: Write failing tests**

Cover: librarian extract/summarize/classify, definition validation (kebab-case name, non-empty fields), registry resolution with bidding, circulation desk job processing, library services system prompt enrichment.

**Step 2: Run tests to fail**

**Step 3: Implement**

- `Librarian` struct calling ACP for LLM operations (extract, summarize, classify, reorganize, optimize)
- `LibrarianDefinition` with validation + file persistence
- `LibrarianRegistry` — multi-librarian management, topic-based resolution with bidding
- `CirculationDesk` — `tokio::sync::mpsc` job queue with threshold-based escalation
- `LibraryServices` — middleware hooking library into agentic loop

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/library/librarian.rs simse-core/src/library/librarian_def.rs simse-core/src/library/librarian_reg.rs simse-core/src/library/circulation.rs simse-core/src/library/services.rs simse-core/tests/librarian.rs
git commit -m "feat(simse-core): librarian, registry, circulation desk, library services"
```

---

## Phase 8: VFS Orchestration

### Task 19: VFS orchestration

Ports: `src/ai/vfs/vfs.ts` + `vfs-disk.ts` + `exec.ts` + `validators.ts` (~1,204 lines)

**Files:**
- Create: `simse-core/src/vfs/mod.rs`
- Create: `simse-core/src/vfs/vfs.rs`
- Create: `simse-core/src/vfs/disk.rs`
- Create: `simse-core/src/vfs/exec.rs`
- Create: `simse-core/src/vfs/validators.rs`
- Create: `simse-core/tests/vfs.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Cover: VirtualFs read/write/list/tree/search wrapping engine directly, VfsDisk commit/load with binary detection, file validators (JSON syntax, trailing whitespace), exec passthrough.

**Step 2: Run tests to fail**

**Step 3: Implement**

- `VirtualFs` wrapping `simse_vfs::vfs::VirtualFs` directly
- `VfsDisk` — disk commit/load with binary extension detection
- `VfsValidator` trait + JSON validator + whitespace validator
- `VfsExec` — command execution passthrough via `tokio::process::Command`

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/vfs/ simse-core/tests/vfs.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): VFS orchestration wrapping simse-vfs directly"
```

---

## Phase 9: Agentic Loop

### Task 20: Agentic loop

Ports: `src/ai/loop/agentic-loop.ts` + `types.ts` (~662 lines) — the most complex module

**Files:**
- Create: `simse-core/src/agentic_loop.rs`
- Create: `simse-core/tests/agentic_loop.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Cover: single-turn text response, multi-turn with tool calls, doom loop detection (3 identical calls → system warning injected), auto-compaction triggering (two-stage: prune then summarize), stream retry on transient errors, tool retry on transient output, abort via CancellationToken, hit turn limit, event publishing.

**Step 2: Run tests to fail**

**Step 3: Implement agentic_loop.rs**

The core cycle:
1. conversation.add_user(input)
2. For turn 1..=max_turns:
   a. Enrich system prompt (if library_services)
   b. Check abort
   c. Two-stage compaction (prune then summarize)
   d. Stream from ACP with retry (exponential backoff)
   e. Parse tool calls from response
   f. If no tool calls → return text result
   g. Execute tool calls with retry + doom loop detection
   h. Add results to conversation
   i. Continue loop
3. If loop exits → hit turn limit

Key structures:
- `AgenticLoopOptions`: max_turns, max_identical_tool_calls, compaction_prompt, sampling_params, stream/tool retry config
- `AgenticLoopResult`: final_text, turns, total_turns, hit_turn_limit, aborted, total_duration_ms, total_usage
- `LoopTurn`: turn number, type (text/tool_use), text, tool calls/results, duration, usage
- `LoopCallbacks`: on_stream_delta, on_tool_call_start/end, on_turn_complete, on_compaction, on_doom_loop, etc.
- CancellationToken from tokio-util replaces AbortSignal

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/agentic_loop.rs simse-core/tests/agentic_loop.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): agentic loop with doom detection, compaction, retry"
```

---

## Phase 10: Server & Hooks

### Task 21: Hook system

Ports: `src/hooks/hook-system.ts` + `types.ts` (~281 lines)

**Files:**
- Create: `simse-core/src/hooks.rs`
- Create: `simse-core/tests/hooks.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Cover: before hook blocking tool execution, after hook transforming results, transform hooks chaining sequentially, validate hooks collecting errors, register/unregister lifecycle.

**Step 2: Run tests to fail**

**Step 3: Implement hooks.rs**

- `HookSystem` struct with typed handler storage
- `HookEvent` enum: BeforeToolCall, AfterToolCall, ValidateToolResult, TransformSystemPrompt, TransformMessages, SessionCompacting
- `register()` returns unregister closure
- Before hooks can block; Transform hooks chain output→input

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/hooks.rs simse-core/tests/hooks.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): hook system with blocking, transforming, validating"
```

---

### Task 22: Session manager

Ports: `src/server/session-manager.ts` + `types.ts` (~295 lines)

**Files:**
- Create: `simse-core/src/server/mod.rs`
- Create: `simse-core/src/server/session.rs`
- Create: `simse-core/tests/session.rs`
- Modify: `simse-core/src/lib.rs`

**Step 1: Write failing tests**

Cover: create session, get/delete session, list sessions, fork session (clones conversation state), update status.

**Step 2: Run tests to fail**

**Step 3: Implement**

- `Session` struct: id, conversation, event_bus, status, timestamps
- `SessionManager`: create, get, delete, list, update_status, fork
- Fork clones conversation via to_json/from_json with fresh event bus

**Step 4: Run tests, verify pass**

**Step 5: Commit**

```bash
git add simse-core/src/server/ simse-core/tests/session.rs simse-core/src/lib.rs
git commit -m "feat(simse-core): session manager with fork support"
```

---

## Phase 11: Public API & CoreContext

### Task 23: CoreContext and lib.rs public API

**Files:**
- Create: `simse-core/src/context.rs`
- Modify: `simse-core/src/lib.rs`
- Create: `simse-core/tests/integration.rs`

**Step 1: Write failing tests**

```rust
use simse_core::*;

#[test]
fn test_core_context_creation() {
    // Verify CoreContext can be constructed with all engine references
    // (This requires real or mock engine instances)
}

#[test]
fn test_public_api_surface() {
    // Verify all expected types are accessible from simse_core::
    let _: error::SimseError;
    let _: config::AppConfig;
    // etc.
}
```

**Step 2: Run tests to fail**

**Step 3: Implement**

- `CoreContext` struct holding all engine references + events + logger + config
- `CoreContext::new(config)` — initializes all engines from config
- `lib.rs` — final pub mod list, re-exports of key types

```rust
// lib.rs final form
pub mod error;
pub mod config;
pub mod logger;
pub mod events;
pub mod conversation;
pub mod tasks;
pub mod utils;
pub mod prompts;
pub mod chain;
pub mod agent;
pub mod tools;
pub mod library;
pub mod vfs;
pub mod agentic_loop;
pub mod hooks;
pub mod server;
pub mod context;

// Re-export key types at crate root
pub use error::SimseError;
pub use config::AppConfig;
pub use context::CoreContext;
pub use conversation::Conversation;
pub use tasks::TaskList;
pub use events::EventBus;
pub use logger::Logger;
```

**Step 4: Run tests, verify pass**

**Step 5: Full test suite**

Run: `cd simse-core && cargo test`
Expected: All tests pass

**Step 6: Commit**

```bash
git add simse-core/src/context.rs simse-core/src/lib.rs simse-core/tests/integration.rs
git commit -m "feat(simse-core): CoreContext and public API surface"
```

---

## Phase 12: Cleanup

### Task 24: Final verification and CLAUDE.md update

**Step 1: Run full Rust test suite**

```bash
cd simse-core && cargo test
cd simse-vector && cargo test
cd simse-vfs && cargo test
cd simse-acp && cargo test
cd simse-mcp && cargo test
```

Expected: All pass

**Step 2: Run clippy**

```bash
cd simse-core && cargo clippy -- -D warnings
```

Expected: No warnings

**Step 3: Update CLAUDE.md**

Add simse-core to repository layout, module layout, and key patterns sections.

**Step 4: Update package.json**

Add build script: `"build:core": "cd simse-core && cargo build --release"`

**Step 5: Commit**

```bash
git add CLAUDE.md package.json
git commit -m "docs: update CLAUDE.md and package.json for simse-core"
```

---

## Summary

| Phase | Tasks | Estimated Lines |
|-------|-------|-----------------|
| 1. Foundation | 1-5 (scaffold, error, logger, events, config) | ~1,500 |
| 2. Core Data | 6-7 (conversation, tasks) | ~600 |
| 3. Utils | 8 (retry, circuit breaker, health, timeout) | ~400 |
| 4. Prompts | 9 (system prompt builder) | ~300 |
| 5. Chain & Agent | 10-12 (template, executor, chain) | ~800 |
| 6. Tools | 13-16 (registry, builtin, host, subagent) | ~2,000 |
| 7. Library | 17-18 (library API, librarian system) | ~1,200 |
| 8. VFS | 19 (VFS orchestration) | ~500 |
| 9. Agentic Loop | 20 (the core loop) | ~400 |
| 10. Server & Hooks | 21-22 (hooks, sessions) | ~350 |
| 11. Public API | 23 (CoreContext, lib.rs) | ~150 |
| 12. Cleanup | 24 (verification, docs) | ~50 |
| **Total** | **24 tasks** | **~8,250 lines** |
