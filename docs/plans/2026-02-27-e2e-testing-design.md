# E2E Testing Design for simse-code Terminal UI

**Date:** 2026-02-27
**Status:** Approved

## Problem

simse-code is a terminal UI built with Ink (React for CLI) that has zero E2E test coverage. There are 74 unit/integration test files covering core modules, but no tests exercise the actual terminal interface — commands, prompts, streaming, tool calls, permission dialogs, or any real user interaction flow. This makes it impossible to catch rendering bugs, stale closure issues, race conditions, or broken user flows without manual testing.

## Solution

A real PTY-based E2E test harness using `node-pty` + `@xterm/headless` + `bun:test` that spawns the actual CLI in a pseudo-terminal and asserts against parsed screen state.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│  bun:test                                                │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  SimseTerminal (test harness)                       │ │
│  │  ┌──────────┐   raw ANSI   ┌───────────────────┐   │ │
│  │  │ node-pty │ ──────────►  │ @xterm/headless   │   │ │
│  │  │ (spawn)  │              │ (VT100 emulator)  │   │ │
│  │  └────┬─────┘              └────────┬──────────┘   │ │
│  │       │ stdin                       │ screen buffer │ │
│  │       ▼                             ▼              │ │
│  │  proc.write()              vt.buffer.active        │ │
│  │  '/help\r'                 getLine(n).translate()  │ │
│  │                                                    │ │
│  │  ┌──────────────────────────────────────────────┐  │ │
│  │  │  Assertions                                  │  │ │
│  │  │  waitForText('Available commands')            │  │ │
│  │  │  waitForPrompt()                             │  │ │
│  │  │  waitForIdle()                               │  │ │
│  │  └──────────────────────────────────────────────┘  │ │
│  └─────────────────────────────────────────────────────┘ │
│                                                          │
│  ┌─────────────────────────────────────────────────────┐ │
│  │  Temp Config (per test, isolated)                   │ │
│  │  /tmp/simse-e2e-xyz/                                │ │
│  │    acp.json  → claude / ollama / none               │ │
│  │    config.json, mcp.json, memory.json               │ │
│  └─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

### Core Stack

| Component | Purpose |
|-----------|---------|
| `node-pty` | Spawn CLI in real pseudo-terminal (cross-platform, Windows via conpty) |
| `@xterm/headless` | Parse raw ANSI output into structured screen buffer |
| `bun:test` | Test runner with `describe`, `it`, `expect` |
| Temp `--data-dir` | Isolated config per test, no shared state |
| `--bypass-permissions` | Skip permission dialogs in tests |

### Key Design Decisions

1. **Real PTY** — Ink only renders correctly when `isTTY=true`. Using `node-pty` gives the CLI a real terminal.
2. **Screen buffer assertions** — `@xterm/headless` parses ANSI codes into a rows×cols grid. Assert against screen content, not raw escape sequences.
3. **Structure over content** — For ACP-backed tests, assert UI structure (message appeared, tool call box rendered, status bar updated) not exact AI response text.
4. **Isolated config** — Each test creates a temp directory with its own `acp.json`, `config.json`, etc. Tests don't share state.
5. **Two real ACP backends** — Claude Code (`ANTHROPIC_API_KEY` required) and Ollama (local). Selectable via `ACP_BACKEND` env var.

## Test Harness API

