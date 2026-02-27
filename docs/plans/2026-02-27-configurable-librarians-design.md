# Configurable Librarians Design

**Goal:** Make librarians configurable via JSON definition files, support multiple specialized librarians that bid for ownership of memory actions, and enable the default librarian to spawn new specialists when topic areas become too complex.

**Architecture:** A new `LibrarianRegistry` manages multiple named librarians, each with their own JSON definition, ACP connection, and topic scope. When multiple librarians match a topic, they bid for ownership by evaluating content against their purpose with full library access. The default librarian arbitrates ties. The CirculationDesk routes jobs through the registry.

**Tech Stack:** TypeScript, Bun, ACP JSON-RPC 2.0, JSON definition files

---

## 1. Librarian Definition Schema

Each librarian is defined by a JSON file in `<library-dir>/librarians/<name>.json`:

```json
{
  "name": "code-patterns",
  "description": "Manages code pattern and architecture memories",
  "purpose": "I specialize in identifying reusable code patterns, architectural decisions, and technical debt observations. I understand software design principles and can organize memories by pattern type, language, and architectural layer.",
  "topics": ["code/*", "architecture/*"],
  "permissions": {
    "add": true,
    "delete": true,
    "reorganize": true
  },
  "thresholds": {
    "topicComplexity": 50,
    "escalateAt": 100
  },
  "acp": {
    "command": "simse-engine",
    "args": ["--mode", "librarian"],
    "agentId": "code-librarian"
  }
}
```

TypeScript interface:

```typescript
interface LibrarianDefinition {
  readonly name: string;
  readonly description: string;
  readonly purpose: string;
  readonly topics: readonly string[];
  readonly permissions: {
    readonly add: boolean;
    readonly delete: boolean;
    readonly reorganize: boolean;
  };
  readonly thresholds: {
    readonly topicComplexity: number;
    readonly escalateAt: number;
  };
  readonly acp?: {
    readonly command: string;
    readonly args?: readonly string[];
    readonly agentId?: string;
  };
}
```

Key fields:

- **`name`**: Unique identifier, matches the filename (without `.json`)
- **`description`**: Short human-readable summary
- **`purpose`**: Rich expertise statement the librarian uses during bidding to argue for ownership. This is the core of the librarian's identity.
- **`topics`**: Glob patterns for topic matching. `["*"]` for the default librarian. Multiple librarians can match the same topic — this is intended behavior.
- **`permissions`**: What operations this librarian can perform (add volumes, delete volumes, reorganize topics)
- **`thresholds.topicComplexity`**: Volume count in a subtree that triggers the hybrid spawn check
- **`thresholds.escalateAt`**: Maximum volumes before forced optimization
- **`acp`**: Optional per-librarian ACP connection config. If omitted, uses the shared default provider.

The **default librarian** always exists even without a JSON file. It's created programmatically with `topics: ["*"]`, a general-purpose `purpose`, and no custom ACP (uses the shared provider). It has two special roles:
1. Catches any content that no specialist claims
2. Acts as arbiter when specialists can't resolve ownership

## 2. Bidding & Arbitration

When a memory action arrives and multiple librarians match the topic, ownership is resolved dynamically through bidding — not static priority.

### Bidding Flow

1. **Topic matching**: The registry finds all librarians whose `topics` globs match the content's topic
2. **Context gathering**: The registry searches the library for existing volumes in the topic area, giving each librarian context about what's already stored
3. **Bid generation**: Each matching librarian evaluates the content against its `purpose` with full library access (search, read). It produces a `LibrarianBid` — an argument for why it should handle this content, plus a confidence score.
4. **Self-resolution**: If one librarian's confidence is significantly higher (>0.3 gap above all others), it wins without arbitration
5. **Arbitration**: If bids are close, the **default librarian** acts as arbiter — it reads all bids and the content, then selects the winner based on argument quality
6. **Execution**: The winning librarian performs the memory actions using its own provider

### Interfaces

```typescript
interface LibrarianBid {
  readonly librarianName: string;
  readonly argument: string;
  readonly confidence: number;
}

interface ArbitrationResult {
  readonly winner: string;
  readonly reason: string;
  readonly bids: readonly LibrarianBid[];
}
```

### Library Access During Bidding

Librarians have full library access during bidding so they can make informed arguments. The `bid()` method on the Librarian interface receives a library reference:

```typescript
interface Librarian {
  // ... existing methods (extract, summarize, classifyTopic, reorganize, optimize)
  readonly bid: (
    content: string,
    topic: string,
    library: Library,
  ) => Promise<LibrarianBid>;
}
```

The registry pre-fetches relevant context before calling `bid()`:
- Existing volumes in the topic area
- Related topics the librarian manages
- Volume counts per subtopic

This context is injected into the bid prompt so each librarian sees what's already stored and can argue, e.g., "I already manage 15 volumes about React patterns in this subtree, and this new memory fits my existing organization structure."

