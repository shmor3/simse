# Library Memory System — Design

**Date**: 2026-02-26
**Status**: Approved

## Problem

The current memory system has several gaps:

1. **Topic naming is naive** — auto-extracts top 5 frequent words, no meaningful subtopic structure
2. **Summarization is manual only** — requires explicit IDs, no automatic per-turn processing
3. **No automatic organization** — no reclassification, no catalog maintenance
4. **No per-agent memory isolation** — subagents share parent's MemoryManager with no scoping
5. **No per-turn processing** — `memoryMiddleware.afterResponse()` hook exists but has no default implementation
6. **Terminology is generic** — "memory manager", "vector entry", "search result" lack domain identity

## Solution

Redesign the entire memory system using a **library analogy**. The memory store becomes a library, entries become volumes, topics become a catalog, summarization models become librarians, and agent-scoped sections become shelves. Add automatic per-turn extraction, background processing via a circulation desk (queue), and librarian-driven summarization and reorganization.

## The Library Model

| Concept | Library Term | Code Name |
|---------|-------------|-----------|
| Full memory store | **Library** | `Library` |
| Namespaced agent section | **Shelf** | `Shelf` |
| Topic hierarchy | **Catalog** | `TopicCatalog` |
| Individual memory entry | **Volume** | `Volume` |
| Summarization/organization model | **Librarian** | `Librarian` |
| Condensed summary of volumes | **Compendium** | entry with `entryType: 'compendium'` |
| Tags on entries | **Index cards** | `tags` metadata field |
| Background processing queue | **Circulation Desk** | `CirculationDesk` |
| User behavior learning | **Patron Learning** | `PatronProfile` |
| Per-turn middleware | **Library Services** | `LibraryServices` |
| Physical vector storage | **Stacks** | `Stacks` |
| Embedding preservation | **Preservation** | preservation module |

## File Renames

```
src/ai/memory/                    →  src/ai/library/
  memory.ts                       →  library.ts
  vector-store.ts                 →  stacks.ts
  vector-persistence.ts           →  stacks-persistence.ts
  types.ts                        →  types.ts
  compression.ts                  →  preservation.ts
  indexing.ts                     →  cataloging.ts
  deduplication.ts                →  deduplication.ts
  recommendation.ts               →  recommendation.ts
  text-search.ts                  →  text-search.ts
  learning.ts                     →  patron-learning.ts
  cosine.ts                       →  cosine.ts
  storage.ts                      →  storage.ts
  (new) topic-catalog.ts
  (new) librarian.ts
  (new) circulation-desk.ts
  (new) shelf.ts
```

## Type Renames

### Volume (was VectorEntry)

```typescript
interface Volume {
  readonly id: string;
  readonly text: string;
  readonly embedding: readonly number[];
  readonly metadata: Readonly<Record<string, string>>;
  readonly timestamp: number;
}
```

### Library (was MemoryManager)

```typescript
interface Library {
  // Lifecycle
  initialize: () => Promise<void>;
  dispose: () => Promise<void>;

  // Shelving
  add: (text: string, metadata?) => Promise<string>;
  addBatch: (entries: Array<{ text: string; metadata? }>) => Promise<string[]>;

  // Lookups
  search: (query: string, maxResults?, threshold?) => Promise<Lookup[]>;
  textSearch: (options: TextSearchOptions) => TextLookup[];
  advancedSearch: (options: SearchOptions) => Promise<AdvancedLookup[]>;
  query: (dsl: string) => Promise<AdvancedLookup[]>;

  // Catalog
  getTopics: () => TopicInfo[];
  filterByTopic: (topics: string[]) => Volume[];
  filterByMetadata: (filters: MetadataFilter[]) => Volume[];
  filterByDateRange: (range: DateRange) => Volume[];

  // Recommendations
  recommend: (query: string, options?) => Promise<Recommendation[]>;

  // Deduplication
  findDuplicates: (threshold?) => DuplicateVolumes[];
  checkDuplicate: (text: string) => Promise<DuplicateCheckResult>;

  // Compendium (was summarize)
  compendium: (options: CompendiumOptions) => Promise<CompendiumResult>;

  // Patron learning
  recordFeedback: (volumeId: string, relevant: boolean) => void;
  patronProfile: PatronProfile | undefined;

  // Volume access
  getById: (id: string) => Volume | undefined;
  getAll: () => Volume[];
  delete: (id: string) => Promise<boolean>;
  deleteBatch: (ids: string[]) => Promise<number>;
  clear: () => Promise<void>;

  // Shelf management
  shelf: (name: string) => Shelf;
  shelves: () => string[];

  // Status
  size: number;
  isInitialized: boolean;
}
```

### Lookup (was SearchResult)

