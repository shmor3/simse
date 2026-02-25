# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
bun run build          # Bundle with Bun → dist/
bun run typecheck      # tsc --noEmit (strict mode)
bun run lint           # Biome check
bun run lint:fix       # Biome check --write
bun test               # bun test
bun test --watch       # bun test --watch
bun test --coverage    # bun test --coverage
```

## Architecture

simse is a modular pipeline framework for orchestrating multi-step AI workflows. It connects to AI backends via **ACP** (Agent Client Protocol), exposes tools via **MCP** (Model Context Protocol), and provides a file-backed **vector memory** store with compression, indexing, deduplication, recommendation, and summarization.

### Module Layout

```
src/
  lib.ts                    # Barrel exports (public API surface)
  logger.ts                 # Structured logger with child loggers
  errors/                   # Error hierarchy split by domain
    base.ts                 # SimseError interface, createSimseError, toError, wrapError
    config.ts               # Config error factories + guards
    provider.ts             # Provider error factories + guards (incl. HTTP errors)
    chain.ts                # Chain error factories + guards
    template.ts             # Template error factories + guards
    mcp.ts                  # MCP error factories + guards
    memory.ts               # Memory/Embedding/VectorStore error factories + guards
    loop.ts                 # Agentic loop error factories + guards
    resilience.ts           # CircuitBreaker/Timeout error factories + guards
    tasks.ts                # Task list error factories + guards
    tools.ts                # Tool registry error factories + guards
    vfs.ts                  # Virtual filesystem error factories + guards
    index.ts                # Barrel re-export
  config/
    schema.ts               # Typed config validation (semantic-only, no runtime type guards)
    settings.ts             # AppConfig type + defineConfig()
  ai/
    acp/
      acp-client.ts         # ACP client: generate(), generateStream(), chat(), embed()
                             # Session management: listSessions(), loadSession(), deleteSession()
                             # Mode/model switching: setSessionMode(), setSessionModel()
      acp-connection.ts      # JSON-RPC 2.0 over NDJSON stdio transport
                             # Permission handling with ACP option selection
                             # AbortSignal support for request cancellation
                             # Stderr routing to logger
      acp-results.ts        # Response parsing: extractContentText, extractTokenUsage
                             # Tool call extraction: extractToolCall, extractToolCallUpdate
      acp-adapters.ts       # EmbeddingProvider + TextGenerationProvider adapters for ACP
      local-embedder.ts     # In-process embedding via @huggingface/transformers
      tei-bridge.ts         # Text Embeddings Inference (TEI) HTTP bridge
      types.ts              # ACP types: sessions, content blocks, streaming, permissions,
                             # tool calls, sampling params, model/mode info
    mcp/
      mcp-client.ts         # MCP client: tools, resources, prompts, completions
                             # Logging: setLoggingLevel(), onLoggingMessage()
                             # List-changed: onToolsChanged(), onResourcesChanged()
                             # Roots: setRoots(), sendRootsListChanged()
                             # Resource templates: listResourceTemplates()
                             # Retry logic on tool calls and resource reads
      mcp-server.ts         # MCP server: generate, run-chain, list-agents,
                             # memory-search, memory-add, vfs-*, task-* tools
                             # List-changed notifications
                             # Logging support
      types.ts              # MCP types: tools, resources, prompts, logging,
                             # completions, roots, resource templates, annotations
    agent/
      agent-executor.ts     # Step execution dispatcher (acp/mcp/memory providers)
      types.ts              # AgentResult, AgentStepConfig, ParallelConfig, SwarmMerge
      index.ts              # Barrel re-export
    chain/
      chain.ts              # createChain factory, createChainFromDefinition, runNamedChain
      prompt-template.ts    # PromptTemplate interface + createPromptTemplate
      format.ts             # formatSearchResults helper
      types.ts              # Provider, ChainStepConfig, StepResult, ChainCallbacks
      index.ts              # Barrel re-export
    conversation/
      conversation.ts       # createConversation factory: message management,
                             # serialization, auto-compaction
      types.ts              # ConversationRole, ConversationMessage, ConversationOptions
      index.ts              # Barrel re-export
    loop/
      agentic-loop.ts       # createAgenticLoop: conversation → ACP stream → tool exec → repeat
                             # maxTurns, AbortSignal, auto-compaction, streaming retry
      types.ts              # AgenticLoopOptions, LoopTurn, AgenticLoopResult, LoopCallbacks
      index.ts              # Barrel re-export
    memory/
      memory.ts             # MemoryManager: add/search/recommend/summarize/findDuplicates
      vector-store.ts       # VectorStore: file-backed storage with indexes + compression
      cosine.ts             # Pure cosineSimilarity function (clamped to [-1, 1])
      vector-persistence.ts # IndexEntry / IndexFile types + validation guards
      text-search.ts        # Text search: exact, substring, fuzzy, regex, token modes
      compression.ts        # Float32 base64 embedding encode/decode, gzip wrappers
      indexing.ts           # TopicIndex, MetadataIndex, MagnitudeCache factories
      deduplication.ts      # checkDuplicate, findDuplicateGroups (cosine-based)
      recommendation.ts     # WeightProfile, recency/frequency scoring, computeRecommendationScore
      storage.ts            # StorageBackend interface (pluggable persistence)
      learning.ts           # Adaptive learning engine: query tracking, weight adaptation
      types.ts              # All memory/search/deduplication/recommendation/summarization types
    tasks/
      task-list.ts          # createTaskList factory: CRUD, dependencies, blocking
      types.ts              # TaskItem, TaskStatus, TaskList, TaskCreateInput, TaskUpdateInput
      index.ts              # Barrel re-export
    tools/
      tool-registry.ts      # createToolRegistry: register, discover, execute, parse
      builtin-tools.ts      # registerMemoryTools, registerVFSTools, registerTaskTools
      subagent-tools.ts     # registerSubagentTools: spawn sub-loops as tool calls
      types.ts              # ToolDefinition, ToolHandler, ToolRegistry, ToolCallRequest
      index.ts              # Barrel re-export
    vfs/
      vfs.ts                # createVirtualFS: in-memory filesystem sandbox
      vfs-disk.ts           # createVFSDisk: disk-backed VFS with snapshots
      validators.ts         # File content validators (JSON syntax, trailing whitespace, etc.)
      types.ts              # VirtualFS, VFSDirEntry, VFSReadResult, VFSWriteOptions
      index.ts              # Barrel re-export
  utils/
    retry.ts                # Retry with exponential backoff + jitter, AbortSignal support
    circuit-breaker.ts      # Circuit breaker pattern for fault tolerance
    health-monitor.ts       # Health monitoring with sliding window stats
    timeout.ts              # withTimeout utility for Promise-based timeouts
