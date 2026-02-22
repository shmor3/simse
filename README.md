# SimSE

A modular pipeline framework for orchestrating multi-step AI workflows using the **Agent Communication Protocol (ACP)**, **MCP** (Model Context Protocol), and a built-in **vector memory** system. Built with **Bun** and **TypeScript**.

## Install

```bash
bun add simse
```

## Quick Start

```ts
import { createACPClient, createChain, createPromptTemplate, defineConfig } from "simse";

const config = defineConfig({
  acp: {
    servers: [{ name: "local", url: "http://localhost:8000" }],
    defaultAgent: "default",
  },
});

const client = createACPClient(config.acp);

// Single generation
const result = await client.generate("Explain quantum computing in one paragraph.");
console.log(result.content);

// Multi-step chain
const chain = createChain({ acpClient: client });

chain.addStep({
  name: "outline",
  template: createPromptTemplate("Create an outline for a blog post about {topic}."),
  systemPrompt: "You are a technical writer.",
});

chain.addStep({
  name: "draft",
  template: createPromptTemplate("Write an intro based on this outline:\n\n{previous_output}"),
  systemPrompt: "You are a professional blog writer.",
});

const results = await chain.run({ topic: "AI agents" });
console.log(results.at(-1)?.output);
```

## Configuration

Use `defineConfig()` to create a validated configuration object. Only `acp` with at least one server is required — everything else is optional with sensible defaults.

```ts
import { defineConfig } from "simse";

const config = defineConfig({
  acp: {
    servers: [{ name: "local", url: "http://localhost:8000" }],
    defaultAgent: "default",
  },
});
```

Each optional section can be added as needed:

```ts
const config = defineConfig({
  acp: {
    servers: [
      { name: "local", url: "http://localhost:8000", defaultAgent: "default" },
    ],
  },
  memory: {
    embeddingAgent: "embedding-agent",
    storePath: ".simse/memory",
  },
  mcp: {
    client: {
      servers: [
        { name: "file-tools", transport: "stdio", command: "node", args: ["mcp-servers/file-tools.js"] },
      ],
    },
  },
  chains: {
    summarize: {
      steps: [{ name: "summarize", template: "Summarize the following:\n\n{text}" }],
    },
  },
});
```

### Types

#### `SimseConfig`

The top-level input passed to `defineConfig()`.

| Field | Type | Required | Description |
|---|---|---|---|
| `acp` | `ACPConfigInput` | ✅ | ACP server configuration. At least one server is required. |
| `mcp` | `MCPConfigInput` | — | MCP client/server configuration. |
| `memory` | `MemoryConfigInput` | — | Memory / vector store configuration. |
| `chains` | `Record<string, ChainDefinition>` | — | Named chain definitions. |

#### `ACPServerInput`

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | `string` | ✅ | Friendly name for this server connection. |
| `url` | `string` | ✅ | Base URL of the ACP-compatible server. |
| `defaultAgent` | `string` | — | Default agent ID when none is specified per-step. |
| `apiKey` | `string` | — | API key (prefer environment variables instead). |
| `timeoutMs` | `number` | — | Request timeout in milliseconds (default: `30000`, min: `1000`, max: `600000`). |

#### `ChainDefinition`

| Field | Type | Required | Description |
|---|---|---|---|
| `description` | `string` | — | Human-readable description. |
| `agentId` | `string` | — | Default ACP agent for all steps in this chain. |
| `serverName` | `string` | — | Default ACP server for all steps in this chain. |
| `initialValues` | `Record<string, string>` | — | Default template variable values. |
| `steps` | `ChainStepDefinition[]` | ✅ | Ordered list of steps to execute. |

#### `ChainStepDefinition`

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | `string` | ✅ | Unique step name (used as a variable key for later steps). |
| `template` | `string` | ✅ | Prompt template with `{variable}` placeholders. |
| `provider` | `"acp" \| "mcp" \| "memory"` | — | Override the default provider for this step. |
| `agentId` | `string` | — | Override the ACP agent for this step. |
| `serverName` | `string` | — | Override the ACP server for this step. |
| `agentConfig` | `Record<string, unknown>` | — | Additional ACP run config passed to the agent. |
| `systemPrompt` | `string` | — | System prompt prepended to the request. |
| `inputMapping` | `Record<string, string>` | — | Map prior step outputs to template variables. |
| `mcpServerName` | `string` | — | (MCP only) Name of the MCP server to call. |
| `mcpToolName` | `string` | — | (MCP only) Name of the tool to invoke. |
| `mcpArguments` | `Record<string, string>` | — | (MCP only) Map tool arguments to chain values. |
| `storeToMemory` | `boolean` | — | Store this step's output in the vector memory. |
| `memoryMetadata` | `Record<string, string>` | — | Metadata to attach when storing to memory. |