```typescript
interface Lookup {
  readonly volume: Volume;
  readonly score: number;
}
```

### Shelf (new — scoped library view)

```typescript
interface Shelf {
  readonly name: string;
  readonly add: (text: string, metadata?) => Promise<string>;
  readonly search: (query: string, opts?) => Promise<Lookup[]>;
  readonly searchGlobal: (query: string, opts?) => Promise<Lookup[]>;
  readonly volumes: () => Volume[];
  readonly librarian: Librarian;
  readonly circulationDesk: CirculationDesk;
}
```

### CompendiumOptions (was SummarizeOptions)

```typescript
interface CompendiumOptions {
  readonly ids: readonly string[];
  readonly prompt?: string;
  readonly systemPrompt?: string;
  readonly deleteOriginals?: boolean;
  readonly metadata?: Readonly<Record<string, string>>;
}
```

### CompendiumResult (was SummarizeResult)

```typescript
interface CompendiumResult {
  readonly compendiumId: string;
  readonly text: string;
  readonly topic: string;
  readonly subtopics: string[];
  readonly tags: string[];
  readonly sourceIds: readonly string[];
  readonly deletedOriginals: boolean;
}
```

### CirculationDesk (new — background queue)

```typescript
interface CirculationDesk {
  enqueueExtraction: (turn: TurnContext) => void;
  enqueueCompendium: (topic: string) => void;
  enqueueReorganization: (topic: string) => void;
  drain: () => Promise<void>;
  flush: () => Promise<void>;
  dispose: () => void;
  pending: number;
  processing: boolean;
}
```

### Librarian (new — summarization/organization model)

```typescript
interface Librarian {
  // Per-turn extraction
  extract: (turn: TurnContext) => Promise<ExtractionResult>;

  // Compendium creation
  summarize: (volumes: Volume[], topic: string) => Promise<CompendiumResult>;

  // Catalog maintenance
  classifyTopic: (text: string, existingTopics: string[]) => Promise<ClassificationResult>;
  proposeName: (volumes: Volume[]) => Promise<string>;

  // Reorganization
  reorganize: (topic: string, volumes: Volume[]) => Promise<ReorganizationPlan>;
}
```

### ExtractionResult (new)

```typescript
interface ExtractionResult {
  memories: Array<{
    text: string;
    topic: string;
    tags: string[];
    entryType: 'fact' | 'decision' | 'observation';
  }>;
}
```

### ReorganizationPlan (new)

```typescript
interface ReorganizationPlan {
  moves: Array<{ volumeId: string; newTopic: string }>;
  newSubtopics: string[];
  merges: Array<{ source: string; target: string }>;
}
```

### LibraryServices (was MemoryMiddleware)

```typescript
interface LibraryServices {
  enrichSystemPrompt: (context: LibraryContext) => Promise<string>;
  afterResponse: (userInput: string, response: string) => Promise<void>;
}
```

### PatronProfile (was LearningProfile)

```typescript
interface PatronProfile {
  readonly queryCount: number;
  readonly topTopics: readonly string[];
  readonly adaptedWeights: WeightProfile;
}
```

## Topic Catalog

### TopicCatalog (replaces TopicIndex)

New file: `src/ai/library/topic-catalog.ts`

The catalog is a managed hierarchical classification system with normalization:

```
architecture/
  database/
    schema/
    optimization/
  api/
    endpoints/
    authentication/
decisions/
  technical/
  process/
research/
  references/
bugs/
  open/
  resolved/
```

**Operations:**
- `catalog.resolve(proposedTopic)` — normalizes a librarian-proposed topic against existing entries using Levenshtein distance. Returns canonical path or registers new topic.
- `catalog.relocate(volumeId, newTopic)` — moves a volume to a different catalog section
- `catalog.merge(sourceTopic, targetTopic)` — merges two sections
- `catalog.sections()` — returns the full tree
- `catalog.volumes(topic)` — returns volumes in a section

**Alias registry** (persisted in index file):
```typescript
{
  'arch/db': 'architecture/database',
  'db': 'architecture/database',
  'auth': 'architecture/api/authentication',
}
```

### Topic naming

LLM-generated by the librarian during per-turn extraction. The catalog normalizer ensures consistency:
- Checks proposed topic against existing catalog via string similarity
- Maps aliases to canonical names
- Adds genuinely new topics to the catalog

## Librarian

New file: `src/ai/library/librarian.ts`

Created via `createLibrarian(textGenerator, options)`. Wraps a `TextGenerationProvider`.

### Per-turn extraction (`extract()`)

Receives user input + assistant response. Prompts the LLM to:
1. Identify distinct facts, decisions, and observations worth remembering
2. Propose a hierarchical topic path for each (`/`-separated)
3. Tag each with relevant keywords
4. Skip trivial or conversational content

