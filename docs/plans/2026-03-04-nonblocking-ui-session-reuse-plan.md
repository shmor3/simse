# Non-Blocking UI & ACP Session Reuse Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Make the TUI event loop non-blocking during ACP calls and reuse the existing ACP session instead of creating a new one per turn.

**Architecture:** Wrap `TuiRuntime` in `Arc<Mutex<>>` so it can be shared between the main event loop and a spawned tokio task. Move `handle_submit` into `tokio::spawn` so the `tokio::select!` loop continues processing terminal events (redraws, spinner, streaming deltas) while the agentic loop runs. Pass the existing `session_id` from `TuiRuntime` into `AgenticLoopOptions` so `run_agentic_loop` reuses it.

**Tech Stack:** Rust, tokio, ratatui, crossterm, simse-bridge (AcpClient, agentic_loop), simse-ui-core

---

### Task 1: Fix StreamEnd to reset loop_status to Idle

**Files:**
- Modify: `simse-tui/src/app.rs:588-594`

**Step 1: Update the existing test to assert Idle**

In `simse-tui/src/app.rs`, find the test `stream_end_moves_to_output` (~line 1254) and add:

```rust
assert_eq!(app.loop_status, LoopStatus::Idle);
```

**Step 2: Run test to verify it fails**

Run: `cd simse-tui && cargo test --lib stream_end_moves_to_output`
Expected: FAIL — `loop_status` is still `Streaming`

**Step 3: Add `loop_status = Idle` to StreamEnd handler**

In `simse-tui/src/app.rs`, modify the `StreamEnd` handler:

```rust
AppMessage::StreamEnd { text } => {
    app.output.push(OutputItem::Message {
        role: "assistant".into(),
        text,
    });
    app.stream_text.clear();
    app.loop_status = LoopStatus::Idle;
}
```

**Step 4: Run test to verify it passes**

Run: `cd simse-tui && cargo test --lib stream_end_moves_to_output`
Expected: PASS

**Step 5: Run full test suite**

Run: `cd simse-tui && cargo test --lib`
Expected: All 869+ tests pass

**Step 6: Commit**

```bash
git add simse-tui/src/app.rs
git commit -m "fix(simse-tui): reset loop_status to Idle on StreamEnd"
```

---

### Task 2: Pass session_id into AgenticLoopOptions for reuse

**Files:**
- Modify: `simse-bridge/src/agentic_loop.rs` (AgenticLoopOptions struct + run_agentic_loop)
- Modify: `simse-tui/src/event_loop.rs:188-194` (handle_submit builds options)

**Step 1: Add session_id to AgenticLoopOptions**

In `simse-bridge/src/agentic_loop.rs`, add the field:

```rust
pub struct AgenticLoopOptions {
    pub max_turns: usize,
    pub server_name: Option<String>,
    pub agent_id: Option<String>,
    pub system_prompt: Option<String>,
    pub agent_manages_tools: bool,
    /// Existing ACP session ID to reuse. If None, a new session is created per turn.
    pub session_id: Option<String>,
}
```

Update the `Default` impl to include `session_id: None`.

**Step 2: Use session_id in run_agentic_loop instead of creating new sessions**

Replace the `new_session()` call inside the loop (around line 312-328):

```rust
// Reuse provided session or create a new one for this turn
let session_id = if let Some(ref sid) = options.session_id {
    sid.clone()
} else {
    match acp_client.new_session().await {
        Ok(id) => id,
        Err(e) => {
            let error_msg = format!("Failed to create ACP session: {e}");
            callbacks.on_error(&error_msg);
            turns.push(LoopTurn {
                turn: turn_num,
                turn_type: TurnType::Error,
                text: Some(error_msg.clone()),
                tool_calls: Vec::new(),
                tool_results: Vec::new(),
            });
            final_text = error_msg;
            break;
        }
    }
};
```

**Step 3: Pass session_id in handle_submit**

In `simse-tui/src/event_loop.rs`, update handle_submit's options:

```rust
let options = AgenticLoopOptions {
    max_turns: 10,
    server_name: self.config.default_server.clone(),
    agent_id: self.config.default_agent.clone(),
    system_prompt: self.config.workspace_prompt.clone(),
    agent_manages_tools: false,
    session_id: self.session_id.clone(),
};
```

**Step 4: Run tests**

Run: `cd simse-bridge && cargo test` and `cd simse-tui && cargo test --lib`
Expected: All pass (existing tests use Default which now includes `session_id: None`)

**Step 5: Commit**

```bash
git add simse-bridge/src/agentic_loop.rs simse-tui/src/event_loop.rs
git commit -m "feat(agentic-loop): reuse ACP session instead of creating new one per turn"
```

---

### Task 3: Make TuiRuntime shareable with Arc<Mutex>

**Files:**
- Modify: `simse-tui/src/main.rs` (wrap runtime in Arc<Mutex<>>)

**Step 1: Wrap runtime in Arc<Mutex<TuiRuntime>>**

In `simse-tui/src/main.rs`, change `run_app`:

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

