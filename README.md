# simse

A modular pipeline framework for orchestrating multi-step AI workflows. Connects to AI backends via **ACP** (Agent Communication Protocol), exposes tools via **MCP** (Model Context Protocol), and provides a file-backed **vector memory** store with compression, indexing, deduplication, recommendation, and summarization.

## Features

- **ACP Client** — generate, stream, chat, and embed via any ACP-compatible server
- **Chain Pipelines** — multi-step prompt chains with templates, input mapping, and callbacks
- **MCP Server/Client** — expose simse tools or connect to external MCP servers
- **Vector Memory** — file-backed vector store with cosine similarity search
- **Text Search** — exact, substring, fuzzy, regex, and token matching
- **Deduplication** — cosine-based duplicate detection and grouping
- **Recommendations** — weighted scoring combining vector similarity, recency decay, and access frequency
- **Summarization** — condense multiple memory entries into one via a text generation provider
- **Retry** — exponential backoff with jitter and AbortSignal support
- **Structured Logging** — leveled logger with transports and child loggers
- **Typed Error Hierarchy** — domain-specific error factories with duck-typed guards

## Requirements

- [Bun](https://bun.sh) >= 1.0

## Install

```bash
bun install
```

## Usage

Configuration is fully typed via TypeScript interfaces — `defineConfig()` accepts a `SimseConfig` object and validates semantic constraints (URL format, numeric ranges, cross-references) at runtime. Structural validation is handled at compile time by TypeScript.

```typescript
import {
  defineConfig,
  createACPClient,
  createChain,
  createPromptTemplate,
  createMemoryManager,
  createVectorStore,
} from 'simse';

// Configure — fully typed, no JSON parsing
const config = defineConfig({
  acp: {
    servers: [{ name: 'local', url: 'http://localhost:8000', defaultAgent: 'default' }],
  },
  memory: {
    embeddingAgent: 'default',
    storePath: './memory-data',
  },
  chains: {
    summarize: {
      steps: [{ name: 'summarize', template: 'Summarize:\n\n{text}' }],
    },
  },
});

// Generate text
const client = createACPClient(config.acp);
const result = await client.generate('Hello, world!');

// Build a chain programmatically
const chain = createChain({ acpClient: client });
chain.addStep({
  name: 'brainstorm',
  template: createPromptTemplate('List 3 facts about {topic}.'),
});
chain.addStep({
  name: 'article',
  template: createPromptTemplate('Write a paragraph from these facts:\n\n{brainstorm}'),
  inputMapping: { brainstorm: 'brainstorm' },
});
const results = await chain.run({ topic: 'Bun runtime' });

// Vector memory (bring your own StorageBackend)
const store = createVectorStore({ storage: myStorageBackend, autoSave: true });
await store.load();
await store.add('TypeScript is a typed superset of JavaScript', [0.9, 0.1, 0.0]);
const matches = store.search([0.85, 0.15, 0.0], 5, 0.5);

// Memory manager (auto-embeds text)
const memory = createMemoryManager(embeddingProvider, {
  enabled: true,
  embeddingAgent: 'default',
  similarityThreshold: 0.7,
  maxResults: 10,
}, { storage: myStorageBackend });
await memory.initialize();
await memory.add('Some important information', { category: 'notes' });
const searchResults = await memory.search('important');
```

See the [`example/`](example/) directory for complete walkthroughs: [`app.ts`](example/app.ts) (full knowledge base app), [`config.ts`](example/config.ts), [`agents.ts`](example/agents.ts), [`tools.ts`](example/tools.ts).

## Scripts

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
    acp/                 # ACP client, HTTP helpers, streaming
    mcp/                 # MCP server + client
    chain/               # Chain pipelines + prompt templates
    memory/              # Vector store, text search, compression,
                         # indexing, deduplication, recommendation
  utils/
    retry.ts             # Retry with exponential backoff
```

### Key Patterns

- **Factory functions** — no classes; every module exports `createXxx()` returning a frozen readonly interface
- **Immutable returns** — all factories use `Object.freeze()`
- **ESM-only** — all imports use `.js` extensions
- **Typed config validation** — validators accept typed interfaces, not `unknown`; TypeScript handles structural checks, runtime validates only semantics (URL format, ranges, cross-references)
- **Zero external runtime deps** for core logic (only `@modelcontextprotocol/sdk` for MCP)
- **Write-lock serialization** — vector store serializes concurrent mutations via a promise chain
- **Crash-safe persistence** — content files written before index; atomic writes via tmp+rename
- **Compressed v2 format** — Float32 base64 embeddings, gzipped index

## License

MIT