### Bid Prompt (sent to each candidate librarian)

```
You are {name}, a specialized librarian.
Your purpose: {purpose}

You are bidding to manage this new content:
Topic: {topic}
Content: {text}

Here is what you currently manage in related topics:
{search results from library filtered to this librarian's topic patterns}

Existing volumes in this topic area: {count}
Related topics you manage: {list}

Evaluate whether this content falls within your expertise.
Then argue why you should manage it.

Return JSON: {"argument": "your informed case", "confidence": 0.0-1.0}
```

### Arbitration Prompt (sent to default librarian when bids are close)

```
You are the head librarian. Multiple specialist librarians want to handle this content.
Review their arguments and choose the best fit.

Content: {text}
Topic: {topic}

Bids:
{formatted bids with arguments and confidence scores}

Return JSON: {"winner": "librarian-name", "reason": "why this librarian is the best fit"}
```

## 3. Librarian Registry

The `LibrarianRegistry` is the central coordinator. It manages librarian lifecycle, routes work via bidding, and handles specialist spawning.

```typescript
interface ManagedLibrarian {
  readonly definition: LibrarianDefinition;
  readonly librarian: Librarian;
  readonly provider: TextGenerationProvider;
  readonly connection?: ACPConnection;
}

interface LibrarianRegistry {
  // Lifecycle
  readonly initialize: () => Promise<void>;
  readonly dispose: () => Promise<void>;

  // Librarian management
  readonly register: (definition: LibrarianDefinition) => Promise<ManagedLibrarian>;
  readonly unregister: (name: string) => Promise<void>;
  readonly get: (name: string) => ManagedLibrarian | undefined;
  readonly list: () => readonly ManagedLibrarian[];
  readonly defaultLibrarian: ManagedLibrarian;

  // Routing (the bidding system)
  readonly resolveLibrarian: (
    topic: string,
    content: string,
  ) => Promise<ArbitrationResult>;

  // Spawning
  readonly spawnSpecialist: (
    topic: string,
    volumes: readonly Volume[],
  ) => Promise<ManagedLibrarian>;
}
```

### Factory

```typescript
interface LibrarianRegistryOptions {
  readonly libraryDir: string;
  readonly library: Library;
  readonly defaultProvider: TextGenerationProvider;
  readonly logger?: Logger;
  readonly eventBus?: EventBus;
  readonly selfResolutionGap?: number; // default 0.3
}

function createLibrarianRegistry(
  options: LibrarianRegistryOptions,
): LibrarianRegistry;
```

### Key Behaviors

- **`initialize()`**: Loads all JSON definitions from `<library-dir>/librarians/`, creates ACP connections for those that specify one, creates the default librarian if no `default.json` exists. Uses in-flight promise deduplication (existing pattern).
- **`register()`**: Hot-loads a new librarian definition — validates the JSON, creates the ACP connection if specified, saves the JSON file to disk, adds to the registry.
- **`unregister()`**: Disposes the librarian's ACP connection, removes the JSON file, removes from registry.
- **`resolveLibrarian()`**: Implements the full bidding/arbitration flow from Section 2. Returns the winning librarian and all bids for transparency.
- **`spawnSpecialist()`**: The hybrid spawn flow (see Section 4).

## 4. Specialist Spawning

The default librarian can spawn new specialist librarians when a topic area becomes too complex. This uses a hybrid trigger: volume count threshold + LLM confirmation.

### Spawn Flow

1. After extraction, the CirculationDesk checks if any topic subtree exceeds `thresholds.topicComplexity` (default 50 volumes) AND has more than 3 levels of nesting or 5+ child topics
2. If the heuristic triggers, the registry calls the powerful model (via `generateWithModel`) to confirm:

```
You are the head librarian assessing whether a topic area needs a specialist.

Topic: {topic}
Volume count: {count}
Subtopic depth: {depth}
Child topics: {list}

Sample volumes:
{5 representative volumes from the topic}

Should a specialist librarian be created for this area?
Consider: Is the content diverse enough to benefit from specialized organization?
Is there a coherent theme that a specialist could focus on?

Return JSON: {"shouldSpawn": true/false, "reason": "why"}
```

3. If confirmed, the powerful model generates a `LibrarianDefinition`:

```
Create a specialist librarian definition for managing the "{topic}" area of the library.

Existing volumes show these themes:
{summarized themes from volumes}

Generate a JSON librarian definition with:
- A descriptive name (kebab-case)
- A purpose statement explaining the specialist's expertise
- Topic glob patterns covering this area
- Appropriate permissions and thresholds

Return ONLY valid JSON matching the LibrarianDefinition schema.
```

4. The registry validates the generated definition, saves it to disk, creates the ACP connection, and the new specialist is live — it immediately starts winning bids in its domain.

