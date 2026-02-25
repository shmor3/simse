# Subagent Tools Design

## Problem

simse has an agentic loop with tool execution, but no way for the model to spawn child agent loops (subagents). Claude Code's Task tool lets agents delegate complex sub-tasks to autonomous child agents with progress reporting. simse needs equivalent capability.

## Approach

Follow the existing `registerMemoryTools()` / `registerTaskTools()` pattern. A new `registerSubagentTools(registry, options)` function registers two tools and wires child loop callbacks to parent display callbacks.

## Tools

### `subagent_spawn`

Spawns a nested `createAgenticLoop()` with its own conversation context. Runs autonomously to completion and returns `finalText` as the tool result.

Parameters:
- `task` (string, required) — the prompt/instruction for the child agent
- `description` (string, required) — short label for display (e.g. "Researching API endpoints")
- `maxTurns` (number, optional) — turn limit for the child loop
- `systemPrompt` (string, optional) — override system prompt for the child

### `subagent_delegate`

Single-shot ACP delegation via `acpClient.generate()`. For simple tasks that don't need multi-turn tool use.

Parameters:
- `task` (string, required) — the prompt
- `description` (string, required) — short label for display
- `serverName` (string, optional) — target a different ACP server
- `agentId` (string, optional) — target a different agent

## Callbacks

New fields on `LoopCallbacks`:

```
onSubagentStart(info: SubagentInfo)
onSubagentStreamDelta(id: string, text: string)
onSubagentToolCallStart(id: string, call: ToolCallRequest)
onSubagentToolCallEnd(id: string, result: ToolCallResult)
onSubagentComplete(id: string, result: SubagentResult)
onSubagentError(id: string, error: Error)
```

`SubagentInfo` contains `id` (unique per spawn), `description`, and `mode` ('spawn' | 'delegate').

`SubagentResult` contains `text`, `turns`, and `durationMs`.

## Registration API

```
registerSubagentTools(registry, {
  acpClient,
  toolRegistry,       // parent's registry, inherited by child
  callbacks?,
  defaultMaxTurns?,   // default 10
  maxDepth?,          // default 2 — prevents infinite nesting
  serverName?,
  agentId?,
  systemPrompt?,
})
```

## Recursion Control

Child loops inherit the parent's tool registry but subagent tools are re-registered with `depth + 1`. When `depth >= maxDepth`, subagent tools are omitted entirely. Default maxDepth is 2 (parent can spawn children, children can spawn grandchildren, grandchildren cannot spawn).

## File Layout

| File | Action |
|------|--------|
| `src/ai/tools/subagent-tools.ts` | Create — types + registerSubagentTools() |
| `src/ai/loop/types.ts` | Modify — add subagent callbacks to LoopCallbacks |
| `src/ai/tools/types.ts` | Modify — add 'subagent' to ToolCategory |
| `src/ai/tools/index.ts` | Modify — re-export registerSubagentTools |
| `src/lib.ts` | Modify — export new types and function |
| `tests/subagent-tools.test.ts` | Create — test spawn, delegate, callbacks, depth limit |
