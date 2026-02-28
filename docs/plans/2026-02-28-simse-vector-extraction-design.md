# simse-vector Extraction Design

**Date:** 2026-02-28

## Goal

Extract the entire library/memory/vector system (26 files, ~7.8K LOC) from `src/ai/library/` into a standalone `simse-vector/` package. The package defines its own interfaces for external dependencies (Logger, TextGenerationProvider, EmbeddingProvider, errors) so it has zero imports from the main simse package.

## Current State

All library code lives in `src/ai/library/` inside the main simse package. It has ~7 dedicated test files. The library system is consumed by tools, MCP server, agentic loop, and chain modules within simse.

## Architecture

### Package structure

```
simse-vector/              <- NEW standalone package
  package.json
  tsconfig.json
  src/
    lib.ts                 <- barrel exports (public API)
    types.ts               <- all library types + provider interfaces
    errors.ts              <- self-contained error factories
    cosine.ts
    preservation.ts
    storage.ts
    stacks.ts
    stacks-persistence.ts
    stacks-serialize.ts
    stacks-search.ts
    stacks-recommend.ts
    text-search.ts
    text-cache.ts
    inverted-index.ts
    cataloging.ts
    query-dsl.ts
    deduplication.ts
    recommendation.ts
    patron-learning.ts
    topic-catalog.ts
    shelf.ts
    library.ts
    library-services.ts
    librarian.ts
    librarian-registry.ts
    librarian-definition.ts
    circulation-desk.ts
    prompt-injection.ts
  tests/
    library.test.ts
    stacks.test.ts
    library-types.test.ts
    library-services.test.ts
    library-errors.test.ts
    e2e-library-pipeline.test.ts
    hierarchical-library-integration.test.ts
```

### Interface-only dependency

simse-vector defines these interfaces in its own `types.ts`:

- **Logger** — `{ debug, info, warn, error, child }` (subset of simse logger)
- **TextGenerationProvider** — `{ generate(prompt, options): Promise<string> }`
- **EmbeddingProvider** — `{ embed(texts): Promise<number[][]> }`
- **EventBus** — `{ publish(event, data): void }` (optional)

The error layer gets its own `errors.ts` with `createLibraryError`, `createEmbeddingError`, `createStacksError`, etc. Self-contained, no simse imports.

### Dependency direction

```
simse-vector  <-  simse  (simse depends on simse-vector, never the reverse)
                    ^
               simse-code  (CLI depends on simse)
```

### simse integration

- `src/lib.ts` re-exports everything from `simse-vector` (preserving public API)
- `src/ai/acp/acp-adapters.ts` implements `EmbeddingProvider`/`TextGenerationProvider` using ACPClient
- Agentic loop, tools, MCP server import types from `simse-vector`
- External consumers see no change: `import { createLibrary } from 'simse'` still works

### Package config

- `simse-vector/package.json`: name `simse-vector`, Bun build, Biome lint, ESM-only, `.js` extensions
- Root `package.json` adds workspace reference
- `simse/package.json` adds `"simse-vector": "workspace:*"` dependency
- `picomatch` moves to simse-vector (used by librarian-definition.ts)

## API Impact

No breaking changes. simse re-exports everything from simse-vector, so existing consumers are unaffected.