#### `MemoryConfigInput`

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | `boolean` | `true` | Enable or disable the memory system. |
| `embeddingAgent` | `string` | — | ACP agent ID used for generating embeddings. |
| `storePath` | `string` | `".simse/memory"` | File path for the persistent vector store. |
| `similarityThreshold` | `number` | `0.7` | Minimum cosine similarity for search results (0–1). |
| `maxResults` | `number` | `5` | Maximum number of search results to return (1–100). |

### Template Variables

Templates use `{variableName}` syntax. Built-in variables:

- `{previous_output}` — The output of the immediately preceding step.
- `{stepName}` — Any prior step's output is available by its name.
- Any key from `initialValues` or override values passed to `chain.run()`.

## ACP Client

The `createACPClient` connects to one or more ACP-compatible servers for inference and embeddings.

```ts
import { createACPClient } from "simse";

const client = createACPClient({
  servers: [
    { name: "local", url: "http://localhost:8000", defaultAgent: "default" },
  ],
});

// Check availability
const available = await client.isAvailable("local");

// List agents
const agents = await client.listAgents("local");

// Generate text
const result = await client.generate("Hello, world!", {
  agentId: "my-agent",
  systemPrompt: "You are a helpful assistant.",
});
console.log(result.content);

// Stream text
for await (const event of client.generateStream("Tell me a story.")) {
  process.stdout.write(event.delta);
}

// Chat with message history
const chatResult = await client.chat([
  { role: "user", content: "What is 2 + 2?" },
  { role: "assistant", content: "4" },
  { role: "user", content: "And 3 + 3?" },
]);

// Generate embeddings
const embedResult = await client.embed(["Hello", "World"]);
console.log(embedResult.embeddings); // number[][]
```

## Chains

Build multi-step pipelines that pass outputs between steps.

```ts
import { createACPClient, createChain, createPromptTemplate, defineConfig } from "simse";

const config = defineConfig({
  acp: {
    servers: [{ name: "local", url: "http://localhost:8000" }],
  },
});
const client = createACPClient(config.acp);
const chain = createChain({ acpClient: client });

chain.addStep({
  name: "research",
  template: createPromptTemplate("List 3 key facts about {topic}."),
});

chain.addStep({
  name: "article",
  template: createPromptTemplate(
    "Write a short article using these facts:\n\n{research}"
  ),
  inputMapping: { research: "research" },
});

const results = await chain.run({ topic: "TypeScript" });

for (const step of results) {
  console.log(`[${step.stepName}] (${step.durationMs}ms)\n${step.output}\n`);
}
```

### Running Named Chains

If you define chains in your config, you can run them by name:

```ts
import { createACPClient, defineConfig, runNamedChain } from "simse";

const config = defineConfig({
  acp: {
    servers: [{ name: "local", url: "http://localhost:8000" }],
  },
  chains: {
    translate: {
      description: "Translate text to another language",
      initialValues: { language: "French" },
      steps: [
        {
          name: "translate",
          template: "Translate the following to {language}:\n\n{text}",
        },
      ],
    },
  },
});

const client = createACPClient(config.acp);
const results = await runNamedChain("translate", config, {
  acpClient: client,
  overrideValues: { text: "Good morning!", language: "Japanese" },
});
```

### Callbacks

Monitor chain execution with lifecycle callbacks:

```ts
chain.setCallbacks({
  onStepStart: ({ stepName, stepIndex, totalSteps, provider }) => {
    console.log(`Starting step ${stepIndex + 1}/${totalSteps}: ${stepName}`);
  },
  onStepComplete: (result) => {
    console.log(`Step "${result.stepName}" completed in ${result.durationMs}ms`);
  },
  onStepError: ({ stepName, error }) => {
    console.error(`Step "${stepName}" failed:`, error.message);
  },
  onChainComplete: (results) => {
    const totalMs = results.reduce((sum, r) => sum + r.durationMs, 0);
    console.log(`Chain completed in ${totalMs}ms`);
  },
  onChainError: ({ error, completedSteps }) => {
    console.error(`Chain failed after ${completedSteps.length} steps:`, error.message);
  },
});
```

## Memory & Vector Store

Embed text and search by similarity, keywords, metadata, or date range.

