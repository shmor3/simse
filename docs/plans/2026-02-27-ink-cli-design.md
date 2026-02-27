# Ink CLI Refactor Design

**Date:** 2026-02-27
**Status:** Approved

## Goal

Rewrite simse-code's monolithic `cli.ts` (~4,300 lines) to use Ink (React for terminals), splitting rendering into React components and organizing code into feature modules for maintainability. The UI should match Claude Code's visual style: bordered tool call boxes, streaming text, interactive permission dialogs, status bar.

## Decisions

- **Full Claude Code style** — bordered tool calls, collapsible sections, streaming, status bar
- **Feature module architecture** — each module owns its commands, state, and components
- **Ink's built-in TextInput** for prompt input (fully within React's rendering model)
- **Clean break** from current AppContext — redesigned with React context providers and hooks
- **New dependencies:** ink, ink-text-input, ink-spinner, ink-select-input, react, ink-testing-library

## Architecture

### Directory Structure

```
simse-code/
├── cli.tsx                       # Entry point: parse args, mount <App />
├── app.tsx                       # Root <App /> component (provider tree + layout)
├── hooks/
│   ├── use-agentic-loop.ts       # Streaming AI conversation hook
│   ├── use-command-dispatch.ts   # Command routing hook
│   └── use-terminal.ts           # Terminal dimensions, raw mode
├── providers/
│   ├── services-provider.tsx     # ACP, MCP, embedder, VFS, library context
│   ├── theme-provider.tsx        # Theme/colors context
│   ├── session-provider.tsx      # Session, conversation, permissions context
│   └── input-provider.tsx        # Input history, current input, @-mentions
├── components/
│   ├── layout/
│   │   ├── main-layout.tsx       # Flex column: output + input + status
│   │   ├── status-bar.tsx        # Bottom bar (server, model, tokens, cost)
│   │   └── banner.tsx            # Welcome banner with service status
│   ├── chat/
│   │   ├── message-list.tsx      # <Static> for completed messages
│   │   ├── streaming-text.tsx    # Live streaming token display
│   │   ├── tool-call-box.tsx     # Bordered tool call (active/complete/failed)
│   │   ├── inline-diff.tsx       # Unified diff display inside tool calls
│   │   └── thinking.tsx          # Thinking indicator (verbose mode)
│   ├── input/
│   │   ├── prompt-input.tsx      # TextInput with history, @-mentions, badges
│   │   └── permission-dialog.tsx # Modal permission request [y/n/d/e]
│   └── shared/
│       ├── spinner.tsx           # Thinking spinner with rotating verbs
│       ├── error-box.tsx         # Error display component
│       └── badge.tsx             # [PLAN], [VERBOSE], [YOLO] badges
├── features/
│   ├── ai/
│   │   ├── commands.ts           # bare text, /chain, /prompts
│   │   ├── hooks.ts              # useAgenticLoop integration
│   │   └── index.ts
│   ├── library/
│   │   ├── commands.ts           # /add, /search, /notes, /topics, /get, /delete, /recommend
│   │   ├── components.tsx        # SearchResults, NoteList, TopicList, NoteDetail
│   │   └── index.ts
│   ├── tools/
│   │   ├── commands.ts           # /tools, /agents
│   │   ├── components.tsx        # ToolList, AgentList
│   │   └── index.ts
│   ├── session/
│   │   ├── commands.ts           # /server, /agent, /model, /mcp, /acp, /library, /bypass-permissions
│   │   ├── components.tsx        # ServerPicker, ModelPicker
│   │   └── index.ts
│   ├── files/
│   │   ├── commands.ts           # /files, /save, /validate, /discard, /diff
│   │   ├── components.tsx        # FileChanges, DiffView
│   │   └── index.ts
│   ├── config/
│   │   ├── commands.ts           # /config, /settings, /init
│   │   └── index.ts
│   └── meta/
│       ├── commands.ts           # /help, /status, /context, /verbose, /theme, /clear, /export,
│       │                         # /cost, /stats, /doctor, /permissions, /hooks, /todos,
│       │                         # /plan, /compact, /retry, /turns, /learning, /bg-tasks, /copy
│       ├── components.tsx        # HelpView, StatusView, ContextGrid, TodoList
│       └── index.ts
├── command-registry.ts           # Command registration, lookup, dispatch
├── types.ts                      # Shared types (Command, CommandResult, OutputItem)
└── package.json
```

### Component Tree

```
<ThemeProvider>
  <ServicesProvider>
    <SessionProvider>
      <InputProvider>
        <App>
          <MainLayout>
            <Static>                    ← Completed messages + tool results
              <MessageBubble />
              <ToolCallBox />
            </Static>
            <ActiveArea>                ← Currently streaming
              <StreamingText />
              <ToolCallBox status="active" />
              <PermissionDialog />      ← Overlay when permission needed
            </ActiveArea>
            <PromptInput />             ← TextInput with badges
            <StatusBar />              ← Server, model, tokens, cost
          </MainLayout>
        </App>
      </InputProvider>
    </SessionProvider>
  </ServicesProvider>
</ThemeProvider>
```