```

### Key Patterns

- **Factory functions over classes**: Every module exports a `createXxx()` factory returning a readonly interface. No classes in the codebase.
- **Immutable returns**: Factory functions use `Object.freeze()` on returned objects.
- **Error hierarchy**: `createSimseError` is the base; specialized factories (`createProviderError`, `createConfigError`, `createMemoryError`, etc.) add typed `code` fields. Type guards use duck-typing on `code`.
- **`toError(unknown)`**: Always wrap catch-block errors with `toError()` from `errors/index.js` before accessing `.message`.
- **ESM-only**: All imports use `.js` extensions (`import { foo } from './bar.js'`). The `verbatimModuleSyntax` tsconfig flag is enabled — use `import type` for type-only imports.
- **Write-lock serialization**: `vector-store.ts` uses a promise-chain (`writeLock`) to serialize concurrent mutations (add, delete, save). Never bypass the write lock for mutating operations.
- **In-flight promise deduplication**: `load()`, `initialize()`, MCP `start()`, and MCP `connect()` use a stored promise to deduplicate concurrent callers. The pattern is: check for existing promise → create if missing → clear in `.finally()`.
- **Crash-safe persistence**: The vector store writes `.md` content files before the index file, so the index never references non-existent files.
- **Compressed v2 format**: On-disk embeddings use Float32 base64 (not JSON arrays). The index file is gzipped. Loading auto-detects v1 (plain JSON array) vs v2 (gzipped `{ version: 2, entries }`).

### ACP Protocol

The ACP client implements the [Agent Client Protocol](https://agentclientprotocol.com) over JSON-RPC 2.0 / NDJSON stdio:

- **Protocol version**: 1
- **Field naming**: camelCase throughout (`sessionId`, `stopReason`, `agentInfo`, not snake_case)
- **Session lifecycle**: `session/new` → `session/prompt` → `session/update` notifications → response
- **Permission flow**: Agent sends `session/request_permission` with options array; client selects `allow_once`/`allow_always`/`reject_once`/`reject_always` via `{ outcome: { outcome: "selected", optionId } }`
- **Session modes**: Set via `session/set_config_option` (configOptionId: "mode", groupId: modeId)
- **Tool call lifecycle**: `tool_call` → `tool_call_update` (in_progress) → `tool_call_update` (completed) — all via `session/update` notifications
- **Sampling params**: `temperature`, `maxTokens`, `topP`, `topK`, `stopSequences` passed in prompt metadata
- **Agent fallback**: When no agentId is configured, falls back to server name

### MCP Protocol

The MCP implementation uses `@modelcontextprotocol/sdk`:

- **Client**: Connects to external MCP servers via stdio or HTTP transport
- **Server**: Exposes simse capabilities as MCP tools (generate, run-chain, list-agents, memory-search, memory-add)
- **Logging**: `setLoggingLevel()` + `onLoggingMessage()` for structured log collection
- **List-changed**: Notification handlers for dynamic tool/resource/prompt discovery
- **Completions**: `complete()` for argument autocomplete
- **Roots**: `setRoots()` + `sendRootsListChanged()` for workspace awareness
- **Resource templates**: `listResourceTemplates()` for URI pattern discovery
- **Retry**: Tool calls and resource reads use exponential backoff

### Memory System

The memory subsystem has five layers:

1. **Compression** (`compression.ts`): `encodeEmbedding`/`decodeEmbedding` (Float32↔base64, ~75% size reduction), `compressText`/`decompressText` (gzip wrappers).
2. **Indexing** (`indexing.ts`): `createTopicIndex` (auto-extracts topics from text or uses `metadata.topic`), `createMetadataIndex` (O(1) key-value lookups), `createMagnitudeCache` (skip recomputation during search).
3. **Deduplication** (`deduplication.ts`): `checkDuplicate` (single-entry cosine check), `findDuplicateGroups` (greedy clustering, O(N²)).
4. **Recommendation** (`recommendation.ts`): `computeRecommendationScore` combining vector similarity + exponential recency decay + logarithmic frequency scoring with configurable `WeightProfile`.
5. **Summarization** (`memory.ts`): `summarize()` requires a `TextGenerationProvider`, condenses multiple entries into one, optionally deletes originals.

### Formatting

- **Biome** (not ESLint/Prettier): tabs for indentation, single quotes, semicolons
- Organize imports is enabled (biome handles this)
