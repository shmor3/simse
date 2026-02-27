# Ink UI Overhaul — Claude Code Visual Parity

## Goal

Rewrite all React Ink display components to pixel-match Claude Code's terminal UI. Keep simse branding (ASCII "S" mascot) but adopt CC's layout patterns, colors, spacing, and interaction model.

## Scope

All 8 areas: tool calls, messages, input, banner, status bar, thinking spinner, permission dialog, markdown rendering.

## Component Designs

### 1. Tool Call Display

Replace bordered `ToolCallBox` with compact inline format:

```
  ⏺ Read(src/lib.ts)
    ⎿  150 lines (42ms)
```

- Magenta `⏺` prefix (active: spinner, completed: `⏺`, failed: red `⏺`)
- Bold tool name + primary arg in parens
- Result lines use `⎿` tree connector, indented 4 spaces
- Duration in dim parens
- Errors in red after `⎿`
- Diffs shown inline with +/- coloring

### 2. Message Rendering

**User:**
```
❯ user input text
```
Bold cyan `❯`, bold text.

**Assistant:**
Full markdown rendering, indented 2 spaces. No bullet prefix for text body. Markdown features: bold, italic, inline code (cyan), fenced code blocks (dim `│` gutter + language label), headers (bold cyan), lists, blockquotes (dim `│`), horizontal rules (`─────`).

### 3. Input Area

```
❯ Send a message...
```

- Bold cyan `❯` prompt
- Dimmed placeholder when empty
- Multi-line: Shift+Enter for newlines, Enter to submit
- Cursor rendered with inverse style
- Badges (`[PLAN]`, `[VERBOSE]`) rendered before prompt marker

### 4. Banner

Single bordered box with round corners:

```
╭──────────────────────────────────────────────────────────╮
│                                                          │
│       ╭──╮       Tips                                    │
│       ╰─╮│       Run /help for all commands              │
│         ╰╯       Use /add <text> to save a note          │
│                                                          │
│   simse-code v1.0.0                                      │
│   ollama: llama3                                         │
│   D:\GitHub\simse                                        │
│                                                          │
╰──────────────────────────────────────────────────────────╯
```

Compact layout. Mascot + tips in upper half, metadata in lower half.

### 5. Status Bar

```
  model · 12.3k tokens · $0.04                    [PLAN] [VERBOSE]
```

Dim left side, colored badges right side. No borders.

### 6. Thinking Spinner

```
  ⏺ Thinking...  (3.2s · 1.5k tokens · claude-3.5-sonnet)
```

Same visual style as tool calls. Rotating verbs from existing `THINKING_VERBS`. Suffix with elapsed time, tokens, server name.

### 7. Permission Dialog

```
  ⚠  simse wants to run Bash(rm -rf node_modules)

     Allow? [y]es / [n]o / [a]lways
```

Yellow warning, tool call display, keyboard hints.

### 8. Markdown Component

Ink-native `<Markdown>` component rendering:
- Bold/italic/underline
- Inline code in cyan
- Fenced code blocks with dim `│` gutter
- Headers (h1: bold cyan, h2: bold, h3: underline)
- Lists with `-` bullets
- Blockquotes with dim `│`
- Horizontal rules as `─────`

## Files to Create/Modify

### New files:
- `components/chat/tool-call-inline.tsx` — new compact tool call component
- `components/chat/markdown.tsx` — Ink markdown renderer component
- `components/input/multi-line-input.tsx` — multi-line text input

### Modified files:
- `components/chat/message-list.tsx` — use new tool call + markdown components
- `components/input/prompt-input.tsx` — use `❯` prompt, integrate multi-line input
- `components/layout/banner.tsx` — simplified single-box layout
- `components/layout/status-bar.tsx` — compact CC-style bar
- `components/input/permission-dialog.tsx` — CC-style permission display
- `components/shared/spinner.tsx` — CC-style thinking spinner with suffix
- `app-ink.tsx` — wire new components, update active area rendering

## Non-Goals

- Syntax highlighting in code blocks (plain dim gutter is sufficient)
- Collapsible/expandable tool results (future enhancement)
- Theme customization beyond CC defaults
