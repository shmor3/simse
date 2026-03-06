# Loading Spinner Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Wire the existing `ThinkingSpinner` widget into the TUI so it displays during the entire AI generation lifecycle (streaming + tool execution).

**Architecture:** TUI-only change (Approach A). The `ThinkingSpinner` already exists in `simse-tui/src/spinner.rs` with animation, verb selection, elapsed time, and token display. The work is purely wiring it into the Elm Architecture `App` model, driving it with a tick timer, and rendering it in the view.

**Tech Stack:** Rust, ratatui, tokio (for tick interval)

---

## State Transitions

```
StreamStart       → spinner = Some(ThinkingSpinner::new(server_name))
StreamDelta       → spinner.tick()
ToolCallStart     → spinner stays active (LoopStatus::ToolExecuting)
ToolCallEnd       → spinner stays active
TokenUsage        → spinner.set_token_count(total)
Tick              → spinner.tick() → redraw if changed
StreamEnd         → spinner = None
LoopComplete      → spinner = None
LoopError         → spinner = None
```

## View Rendering

In `render_chat_area`, when `app.spinner.is_some()`, render the spinner line after streaming text and active tool calls — always the last line in the chat area.

## Tick Timer

Main loop `tokio::select!` gains a third arm with `tokio::time::interval(120ms)`. Sends `AppMessage::Tick` which calls `spinner.tick()`.

## Files Changed

- `simse-tui/src/app.rs` — Add `spinner` field, `Tick` message, update/view wiring
- `simse-tui/src/main.rs` — Add tick interval arm, wire `on_tool_call_start`/`on_tool_call_end`/`on_usage_update` callbacks

No changes to `simse-ui-core` or `simse-core`.
