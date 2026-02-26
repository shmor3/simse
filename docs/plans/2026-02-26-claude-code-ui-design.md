# Claude Code UI/UX Overhaul

## Goal

Match Claude Code's terminal UI patterns across tool calls, spinner, status bar, sub-agents, and diff rendering.

## Changes

### 1. Tool Call Rendering (ui.ts)

**Before**: `● Reading src/lib.ts` / `✓ Read src/lib.ts (150 lines, 42ms)`
**After**: `● Read(src/lib.ts)` / `⎿ Read 150 lines (ctrl+o to expand)`

- Function-call style: `ToolVerb(primary_arg)` instead of `Verb arg`
- Completed tools keep `●` magenta bullet (no checkmark)
- Tool output: `⎿ Summary` with `(ctrl+o to expand)` hint
- Edit/write tools: show inline diff with line count summary

### 2. Status Spinner (ui.ts, cli.ts)

**Before**: `✢ Thinking...`
**After**: `* Cooking... (17m 4s · ↓ 25.1k tokens · thinking)`

- Track elapsed time from loop start
- Display token count from usage tracking
- Show thinking/processing state
- Use `*` prefix instead of animated frames (simpler, matches Claude Code)

### 3. Bottom Status Bar (status-line.ts)

**Before**: `model │ 65% │ +42 -17 │ [AUTO-EDIT]`
**After**: `🔒 bypass permissions on (shift+tab to cycle) · esc to interrupt`

- Red background for bypass/yolo mode
- Descriptive permission text instead of badges
- `esc to interrupt` hint
- Mode cycling hint

### 4. Sub-agent Display (ui.ts, cli.ts)

**Before**: Regular tool call format
**After**: `● Explore(description) Sonnet 4.6` / `⎿ Done (45 tool uses · 106.6k tokens · 3m 37s)`

- Model badge next to sub-agent name
- Rich completion summary with tool uses, tokens, duration
- `(ctrl+o to expand)` / `ctrl+b to run in background` hints

### 5. Diff Rendering (diff-display.ts)

**Before**: `+line` green text, `-line` red text
**After**: Red/green background highlighting for changed lines

- ANSI background colors for full-line highlighting
- Aligned line numbers
- Claude Code style diff blocks

## Files Modified

- `simse-code/ui.ts` — tool call renderers, spinner, sub-agent display
- `simse-code/status-line.ts` — bottom bar redesign
- `simse-code/diff-display.ts` — background color diffs
- `simse-code/cli.ts` — spinner integration, elapsed time tracking, callback wiring