### State Management

React context providers replace the monolithic AppContext:

| Provider | Hook | State |
|----------|------|-------|
| `ThemeProvider` | `useTheme()` | colors, current theme, setTheme |
| `ServicesProvider` | `useServices()` | app, acpClient, vfs, disk, toolRegistry, skillRegistry |
| `SessionProvider` | `useSession()` | conversation, permissionMode, serverName, agentName, libraryEnabled, maxTurns, abortController |
| `InputProvider` | `useInput()` | history, submit(), abort(), currentInput |

### Command System

```ts
interface Command {
  name: string;
  aliases?: string[];
  usage: string;
  description: string;
  category: 'ai' | 'library' | 'tools' | 'session' | 'files' | 'config' | 'meta';
  execute: (args: string) => CommandResult;
}

type CommandResult = {
  element?: React.ReactNode;  // JSX to render
  text?: string;              // Plain text (rendered as <Text>)
};
```

Commands are registered per feature module. The command registry collects all commands and provides lookup by name/alias.

### Tool Call Rendering

Claude Code-style bordered boxes:

```
  ┌─ vfs_write ──────────────────────────┐
  │ path: /src/main.ts                    │
  │                                       │
  │ --- a/src/main.ts                     │
  │ +++ b/src/main.ts                     │
  │ @@ -1,3 +1,5 @@                       │
  │  import { foo } from './bar.js';      │
  │ +import { baz } from './qux.js';      │
  │                                       │
  │ ✓ 125ms                               │
  └───────────────────────────────────────┘
```

States: active (spinning border color), completed (green check + duration), failed (red X + error).

### Permission Dialog

Renders as a modal overlay in the active area:

```
  ┌─ Permission ────────────────────────┐
  │ Allow vfs_write to /src/main.ts?    │
  │ {"path": "/src/main.ts", ...}       │
  │                                     │
  │ [y] Allow  [n] Deny  [d] Diff      │
  └─────────────────────────────────────┘
```

Uses `useInput` to capture single keypress responses.

### Streaming Text

The `useAgenticLoop` hook manages streaming state:

```ts
{
  state: 'idle' | 'streaming' | 'tool-executing';
  streamTokens: string;           // Accumulating text tokens
  activeToolCalls: ToolCallState[];
  completedItems: OutputItem[];   // Moved to <Static> when done
  submit: (input: string) => Promise<void>;
  abort: () => void;
}
```

### Status Bar

Single-line fixed footer:

```
  server:model · 1.2k tokens · $0.03 · [PLAN] [VERBOSE]
```

Updates reactively via `useSession()` hook.

## Dependencies

```json
{
  "dependencies": {
    "simse": "file:..",
    "ink": "^5.2.0",
    "ink-text-input": "^6.0.0",
    "ink-spinner": "^5.0.0",
    "ink-select-input": "^6.0.0",
    "react": "^18.3.0"
  },
  "devDependencies": {
    "ink-testing-library": "^4.0.0",
    "@types/react": "^18.3.0"
  }
}
```

## Migration Strategy

1. Install Ink dependencies
2. Build providers + shared components (theme, services, session, input)
3. Build layout components (main-layout, status-bar, banner)
4. Build chat components (message-list, streaming-text, tool-call-box, permission-dialog)
5. Build input components (prompt-input with TextInput)
6. Build command registry + migrate feature modules one category at a time
7. Wire up the agentic loop hook
8. Replace cli.ts entry point with cli.tsx
9. Update/write tests using ink-testing-library
10. Remove old ui.ts rendering functions (keep non-UI utilities)

## Files Preserved

These existing simse-code files are NOT affected by this refactor:
- `app.ts` — knowledge base facade (logic, not UI)
- `app-context.ts` — replaced by React context providers
- `loop.ts` — agentic loop integration (wrapped by useAgenticLoop hook)
- `conversation.ts`, `providers.ts`, `storage.ts` — business logic, unchanged
- `tool-registry.ts`, `tools.ts`, `skills.ts` — tool/skill logic, unchanged
- `config.ts`, `setup.ts`, `agents.ts` — config logic, unchanged
- `file-tracker.ts`, `file-mentions.ts`, `image-input.ts` — feature logic, unchanged
- `diff-display.ts` — diff computation reused by InlineDiff component
- `status-line.ts` — replaced by StatusBar component
- `ui.ts` — replaced by Ink components (colors utility may be preserved)
- `picker.ts` — replaced by ink-select-input
- `todo-ui.ts` — replaced by TodoList component
