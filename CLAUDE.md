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

simse is a modular pipeline framework for orchestrating multi-step AI workflows. It connects to AI backends via **ACP** (Agent Communication Protocol), exposes tools via **MCP** (Model Context Protocol), and provides a file-backed **vector memory** store with compression, indexing, deduplication, recommendation, and summarization.

### Module Layout

```
src/
  lib.ts                    # Barrel exports (public API surface)
  logger.ts                 # Structured logger with child loggers
  errors/                   # Error hierarchy split by domain
    base.ts                 # SimseError interface, createSimseError, toError, wrapError
    config.ts               # Config error factories + guards
    provider.ts             # Provider error factories + guards
    chain.ts                # Chain error factories + guards
    template.ts             # Template error factories + guards
    mcp.ts                  # MCP error factories + guards
    memory.ts               # Memory/Embedding/VectorStore error factories + guards
    index.ts                # Barrel re-export
  config/
    schema.ts               # Hand-rolled config validation (no Zod)
    settings.ts             # AppConfig type + defineConfig()
  ai/
    acp/
      acp-client.ts         # ACP client: generate(), generateStream(), chat(), embed()
      acp-http.ts           # HTTP helpers: fetchWithTimeout, httpGet, httpPost
      acp-results.ts        # Response parsing: extractGenerateResult, extractEmbeddings
      acp-stream.ts         # SSE delta extraction: extractStreamDelta
      types.ts              # ACP types (servers, agents, responses)
    mcp/
      mcp-client.ts         # MCP client (connects to external MCP servers)
      mcp-server.ts         # MCP server (exposes simse tools: generate, run-chain, list-agents)
      types.ts              # MCP config types
    chain/
      chain.ts              # createChain factory, createChainFromDefinition, runNamedChain
      prompt-template.ts    # PromptTemplate interface + createPromptTemplate
      format.ts             # formatSearchResults helper
      types.ts              # Provider, ChainStepConfig, StepResult, ChainCallbacks
      index.ts              # Barrel re-export
    memory/
      memory.ts             # MemoryManager: add/search/recommend/summarize/findDuplicates
      vector-store.ts       # VectorStore: file-backed storage with indexes + compression
      cosine.ts             # Pure cosineSimilarity function (clamped to [-1, 1])
      vector-persistence.ts # IndexEntry (v1) + CompressedIndexEntry (v2) + guards
      text-search.ts        # Text search: exact, substring, fuzzy, regex, token modes
      compression.ts        # Float32 base64 embedding encode/decode, gzip wrappers
      indexing.ts            # TopicIndex, MetadataIndex, MagnitudeCache factories
      deduplication.ts       # checkDuplicate, findDuplicateGroups (cosine-based)
      recommendation.ts      # WeightProfile, recency/frequency scoring, computeRecommendationScore
      types.ts              # All memory/search/deduplication/recommendation/summarization types
  utils/
    retry.ts                # Retry with exponential backoff + jitter, AbortSignal support
```

### Key Patterns

- **Factory functions over classes**: Every module exports a `createXxx()` factory returning a readonly interface. No classes in the codebase.
- **Immutable returns**: Factory functions use `Object.freeze()` on returned objects.
- **Error hierarchy**: `createSimseError` is the base; specialized factories (`createProviderError`, `createConfigError`, `createMemoryError`, etc.) add typed `code` fields. Type guards use duck-typing on `code`.
- **`toError(unknown)`**: Always wrap catch-block errors with `toError()` from `errors/index.js` before accessing `.message`.
- **ESM-only**: All imports use `.js` extensions (`import { foo } from './bar.js'`). The `verbatimModuleSyntax` tsconfig flag is enabled — use `import type` for type-only imports.
- **Write-lock serialization**: `vector-store.ts` uses a promise-chain (`writeLock`) to serialize concurrent mutations (add, delete, save). Never bypass the write lock for mutating operations.
- **In-flight promise deduplication**: `load()`, MCP `start()`, and MCP `connect()` use a stored promise to deduplicate concurrent callers. The pattern is: check for existing promise → create if missing → clear in `.finally()`.
- **Crash-safe persistence**: The vector store writes `.md` content files before the index file, so the index never references non-existent files.
- **Compressed v2 format**: On-disk embeddings use Float32 base64 (not JSON arrays). The index file is gzipped. Loading auto-detects v1 (plain JSON array) vs v2 (gzipped `{ version: 2, entries }`).

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
