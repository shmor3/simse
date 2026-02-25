# simse

A modular pipeline framework for orchestrating multi-step AI workflows using ACP, MCP, and vector memory.

## Features

- **ACP Client** — Connect to AI backends via the [Agent Client Protocol](https://agentclientprotocol.com) over JSON-RPC 2.0 / NDJSON stdio. Streaming, sessions, permissions, mode/model switching, embeddings.
- **MCP Client & Server** — Expose and consume tools, resources, and prompts via the [Model Context Protocol](https://modelcontextprotocol.io). Retry, logging, completions, roots, resource templates.
- **Agentic Loop** — Multi-turn tool-use loop that streams from ACP, parses tool calls, executes them, and repeats until completion. Auto-compaction, stream retry, tool retry.
- **Subagents** — Spawn nested agent loops or delegate single-shot tasks, with depth-limited recursion and lifecycle callbacks.
- **Chains** — Composable multi-step prompt pipelines with templates, input mapping, and callbacks.
- **Vector Memory** — File-backed vector store with cosine similarity search, topic/metadata indexing, deduplication, recommendation scoring, compression, and summarization.
- **Virtual Filesystem** — In-memory filesystem with history, diffing, snapshots, validation, and optional disk persistence.
- **Task List** — Dependency-aware task tracking for agentic workflows.
- **Resilience** — Circuit breaker, health monitor, timeout utility, and retry with exponential backoff and jitter.
- **Structured Logging** — Leveled logger with transports and child loggers.
- **Typed Error Hierarchy** — Domain-specific error factories with duck-typed guards.

## Requirements

- [Bun](https://bun.sh) >= 1.0

## Install

```bash
bun add simse
```

## Quick Start

```ts
import {
  createACPClient,
  createAgenticLoop,
  createConversation,
  createToolRegistry,
  registerSubagentTools,
} from 'simse';

const acpClient = createACPClient({
  servers: [{ name: 'my-agent', command: 'my-agent-binary' }],
});
await acpClient.initialize();

const registry = createToolRegistry({});
const conversation = createConversation();

// Optionally enable subagent spawning
registerSubagentTools(registry, { acpClient, toolRegistry: registry });

const loop = createAgenticLoop({ acpClient, toolRegistry: registry, conversation });
const result = await loop.run('Hello, what can you do?');
console.log(result.finalText);
```

## Development

```bash
bun test               # Run tests
bun test --watch       # Watch mode
bun test --coverage    # Coverage report
bun run build          # Bundle to dist/
bun run typecheck      # tsc --noEmit (strict mode)
bun run lint           # Biome check
bun run lint:fix       # Biome auto-fix
```

## Architecture

```
src/
  lib.ts                 # Public API barrel exports
  logger.ts              # Structured logger
  errors/                # Error hierarchy by domain
  config/                # Typed config validation + defineConfig()
  ai/
    acp/                 # ACP client, connection, streaming
    mcp/                 # MCP server + client
    chain/               # Chain pipelines + prompt templates
    loop/                # Agentic loop + types
    agent/               # Agent executor
    conversation/        # Conversation buffer
    memory/              # Vector store, text search, compression,
                         # indexing, deduplication, recommendation
    tasks/               # Task list
    tools/               # Tool registry, builtins, subagent tools
    vfs/                 # Virtual filesystem
  utils/
    retry.ts             # Retry with exponential backoff
    circuit-breaker.ts   # Circuit breaker state machine
    health-monitor.ts    # Health tracking with windowed failure rates
    timeout.ts           # Timeout utility with AbortSignal support
```

### Key Patterns

- **Factory functions** — no classes; every module exports `createXxx()` returning a frozen readonly interface
- **Immutable returns** — all factories use `Object.freeze()`
- **ESM-only** — all imports use `.js` extensions
- **Typed config** — TypeScript handles structural checks; runtime validates semantics
- **Write-lock serialization** — vector store serializes concurrent mutations via a promise chain
- **Crash-safe persistence** — content files written before index
- **Compressed v2 format** — Float32 base64 embeddings, gzipped index

## License

[MIT](LICENSE)