### Compendium creation (`summarize()`)

Takes volumes from the same topic. Prompts the LLM to:
1. Condense into a single coherent summary
2. Propose a refined topic/subtopic name if original is too broad
3. Suggest subtopic splits if content covers distinct areas
4. Preserve all key facts and decisions

### Reorganization (`reorganize()`)

Triggered when a topic section grows large. Reviews all volumes and returns:
- Moves (volume → new topic)
- New subtopics to create
- Merges of related topics

## Circulation Desk (Processing Queue)

New file: `src/ai/library/circulation-desk.ts`

Handles async background processing. Runs after each turn without blocking the conversation.

### Pipeline

```
Turn ends
  ↓
enqueueExtraction(turnContext)
  ↓ (async)
Librarian.extract() → memories[]
  ↓
For each memory:
  TopicCatalog.resolve(topic) → canonical topic
  Check duplicate via Stacks
  Embed + store with shelf + topic + tags
  ↓
Check thresholds per topic:
  If 10+ unsummarized volumes AND oldest > 15min:
    enqueueCompendium(topic)
  If 30+ total volumes:
    enqueueReorganization(topic)
```

### Compendium job

1. Gather unsummarized volumes for the topic
2. Call `librarian.summarize(volumes, topic)`
3. Store compendium with `entryType: 'compendium'`
4. Mark original volumes with `summarizedInto: compendiumId`
5. Optionally delete originals (configurable)

### Reorganization job

1. Call `librarian.reorganize(topic, volumes)`
2. Execute moves via `catalog.relocate()`
3. Create new subtopics
4. Execute merges via `catalog.merge()`

### Error handling

Failed queue items retried once, then logged and dropped. Queue never blocks the main conversation loop.

### Configurable thresholds

```typescript
{
  compendium: {
    minEntries: 10,
    minAgeMs: 900_000,      // 15 minutes
    deleteOriginals: false,
  },
  reorganization: {
    maxVolumesPerTopic: 30,
  },
}
```

## Shelf-Scoped Memory for Agents

New file: `src/ai/library/shelf.ts`

Created via `library.shelf('researcher')`. Returns a `Shelf` that auto-scopes all operations.

### Metadata on volumes

```typescript
metadata: {
  shelf: 'researcher',
  topic: 'architecture/database/schema',
  tags: 'decision,postgresql,users',
  turnNumber: '5',
  entryType: 'fact' | 'decision' | 'observation' | 'compendium',
  summarizedFrom: 'id1,id2,...',  // for compendia only
}
```

### Subagent integration

Changes to `src/ai/tools/subagent-tools.ts`:
- Subagent spawns → gets a `Shelf` with `shelf: agentName`
- Subagent's memory tools (`library_shelve`, `library_search`) go through the shelf wrapper
- Subagent's librarian processes its turns independently
- On subagent completion, its circulation desk is drained before results return to parent

### Library services (middleware)

Changes to `src/ai/loop/agentic-loop.ts`:
- `LibraryServices.afterResponse()` calls `circulationDesk.enqueueExtraction()`
- Default implementation wires up librarian + circulation desk automatically
- `enrichSystemPrompt()` searches relevant volumes from the shelf to inject context

## Tool Renames

| Current | New | Description |
|---------|-----|-------------|
| `memory_search` | `library_search` | Search volumes |
| `memory_add` | `library_shelve` | Add a volume |
| `memory_delete` | `library_withdraw` | Remove a volume |
| (new) | `library_catalog` | Browse the topic catalog |
| (new) | `library_compact` | Trigger compendium for a topic |

MCP server tools follow the same pattern (`library-search`, `library-shelve`, etc.).

## Error Renames

`src/errors/memory.ts` → `src/errors/library.ts`:

| Current | New |
|---------|-----|
| `createMemoryError` | `createLibraryError` |
| `createVectorStoreError` | `createStacksError` |
| `isMemoryError` | `isLibraryError` |
| `isVectorStoreError` | `isStacksError` |
| `createEmbeddingError` | `createEmbeddingError` (stays) |

## Public API (lib.ts)

```typescript
// Before
export { createMemoryManager } from './ai/memory/memory.js';
export type { MemoryManager, VectorEntry, SearchResult } from './ai/memory/types.js';

// After
export { createLibrary } from './ai/library/library.js';
export type { Library, Volume, Lookup } from './ai/library/types.js';
```

## Migration Path

Clean break — no backwards compat shims or deprecation aliases:
1. Rename files and types in one pass
2. Update all internal consumers (agentic loop, tools, MCP server, CLI)
3. Update all tests
4. Update `lib.ts` barrel exports
5. Update `CLAUDE.md` documentation
