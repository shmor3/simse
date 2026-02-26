# Memory Middleware, Prompt Injection & Optimization Design

## Overview

Transform simse's memory system from a CLI-layer concern into a reusable library-level middleware that automatically enriches every agentic loop turn with relevant context, stores responses, and manages summarization. Includes RAM/disk optimization, structured prompt injection, dedicated summarization ACP, and pure functional programming enforcement.

## 1. Memory Middleware

### Interface

```typescript
export interface MemoryMiddleware {
  readonly enrichSystemPrompt: (
    context: MiddlewareContext,
  ) => Promise<string>;
  readonly afterResponse: (
    userInput: string,
    response: string,
  ) => Promise<void>;
}

export interface MiddlewareContext {
  readonly userInput: string;
  readonly currentSystemPrompt: string;
  readonly conversationHistory: string;
  readonly turn: number;
}

export interface MemoryMiddlewareOptions {
  readonly maxResults?: number;       // default: 5
  readonly minScore?: number;         // default: 0.5
  readonly format?: PromptInjectionOptions;
  readonly storeTopic?: string;       // default: 'conversation'
  readonly storeResponses?: boolean;  // default: true
  readonly autoSummarizeThreshold?: number;
}
```

### Factory

```typescript
export function createMemoryMiddleware(
  memoryManager: MemoryManager,
  options?: MemoryMiddlewareOptions,
): MemoryMiddleware;
```

### Location

`src/ai/memory/middleware.ts` — part of the simse package, not CLI.

### Behavior

- **Per-turn**: Before each turn, search memory using the user's latest input. Format results as structured context and append to system prompt.
- **After final response**: Store the Q&A pair in memory, trigger auto-summarization if threshold exceeded.
- **Graceful degradation**: If memory search fails, log warning and continue without context.

### Integration with Agentic Loop

```typescript
export interface AgenticLoopOptions {
  // ... existing fields ...
  readonly memoryMiddleware?: MemoryMiddleware;
}
```

The loop calls `memoryMiddleware.enrichSystemPrompt()` before each turn's ACP call, and `memoryMiddleware.afterResponse()` after the loop completes.

## 2. Structured Prompt Injection

### Interface

```typescript
export interface PromptInjectionOptions {
  readonly maxResults?: number;
  readonly minScore?: number;
  readonly format?: 'structured' | 'natural';
  readonly tag?: string;        // default: 'memory-context'
  readonly maxChars?: number;   // default: 4000
}
```

### Format

Structured XML-like tags in the system prompt:

```
<memory-context>
<entry topic="rust/async" relevance="0.92" age="2h">
User prefers tokio over async-std for async Rust projects.
</entry>
</memory-context>
```

### Factory

```typescript
export function formatMemoryContext(
  results: readonly SearchResult[],
  options?: PromptInjectionOptions,
): string;
```

### Location

`src/ai/memory/prompt-injection.ts`

## 3. Summarization ACP Config

### Config File

New file: `summarize.json` in data directory.

```json
{
  "server": "summarize-llm",
  "command": "ollama",
  "args": ["run", "acp-bridge", "--model", "llama3.2"],
  "agent": "summarizer"
}
```

### Config Type

```typescript
export interface SummarizeFileConfig {
  readonly server: string;
  readonly command: string;
  readonly args?: readonly string[];
  readonly agent?: string;
  readonly env?: Readonly<Record<string, string>>;
}
```

### Integration

- On startup, if `summarize.json` exists, create a dedicated ACP client for summarization.
- The `TextGenerationProvider` adapter wraps this client.
- If not configured, summarization is disabled (no fallback to main ACP).

### First-Time Setup

Added as optional step after main ACP selection:

```
  Configure summarization? (uses a separate LLM for auto-summarizing notes)
    1) Same as above (reuse main ACP)
    2) Different provider
    3) Skip (no auto-summarization)
```

## 4. RAM/Disk Optimization

### Split Storage Model

| Layer | In RAM | On Disk |
|-------|--------|---------|
| Entry ID | yes | yes (index) |
| Embedding vector | yes | yes (base64) |
| Metadata | yes | yes (index) |
| Timestamp | yes | yes (index) |
| Text hash | yes | no |
| Full text | LRU cache | yes (.md files) |

### LRU Text Cache

```typescript
export interface TextCacheOptions {
  readonly maxEntries?: number;  // default: 500
  readonly maxBytes?: number;    // default: 5MB
}
```

### Changes to VectorEntry

```typescript
// RAM-resident entry (no full text)
export interface VectorEntrySlim {
  readonly id: string;
  readonly embedding: readonly number[];
  readonly metadata: Readonly<Record<string, string>>;
  readonly timestamp: number;
  readonly textHash: number;
  readonly textLength: number;
}
```

Search uses `VectorEntrySlim` (embeddings in RAM). Full `VectorEntry` (with text) is hydrated on-demand from the LRU cache or disk.

### Search Flow

1. Cosine similarity computed against `VectorEntrySlim.embedding` (all in RAM)
2. Top-K results identified
3. Full text hydrated from LRU cache or disk for the top-K only
4. Results returned with full `VectorEntry` objects

## 5. Pure Functional Programming Enforcement

### Rules

- No `class` keyword anywhere in `src/`
- All factories return `Object.freeze()`'d readonly interfaces
- All interface properties are `readonly`
- No mutation of input parameters
- `import type` for type-only imports (enforced by `verbatimModuleSyntax`)

### Audit Scope

- All new files created in this work
- All existing files modified in this work
- Spot-check of `src/ai/tools/host/` files from previous session

## 6. Package-Level Feature Availability

All features must be importable from `simse`:

```typescript
// Memory middleware
export { createMemoryMiddleware } from './ai/memory/middleware.js';
export type { MemoryMiddleware, MemoryMiddlewareOptions } from './ai/memory/middleware.js';

// Prompt injection
export { formatMemoryContext } from './ai/memory/prompt-injection.js';
export type { PromptInjectionOptions } from './ai/memory/prompt-injection.js';
```

The CLI (`simse-code/`) only calls these library APIs — no memory logic in CLI.

## Testing Strategy

- **Unit tests**: Each new module (middleware, prompt-injection, text-cache, summarize config)
- **Integration tests**: Memory middleware + agentic loop, summarization pipeline
- **E2E tests**: Full flow from user input through memory enrichment to response storage
- **Existing tests**: All 1346 must continue passing

## Non-Goals

- Real-time vector index (HNSW/FAISS) — acceptable at <10K entries
- Multi-user/multi-session memory isolation
- Streaming memory updates during tool execution
