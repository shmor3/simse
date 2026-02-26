# Loop/Stream UX Hardening Design

## Context

simse's agentic loop and streaming APIs lack the instrumentation and control surfaces needed for production agent UIs. Token usage is discarded after streaming, tools have no timeouts, permissions can't filter by category, and there's no way to cancel a stream mid-flight. This design closes 7 specific gaps.

## Part 1: Token Usage Accumulator

### Problem
`AgenticLoopResult` has `totalDurationMs` but no token usage. The data flows through `ACPStreamComplete.usage` but is discarded.

### Changes
- `LoopTurn`: add `readonly usage?: ACPTokenUsage`
- `AgenticLoopResult`: add `readonly totalUsage?: ACPTokenUsage`
- `agentic-loop.ts`: capture usage from stream complete chunk, accumulate across turns

### Files
- Modify: `src/ai/loop/types.ts`
- Modify: `src/ai/loop/agentic-loop.ts`
- Modify: `tests/agentic-loop.test.ts`

---

## Part 2: Tool Execution Timeout

### Problem
`toolRegistry.execute()` has no timeout. A hung tool blocks the loop forever.

### Changes
- `ToolDefinition`: add `readonly timeoutMs?: number`
- `ToolRegistryOptions`: add `readonly defaultToolTimeoutMs?: number`
- `execute()`: wrap handler with `withTimeout()` from `utils/timeout.ts`
- Per-tool timeout overrides global default; returns error result on timeout

### Files
- Modify: `src/ai/tools/types.ts`
- Modify: `src/ai/tools/tool-registry.ts`
- Create: `tests/tool-timeout.test.ts`

---

## Part 3: Tool Execution Metrics

### Problem
No visibility into which tools are slow, error-prone, or heavily used.

### Changes
- New `ToolMetrics` interface: `callCount`, `errorCount`, `totalDurationMs`, `avgDurationMs`, `lastCalledAt`
- `ToolRegistry`: add `readonly getToolMetrics: (name?: string) => ToolMetrics | readonly ToolMetrics[]`
- Internal Map updated in `execute()`

### Files
- Modify: `src/ai/tools/types.ts`
- Modify: `src/ai/tools/tool-registry.ts`
- Modify: `src/lib.ts` (export `ToolMetrics`)
- Create: `tests/tool-metrics.test.ts`

---

## Part 4: Streaming Cancellation via AbortSignal

### Problem
`generateStream()` accepts no `AbortSignal`. The stream runs to completion or timeout once started.

### Changes
- `ACPStreamOptions`: add `readonly signal?: AbortSignal`
- `generateStream()`: check `signal.aborted` in the chunk consumption loop; break and return early
- No connection-level cancellation (ACP has no cancel RPC) — just stops yielding chunks

### Files
- Modify: `src/ai/acp/acp-client.ts`

---

## Part 5: Category-Based Permission Filtering

### Problem
`ToolPermissionResolver.check()` only sees tool name and args. Can't whitelist "all read tools" or block destructive tools by category.

### Changes
- New `ToolPermissionContext` interface extends `ToolCallRequest` with `definition?: ToolDefinition`
- `ToolPermissionResolver.check()` signature: `(request: ToolCallRequest, definition?: ToolDefinition) => Promise<boolean>`
- `execute()` passes the definition alongside the request
- `ToolPermissionRule`: add optional `category?: ToolCategory | ToolCategory[]` and `annotations?: Partial<ToolAnnotations>` matchers
- `createToolPermissionResolver()` evaluates category/annotation rules before name rules

### Files
- Modify: `src/ai/tools/types.ts`
- Modify: `src/ai/tools/tool-registry.ts`
- Modify: `src/ai/tools/permissions.ts`
- Modify: `tests/tool-permissions.test.ts`

---

## Part 6: Turn-Level Usage Callback

### Problem
UIs want to show per-turn costs and running totals, but `onTurnComplete` has no usage info.

### Changes
- `LoopCallbacks`: add `readonly onUsageUpdate?: (accumulated: ACPTokenUsage) => void`
- Fired after each turn with running total (depends on Part 1 accumulator)

### Files
- Modify: `src/ai/loop/types.ts`
- Modify: `src/ai/loop/agentic-loop.ts`
- Modify: `tests/agentic-loop.test.ts`

---

## Part 7: Context Window Usage Percentage

### Problem
Consumers need to know how full the context window is for status bars and compaction decisions.

### Changes
- `ConversationOptions`: add `readonly contextWindowTokens?: number`
- `Conversation`: add `readonly contextUsagePercent: number` getter
- Returns `Math.min(100, Math.round((estimatedTokens / contextWindowTokens) * 100))`, or `0` if not configured

### Files
- Modify: `src/ai/conversation/types.ts`
- Modify: `src/ai/conversation/conversation.ts`
- Modify: `tests/conversation-replace.test.ts`

---

## Implementation Order

1. Part 7 (context usage %) — smallest, no dependencies
2. Part 2 (tool timeouts) — safety-critical, uses existing `withTimeout`
3. Part 3 (tool metrics) — extends Part 2's execute path
4. Part 4 (stream cancellation) — isolated to ACP client
5. Part 5 (category permissions) — extends tool registry + permissions
6. Part 1 (token accumulator) — requires understanding stream flow
7. Part 6 (usage callback) — depends on Part 1

## Verification

- `bun x tsc --noEmit` — typecheck
- `bun test` — all tests pass
- `bun run lint` — clean