```ts
import { createACPClient, createMemoryManager, defineConfig } from "simse";

const config = defineConfig({
  acp: {
    servers: [{ name: "local", url: "http://localhost:8000" }],
  },
  memory: {
    embeddingAgent: "embedding-agent",
    storePath: ".my-app/memory",
  },
});

const client = createACPClient(config.acp);
const memory = createMemoryManager(client, config.memory);
await memory.initialize();

// Store entries
await memory.add("TypeScript is a typed superset of JavaScript.", {
  source: "docs",
  topic: "typescript",
});
await memory.add("Bun is a fast JavaScript runtime.", {
  source: "docs",
  topic: "bun",
});

// Vector similarity search
const results = await memory.search("What is TypeScript?");
for (const r of results) {
  console.log(`[${r.score.toFixed(3)}] ${r.entry.text}`);
}

// Text search (no embeddings needed)
const textResults = memory.textSearch({
  query: "typescript",
  mode: "fuzzy",
  threshold: 0.3,
});

// Metadata filtering
const filtered = memory.filterByMetadata([
  { key: "source", value: "docs", mode: "eq" },
]);

// Advanced combined search
const advanced = await memory.advancedSearch({
  text: { query: "runtime", mode: "fuzzy" },
  metadata: [{ key: "topic", value: "bun" }],
  maxResults: 5,
  rankBy: "average",
});

// Cleanup
await memory.dispose();
```

### VectorStore (Low-Level)

Use `createVectorStore` directly for full control without the embedding layer:

```ts
import { createVectorStore, cosineSimilarity } from "simse";

const store = createVectorStore(".my-app/vectors");
await store.load();

// Add entries with pre-computed embeddings
await store.add("Hello world", [0.1, 0.2, 0.3], { tag: "greeting" });

// Search by vector
const results = store.search([0.1, 0.2, 0.3], 5, 0.5);

// Text search
const textResults = store.textSearch({ query: "hello", mode: "substring" });

// Metadata filtering
const filtered = store.filterByMetadata([{ key: "tag", value: "greeting" }]);

// Date range filtering
const recent = store.filterByDateRange({ after: Date.now() - 86_400_000 });

await store.dispose();
```

## MCP

### MCP Client

Connect to external MCP tool servers:

```ts
import { createMCPClient } from "simse";
import type { MCPClientConfig } from "simse";

const mcpConfig: MCPClientConfig = {
  servers: [
    {
      name: "file-tools",
      transport: "stdio",
      command: "node",
      args: ["mcp-servers/file-tools.js"],
    },
    {
      name: "web-search",
      transport: "http",
      url: "http://localhost:3100/mcp",
    },
  ],
};

const mcp = createMCPClient(mcpConfig);
const failures = await mcp.connectAll();

// List available tools
const tools = await mcp.listTools("file-tools");

// Call a tool
const result = await mcp.callTool("file-tools", "read-file", {
  path: "./README.md",
});
console.log(result.content);

// Cleanup
await mcp.disconnectAll();
```

### MCP Server

Expose SimSE as an MCP server so other tools can call it:

```ts
import { createACPClient, createMCPServer, defineConfig } from "simse";

const config = defineConfig({
  acp: {
    servers: [{ name: "local", url: "http://localhost:8000" }],
  },
  mcp: {
    server: { enabled: true, name: "simse", version: "1.0.0" },
  },
});

const client = createACPClient(config.acp);
const server = createMCPServer(config.mcp.server, client);
await server.start();

// Exposes tools: generate, run-chain, list-agents
// Exposes resources: agents://acp
// Exposes prompts: single-prompt
```

## Logging

Structured levelled logger with pluggable transports.

```ts
import { createLogger, createConsoleTransport, createMemoryTransport } from "simse";

const logger = createLogger({
  context: "my-app",
  level: "debug",
  transports: [createConsoleTransport()],
});

logger.info("Application started", { version: "1.0.0" });
logger.debug("Processing request", { requestId: "abc-123" });
logger.warn("Rate limit approaching", { remaining: 5 });
logger.error("Request failed", new Error("Connection refused"));

// Child loggers inherit config with an appended context
const dbLogger = logger.child("db");
dbLogger.info("Connected"); // logs as [my-app:db]

// Memory transport for testing
const mem = createMemoryTransport();
const testLogger = createLogger({ transports: [mem] });
testLogger.info("test message");
console.log(mem.entries); // [{ level: "info", message: "test message", ... }]
```

## Error Handling

All errors use `SimseError` with machine-readable codes and structured metadata. Use `create*` factory functions to create errors and `is*` type guards to check them.