```typescript
interface SimseTerminalOptions {
  cols?: number;                // default 120
  rows?: number;                // default 40
  acpBackend?: 'claude' | 'ollama' | 'none';
  bypassPermissions?: boolean;  // default true
  timeout?: number;             // default 30_000ms
  env?: Record<string, string>;
}

interface SimseTerminal {
  // Input
  type(text: string): void;
  submit(text: string): void;
  pressKey(key: 'enter' | 'escape' | 'tab' | 'up' | 'down' | ...): void;
  pressCtrl(char: string): void;

  // Screen reading
  getScreen(): string;
  getLine(row: number): string;
  getCursorPosition(): { row: number; col: number };

  // Polling assertions (with timeout)
  waitForText(text: string, opts?: { timeout?: number }): Promise<void>;
  waitForNoText(text: string, opts?: { timeout?: number }): Promise<void>;
  waitForPrompt(): Promise<void>;
  waitForIdle(): Promise<void>;

  // Structural assertions
  hasToolCallBox(name?: string): boolean;
  hasErrorBox(): boolean;
  hasSpinner(): boolean;
  hasBanner(): boolean;
  hasStatusBar(): boolean;
  hasPermissionDialog(): boolean;

  // Lifecycle
  kill(): Promise<void>;
}

function createSimseTerminal(opts?: SimseTerminalOptions): Promise<SimseTerminal>;
```

### Usage Example

```typescript
import { describe, it, afterEach } from 'bun:test';
import { createSimseTerminal } from './harness/index.js';

describe('E2E: /help command', () => {
  let term: SimseTerminal;
  afterEach(async () => { await term?.kill(); });

  it('displays available commands', async () => {
    term = await createSimseTerminal({ acpBackend: 'none' });
    await term.waitForPrompt();
    term.submit('/help');
    await term.waitForText('Available commands');
    await term.waitForText('/search');
  });
});
```

## Test Matrix

### Tier 1: Every Command (35 commands)

#### Meta (6)

| Command | Test | ACP |
|---------|------|-----|
| `/help` | Lists all categories and all 35 commands | No |
| `/help <cmd>` | Shows usage for specific command | No |
| `/clear` | Clears conversation history, screen resets | No |
| `/verbose` | Badge toggles on/off in prompt line | No |
| `/plan` | Badge toggles on/off in prompt line | No |
| `/context` | Shows context stats | No |
| `/exit` (+ `/quit`, `/q`) | Process exits cleanly | No |

#### AI (2)

| Command | Test | ACP |
|---------|------|-----|
| `/prompts` | Lists available prompt templates | No |
| `/chain <name>` | Runs a chain, renders output | Yes |

#### Library (7)

| Command | Test | ACP |
|---------|------|-----|
| `/add <text>` | Confirms note saved, assigns ID | No |
| `/search <query>` | Returns ranked results with scores | No |
| `/recommend` | Returns recommendations | No |
| `/topics` | Shows topic hierarchy | No |
| `/notes` | Lists notes, optionally filtered by topic | No |
| `/get <id>` | Retrieves specific note by ID | No |
| `/delete <id>` | Deletes note, confirms removal | No |

#### Tools (3)

| Command | Test | ACP |
|---------|------|-----|
| `/tools` | Lists registered tools | No |
| `/agents` | Lists available agents | No |
| `/skills` | Lists available skills | No |

#### Session (8)

| Command | Test | ACP |
|---------|------|-----|
| `/server` | Shows/sets active server | Yes |
| `/agent` | Shows/sets active agent | Yes |
| `/model` | Shows/sets active model | Yes |
| `/mcp` | Shows MCP connection status | No |
| `/acp` | Shows ACP connection status | Yes |
| `/library` / `/memory` | Toggles library integration | No |
| `/bypass-permissions` | Toggles permission bypass | No |
| `/embed <text>` | Generates and displays embedding | Yes |

#### Files (5)

| Command | Test | ACP |
|---------|------|-----|
| `/files` | Lists VFS files | No |
| `/save` | Saves VFS files to disk | No |
| `/validate` | Validates VFS file contents | No |
| `/discard` | Discards VFS changes | No |
| `/diff` | Shows VFS file diffs | No |

#### Config (4)

| Command | Test | ACP |
|---------|------|-----|
| `/config` | Shows current configuration | No |
| `/settings` | Views/updates settings | No |
| `/init` | Initializes project config | No |
| `/setup claude-code` | Writes acp.json, hot-reloads | No |
| `/setup ollama` | Configures Ollama, connects | No |