// Change:
//   let mut runtime = event_loop::TuiRuntime::new(config);
//   runtime.verbose = cli.verbose;
// To:
let mut rt = event_loop::TuiRuntime::new(config);
rt.verbose = cli.verbose;
let runtime = Arc::new(Mutex::new(rt));
```

**Step 2: Update all runtime calls to lock first**

Every `runtime.xxx()` call becomes `runtime.lock().await.xxx()`. The key call sites in `run_app`:

- `runtime.is_connected()` → `runtime.lock().await.is_connected()`
- `runtime.needs_onboarding()` → `runtime.lock().await.needs_onboarding()`
- `runtime.connect().await` → `runtime.lock().await.connect().await`
- `runtime.build_command_context()` → `runtime.lock().await.build_command_context()`
- `runtime.dispatch_bridge_action(action).await` → `runtime.lock().await.dispatch_bridge_action(action).await`
- `runtime.handle_submit(...)` — will be moved to spawn in Task 4, handle there

**Step 3: Build and verify**

Run: `cd simse-tui && cargo build --release`
Expected: Compiles (tests aren't affected — they don't use main.rs)

**Step 4: Commit**

```bash
git add simse-tui/src/main.rs
git commit -m "refactor(simse-tui): wrap TuiRuntime in Arc<Mutex> for sharing"
```

---

### Task 4: Spawn handle_submit as background task

**Files:**
- Modify: `simse-tui/src/main.rs:118-150` (the pending_chat_message block)

**Step 1: Replace blocking handle_submit with tokio::spawn**

Replace the entire `pending_chat_message` block in `main.rs`:

```rust
// Dispatch pending chat message through the agentic loop.
if let Some(text) = app.pending_chat_message.take() {
    // Lazily connect to ACP on first chat message.
    if !runtime.lock().await.is_connected() && !runtime.lock().await.needs_onboarding() {
        match runtime.lock().await.connect().await {
            Ok(()) => {
                let ctx = runtime.lock().await.build_command_context();
                app = update(app, AppMessage::RefreshContext(ctx));
            }
            Err(e) => {
                app = update(
                    app,
                    AppMessage::LoopError(format!("ACP connection failed: {e}")),
                );
                continue;
            }
        }
    }

    let tx = msg_tx.clone();
    let callbacks = TuiLoopCallbacks { tx: tx.clone() };
    app = update(app, AppMessage::StreamStart);
    terminal.draw(|frame| view(&app, frame))?;

    // Spawn the agentic loop as a background task so the UI stays responsive.
    let rt = Arc::clone(&runtime);
    tokio::spawn(async move {
        match rt.lock().await.handle_submit(&text, &callbacks).await {
            Ok(final_text) => {
                let _ = tx.send(AppMessage::StreamEnd { text: final_text });
            }
            Err(e) => {
                let _ = tx.send(AppMessage::LoopError(e.to_string()));
            }
        }
    });
}
```

Key changes:
- `handle_submit` runs inside `tokio::spawn` — the event loop continues
- `StreamEnd` / `LoopError` are sent via `msg_tx` channel (picked up by the `tokio::select!` loop)
- `TuiLoopCallbacks` already sends `StreamDelta` via the same channel — now those will be processed during the await
- The `terminal.draw()` call before spawn ensures the spinner renders immediately

**Step 2: Build and test**

Run: `cd simse-tui && cargo build --release`
Expected: Compiles

Run: `cd simse-tui && cargo test --lib`
Expected: All pass (tests don't go through main.rs)

**Step 3: Commit**

```bash
git add simse-tui/src/main.rs
git commit -m "feat(simse-tui): spawn agentic loop as background task for non-blocking UI"
```

---

### Task 5: Guard against concurrent submits

**Files:**
- Modify: `simse-tui/src/app.rs:336-338` (Submit handler, chat path)

**Step 1: Write a test for the guard**

Add to `simse-tui/src/app.rs` tests:

```rust
#[test]
fn submit_ignored_while_streaming() {
    let mut app = App::new();
    app.loop_status = LoopStatus::Streaming;
    app.input = input::insert(&app.input, "new message");
    app = update(app, AppMessage::Submit);
    // Message should NOT be queued while loop is active
    assert!(app.pending_chat_message.is_none());
    // Input should NOT be cleared
    assert_eq!(app.input.value, "new message");
}
```

**Step 2: Run test to verify it fails**

Run: `cd simse-tui && cargo test --lib submit_ignored_while_streaming`
Expected: FAIL — pending_chat_message is set

**Step 3: Add guard in Submit handler**

In `simse-tui/src/app.rs`, in the `AppMessage::Submit` handler, after the screen match and autocomplete deactivation (around line 306-307), before trimming text, add:

```rust
// Don't accept new messages while the agentic loop is running.
if app.loop_status != LoopStatus::Idle {
    return app;
}
```

Note: place this AFTER `app.autocomplete.deactivate()` (line 306) but BEFORE `let text = app.input.value.trim()...` (line 307).

**Step 4: Run test to verify it passes**

Run: `cd simse-tui && cargo test --lib submit_ignored_while_streaming`
Expected: PASS

**Step 5: Run full test suite**

Run: `cd simse-tui && cargo test --lib && cd ../simse-bridge && cargo test`
Expected: All pass

**Step 6: Commit and push**

```bash
git add simse-tui/src/app.rs
git commit -m "feat(simse-tui): guard against concurrent message submits"
git push
```