```ts
import { defineConfig, isConfigValidationError, isSimseError } from "simse";

try {
  const config = defineConfig({
    acp: { servers: [] }, // invalid — needs at least one server
  });
} catch (err) {
  if (isConfigValidationError(err)) {
    for (const issue of err.issues) {
      console.error(`${issue.path}: ${issue.message}`);
    }
  }

  if (isSimseError(err)) {
    console.error(err.code);       // "CONFIG_VALIDATION"
    console.error(err.statusCode); // 400
    console.error(err.toJSON());   // structured representation
  }
}
```

### Error Hierarchy

```
SimseError
├── ConfigError
│   └── ConfigValidationError
├── ProviderError
│   ├── ProviderUnavailableError
│   ├── ProviderTimeoutError
│   └── ProviderGenerationError
├── ChainError
│   ├── ChainStepError
│   └── ChainNotFoundError
├── TemplateError
│   └── TemplateMissingVariablesError
├── MCPError
│   ├── MCPConnectionError
│   ├── MCPServerNotConnectedError
│   ├── MCPToolError
│   └── MCPTransportConfigError
├── MemoryError
│   ├── EmbeddingError
│   ├── VectorStoreCorruptionError
│   └── VectorStoreIOError
└── RetryExhaustedError
```

## Retry

Automatic exponential backoff with jitter for transient failures.

```ts
import { retry, isTransientError } from "simse";

const result = await retry(
  async (attempt) => {
    console.log(`Attempt ${attempt}...`);
    return await fetchFromUnreliableAPI();
  },
  {
    maxAttempts: 4,
    baseDelayMs: 1000,
    maxDelayMs: 15_000,
    backoffMultiplier: 2,
    jitterFactor: 0.25,
    shouldRetry: (err) => isTransientError(err),
    onRetry: (err, attempt, delayMs) => {
      console.log(`Retrying in ${delayMs}ms (attempt ${attempt})...`);
    },
  }
);
```

## Architecture

```
src/
├── lib.ts                    # Public API re-exports
├── errors/                   # Error hierarchy split by domain
│   ├── base.ts               # SimseError, createSimseError, toError, wrapError
│   ├── config.ts             # Config errors
│   ├── provider.ts           # Provider errors
│   ├── chain.ts              # Chain errors
│   ├── template.ts           # Template errors
│   ├── mcp.ts                # MCP errors
│   ├── memory.ts             # Memory/VectorStore errors
│   └── index.ts              # Barrel re-export
├── logger.ts                 # Structured logger with transports
├── config/
│   ├── settings.ts           # defineConfig() with validation
│   └── schema.ts             # Config validation schemas
├── utils/
│   └── retry.ts              # Retry with exponential backoff + jitter
└── ai/
    ├── acp/
    │   ├── acp-client.ts     # ACP client (Agent Communication Protocol)
    │   ├── acp-http.ts       # HTTP helpers (fetchWithTimeout, httpGet, httpPost)
    │   ├── acp-results.ts    # Response parsing helpers
    │   ├── acp-stream.ts     # SSE stream parsing
    │   └── types.ts          # ACP type definitions
    ├── chain/
    │   ├── chain.ts          # createChain, createChainFromDefinition, runNamedChain
    │   ├── prompt-template.ts # PromptTemplate + createPromptTemplate
    │   ├── format.ts         # formatSearchResults
    │   ├── types.ts          # Chain type definitions
    │   └── index.ts          # Barrel re-export
    ├── mcp/
    │   ├── mcp-client.ts     # MCP client (connects to external servers)
    │   ├── mcp-server.ts     # MCP server (exposes SimSE as a tool server)
    │   └── types.ts          # MCP type definitions
    └── memory/
        ├── memory.ts         # MemoryManager (embed + store + search)
        ├── vector-store.ts   # Persistent vector store with cosine similarity
        ├── cosine.ts         # Pure cosineSimilarity function
        ├── vector-persistence.ts # IndexEntry type + validation
        ├── text-search.ts    # Fuzzy, substring, exact, regex, token overlap
        └── types.ts          # Memory/embedding type definitions
```

## Development

```bash
bun install              # Install dependencies
bun run build            # Bundle with Bun → dist/
bun test                 # Run all tests (526 tests)
bun test --watch         # Run tests in watch mode
bun test --coverage      # Run tests with coverage
bun run typecheck        # Type-check without emitting
bun run lint             # Lint with Biome
bun run lint:fix         # Lint and auto-fix
```

## License

MIT