### Tier 2: ACP Prompt & Streaming

| Test | Backend | Assertion |
|------|---------|-----------|
| Simple prompt | Both | User msg shown, spinner, response rendered, idle |
| Multi-turn conversation | Both | 2+ prompts, all in scroll history |
| Long streaming response | Both | Progressive text updates |
| Abort mid-stream (Ctrl+C) | Both | Streaming stops, prompt recovers |
| Plan mode prompt | Both | Plan badge visible during interaction |
| Verbose mode output | Both | Extra output shown |

### Tier 3: Tool Call Lifecycle

| Test | Backend | Assertion |
|------|---------|-----------|
| Tool call render | Both | Cyan box during, green on complete |
| Tool call failure | Both | Red box with error |
| Multiple sequential tools | Both | Multiple boxes in order |
| Tool call duration | Both | Duration shown on complete |
| Permission dialog (no -y) | Claude | Dialog renders, y/n/a works |

### Tier 4: Subagent Lifecycle

| Test | Backend | Assertion |
|------|---------|-----------|
| Subagent spawn via tool | Claude | Subagent tool call box appears |
| Subagent shelf isolation | Claude | Writes to own shelf, not main library |
| Subagent completion | Claude | Result returned to parent |
| Subagent failure | Claude | Error propagated, parent continues |

### Tier 5: MCP Integration

| Test | Backend | Assertion |
|------|---------|-----------|
| MCP server connection | Both | `/mcp` shows connected servers |
| MCP tool discovery | Both | `/tools` includes MCP tools |
| MCP tool execution | Claude | Prompt triggers MCP tool, result rendered |

### Tier 6: Chain Execution

| Test | Backend | Assertion |
|------|---------|-----------|
| `/chain <name>` | Both | Chain runs, output rendered |
| `/chain` with template vars | Both | Variables interpolated |
| Chain error handling | Both | Missing chain shows error |

### Tier 7: Library Memory System

| Test | Backend | Assertion |
|------|---------|-----------|
| Embedded model init (simse-engine) | No | Local embedder loads, search works |
| Add + search roundtrip | No | `/add` then `/search` finds the note |
| Topic auto-extraction | No | Notes get auto-classified |
| Deduplication | No | Duplicate content detected |
| `/recommend` with history | No | Recommendations reflect usage |
| Library memory in prompts | Claude | Memory context injected |
| Memory optimization (large model) | Claude | Librarian extract/summarize/classify |
| Compendium generation | Claude | Multiple notes condensed |

### Tier 8: Conversation Management

| Test | Backend | Assertion |
|------|---------|-----------|
| Auto-compaction trigger | Claude | Long convo triggers compaction |
| `/compact` manual | Claude | Conversation compacted, stats reduced |
| `/clear` + resume | Both | History cleared, new prompt works |
| Context window tracking | Both | `/context` stats update |

### Tier 9: UI Behavior

| Test | Backend | Assertion |
|------|---------|-----------|
| TextInput cursor movement | No | Arrow keys, backspace, insert at cursor |
| Banner rendering | No | Mascot + tips + model info |
| Banner responsive widths | No | Correct at 80, 120, 200 cols |
| Status bar live updates | Claude | Tokens, cost update |
| Error box rendering | No | Red bordered box |

### Tier 10: Error & Edge Cases

| Test | Backend | Assertion |
|------|---------|-----------|
| Invalid command | No | Error message, prompt recovers |
| ACP server crash | Claude | Error shown, prompt recovers |
| Empty prompt submission | No | Nothing happens or warning |
| Very long input | No | Handles overflow gracefully |
| Rapid consecutive inputs | No | No race conditions |
| Config missing (first run) | No | Degraded mode, /setup offered |

**Total: ~80+ test cases across 10 tiers.**

## File Organization