## 5. CirculationDesk Integration

The CirculationDesk accepts a `LibrarianRegistry` instead of a single `Librarian`.

### Updated Options

```typescript
interface CirculationDeskOptions {
  readonly registry: LibrarianRegistry;
  readonly addVolume: (text: string, metadata?: Record<string, string>) => Promise<string>;
  readonly checkDuplicate: (text: string) => Promise<DuplicateCheckResult>;
  readonly getVolumesForTopic: (topic: string) => Volume[];
  readonly deleteVolume?: (id: string) => Promise<void>;
  readonly getTotalVolumeCount?: () => number;
  readonly getAllTopics?: () => string[];
  readonly thresholds?: CirculationDeskThresholds;
  readonly catalog?: TopicCatalog;
}
```

### Updated Thresholds

```typescript
interface CirculationDeskThresholds {
  readonly compendium?: { ... };        // existing
  readonly reorganization?: { ... };    // existing
  readonly optimization?: { ... };      // existing
  readonly spawning?: {
    readonly complexityThreshold?: number;  // default 50 volumes in subtree
    readonly depthThreshold?: number;       // default 3 levels of nesting
    readonly childTopicThreshold?: number;  // default 5 child topics
    readonly modelId: string;               // powerful model for confirmation
  };
}
```

### Updated Job Processing

```
extraction job arrives:
  1. Default librarian extracts memories (fast model)
  2. For each extracted memory:
     a. Resolve topic via TopicCatalog
     b. Find matching librarians by topic glob
     c. If single match → that librarian handles it
     d. If multiple matches → run bidding/arbitration (librarians get library access)
     e. Winning librarian performs the add (with its own provider)
  3. Check complexity thresholds per topic subtree
  4. If threshold exceeded → trigger hybrid spawn check

reorganization job:
  1. Resolve which librarian owns this topic (via bidding if multiple)
  2. Winning librarian runs reorganize()

optimization job:
  1. Resolve which librarian owns this topic
  2. Winning librarian runs optimize() with its own model
```

### Backwards Compatibility

The CirculationDesk still accepts a single `Librarian` for backwards compatibility — it wraps it in a trivial registry with only the default librarian.

## 6. Data Flow

```
Per-turn (fast):
  User turn → CirculationDesk.enqueueExtraction()
    → Default librarian extracts memories [fast model]
    → For each memory:
        → TopicCatalog resolves topic
        → Registry finds matching librarians (topic glob)
        → If 1 match: that librarian handles it
        → If N matches: each librarian bids (with library access)
          → Clear winner? → self-resolution
          → Close bids? → default librarian arbitrates
        → Winner adds volume via its own provider
    → Check thresholds → maybe trigger spawn check

Spawn check (powerful, rare):
  Threshold exceeded → Registry.spawnSpecialist()
    → Powerful model assesses: "Is this topic complex enough?"
    → If yes: powerful model generates LibrarianDefinition JSON
    → Registry saves definition, creates ACP connection
    → New specialist is live, starts winning bids in its domain

Optimization (powerful, periodic):
  Per-topic threshold → winning librarian for that topic runs optimize()
    → Uses its own provider (possibly powerful model)
    → Prunes, summarizes, reorganizes
```

## 7. Files Changed

| File | Change |
|------|--------|
| `src/ai/library/types.ts` | Add `LibrarianDefinition`, `LibrarianBid`, `ArbitrationResult`, `ManagedLibrarian`, `LibrarianRegistry` interfaces. Add `bid()` to `Librarian`. Update `CirculationDeskThresholds` with spawning config. |
| `src/ai/library/librarian-definition.ts` | **New.** JSON schema validation, `loadDefinition()`, `saveDefinition()`, `loadAllDefinitions()`, `validateDefinition()` |
| `src/ai/library/librarian-registry.ts` | **New.** `createLibrarianRegistry()` factory — lifecycle, routing, bidding, arbitration, spawn logic, hot-reload |
| `src/ai/library/librarian.ts` | Add `bid()` method to `createLibrarian` — evaluates content against purpose via LLM, returns `LibrarianBid`. Update `createLibrarian` to accept optional `LibrarianDefinition`. |
| `src/ai/library/circulation-desk.ts` | Accept `LibrarianRegistry` (with backwards-compat for single `Librarian`). Route jobs through registry. Add spawn threshold checking. |
| `src/ai/library/library-services.ts` | Support passing `LibrarianRegistry` to CirculationDesk |
| `src/lib.ts` | Export new types and factories |
| `tests/librarian-definition.test.ts` | **New.** Validation, load/save, glob matching |
| `tests/librarian-registry.test.ts` | **New.** Registry lifecycle, bidding, arbitration, spawning |
| `tests/librarian.test.ts` | Add tests for `bid()` method |
| `tests/circulation-desk.test.ts` | Add tests for registry routing, spawn thresholds |