```
simse-code/
  e2e/
    harness/
      terminal.ts           # SimseTerminal: node-pty + @xterm/headless wrapper
      config.ts             # Temp config scaffolding per test
      assertions.ts         # waitForText, waitForPrompt, waitForIdle
      keys.ts               # Escape sequences for special keys
      index.ts              # Barrel export
    fixtures/
      prompts.json          # Test prompt templates for /chain
      agents/               # Test agent .md files
      skills/               # Test skill definitions
    commands/
      meta.e2e.ts           # /help, /clear, /verbose, /plan, /context, /exit
      ai.e2e.ts             # /chain, /prompts
      library.e2e.ts        # /add, /search, /recommend, /topics, /notes, /get, /delete
      tools.e2e.ts          # /tools, /agents, /skills
      session.e2e.ts        # /server, /agent, /model, /mcp, /acp, /library, /bypass, /embed
      files.e2e.ts          # /files, /save, /validate, /discard, /diff
      config.e2e.ts         # /config, /settings, /init, /setup
    flows/
      prompt-streaming.e2e.ts    # Prompt submission, streaming, multi-turn
      tool-calls.e2e.ts          # Tool call lifecycle, permission dialog
      subagents.e2e.ts           # Subagent spawn, shelf isolation, completion
      mcp-integration.e2e.ts     # MCP server connection, tool discovery
      chain-execution.e2e.ts     # Chain runs, template vars, errors
      library-memory.e2e.ts      # Embedded model, add/search, topics, dedup, compendium
      memory-optimization.e2e.ts # Librarian with large model, optimization
      conversation.e2e.ts        # Compaction, clear, context tracking
    ui/
      text-input.e2e.ts     # Cursor movement, rapid input
      banner.e2e.ts         # Banner rendering, responsive widths
      status-bar.e2e.ts     # Live status bar updates
      error-states.e2e.ts   # Error rendering, recovery
    startup.e2e.ts          # Startup with/without ACP, first-run
```

## Test Execution

```bash
# All E2E tests (no ACP — command tests only)
bun test e2e/

# With Claude backend
ACP_BACKEND=claude bun test e2e/

# With Ollama backend
ACP_BACKEND=ollama bun test e2e/

# By category
bun test e2e/commands/          # All command tests
bun test e2e/flows/             # All flow tests
bun test e2e/ui/                # UI behavior tests

# Quick smoke
bun test e2e/commands/ e2e/startup.e2e.ts
```

### ACP Backend Selection

```typescript
type ACPBackend = 'claude' | 'ollama' | 'none';

function getACPBackend(): ACPBackend {
  return (process.env.ACP_BACKEND as ACPBackend) ?? 'none';
}

// Tests that need ACP:
describe.skipIf(getACPBackend() === 'none')('prompt streaming', () => { ... });

// Tests on both backends:
for (const backend of ['claude', 'ollama']) {
  describe.skipIf(getACPBackend() !== backend)(`prompt with ${backend}`, () => { ... });
}
```

## Timeout Strategy

| Test type | Timeout | Rationale |
|-----------|---------|-----------|
| Command (no ACP) | 10s | Startup + command only |
| Prompt (ACP) | 60s | Streaming response time |
| Tool call (ACP) | 90s | Tool execution + response |
| Library (embedded model) | 120s | ONNX model download |
| Subagent | 120s | Nested loop execution |

## Dependencies

```json
{
  "devDependencies": {
    "node-pty": "^1.0.0",
    "@xterm/headless": "^5.5.0"
  }
}
```

## CI Strategy

Three stages, progressively heavier:

1. **e2e-commands** — No dependencies, runs on every PR. Tests all 35 commands + UI behavior.
2. **e2e-ollama** — Requires Ollama running. Tests prompt streaming, tool calls, chains with local model.
3. **e2e-claude** — Requires `ANTHROPIC_API_KEY`. Tests full flows including subagents, memory optimization, compendium.
