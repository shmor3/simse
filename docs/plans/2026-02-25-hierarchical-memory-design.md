# Hierarchical Memory System Design

## Overview

Comprehensive improvement to the memory subsystem: hierarchical topics, rich metadata operators, improved adaptive learning, BM25 search with inverted index, query DSL, and file decomposition of vector-store.ts.

## 1. Hierarchical Topic System

### Data Model

```typescript
TopicNode {
  name: string              // 'rust'
  path: string              // 'programming/rust'
  parent?: string           // 'programming'
  children: string[]        // ['programming/rust/async']
  entryIds: Set<string>
  coOccurrence: Map<string, number>  // topic path -> count
}
```

### Behavior

- Path separator: `/` (e.g., `programming/rust/async`)
- Auto-hierarchy: adding entry with topic `programming/rust/async` auto-creates parent nodes
- Ancestor queries: `filterByTopic('programming')` returns entries from all descendants
- Co-occurrence tracking: topics on the same entry increment shared counters
- `getRelatedTopics(topic)` returns topics that frequently co-occur
- Multi-topic support: `metadata.topics: string[]` (array) alongside existing `metadata.topic: string`
- Topic merging: `mergeTopic(from, to)` reparents entries
- Auto-extraction stays as fallback when no explicit topics provided

## 2. Rich Metadata Model

### New Operators (additive to existing 8)

| Operator | Type | Description |
|----------|------|-------------|
| `gt` | numeric | Greater than |
| `gte` | numeric | Greater than or equal |
| `lt` | numeric | Less than |
| `lte` | numeric | Less than or equal |
| `in` | array | Value is in provided array |
| `notIn` | array | Value is not in provided array |
| `between` | range | Value is in `[min, max]` inclusive |

### Behavior

- Numeric operators fall back to linear scan (no range index — acceptable at typical sizes)
- Array metadata values: `metadata.tags = ['rust', 'async']` + `{key: 'tags', value: 'rust', mode: 'contains'}` matches
- `in` checks if metadata value is in provided array

## 3. Improved Adaptive Learning

### 3a. Explicit Relevance Feedback

```typescript
recordFeedback(queryId: string, entryId: string, relevant: boolean): void
```

- Explicit feedback weighted 5x stronger than implicit retrieval signals
- Stored persistently in learning state

### 3b. Per-Topic Weight Profiles

- Separate `{vector, recency, frequency}` weights per top-level topic
- Minimum 10 queries before topic-specific weights activate (falls back to global)
- Example: `programming` -> `{vector: 0.8, recency: 0.1, frequency: 0.1}`

### 3c. Per-Topic Interest Embeddings

- Separate interest vectors per topic cluster
- Used for recommendation boosting within topic context
- Prevents cross-topic interest dilution

### 3d. Query-Result Correlation

- Track which entries co-appear in results across queries
- Entries with high co-occurrence get cluster affinity boost
- Surfaces unseen related entries

### Persistence

- LearningState version bumped to 2
- Backward compatible: v1 state loads, missing fields default

## 4. Search & Query Improvements

### 4a. BM25 Text Scoring

New text search mode: `'bm25'`

- TF with saturation: `(tf * (k1 + 1)) / (tf + k1 * (1 - b + b * dl/avgdl))`
- IDF: `log((N - df + 0.5) / (df + 0.5) + 1)`
- Default params: `k1 = 1.2`, `b = 0.75`
- Multi-term queries split on whitespace, scores summed

### 4b. Inverted Text Index

- Term -> entryId[] mapping, built on add/delete
- BM25 and token search use inverted index: O(terms) instead of O(N)
- Fuzzy/substring/regex remain linear scan

### 4c. Query DSL

```typescript
query(dsl: string): Promise<AdvancedSearchResult[]>
```

Syntax: `topic:programming/rust metadata:language=rust "exact phrase" fuzzy~term score>0.5`

Parsed into `SearchOptions` internally. Sugar over `advancedSearch`.

### 4d. Field Boosting

```typescript
fieldBoosts?: {
  text?: number       // default 1.0
  metadata?: number   // default 1.0
  topic?: number      // default 1.0
}
```

### 4e. Weighted Ranking Mode

```typescript
rankBy: 'weighted'
rankWeights?: { vector?: number, text?: number, metadata?: number, recency?: number }
```

## 5. File Breakup & Performance

### vector-store.ts decomposition

| New File | Lines (est.) | Content |
|----------|-------------|---------|
| `vector-store.ts` | ~400 | Core CRUD, lifecycle, write-lock |
| `vector-search.ts` | ~300 | search(), advancedSearch(), textSearch() |
| `vector-recommend.ts` | ~200 | recommend(), access tracking, learning |
| `vector-serialize.ts` | ~250 | load(), save(), index rebuild, format detection |
| `inverted-index.ts` | ~200 | Inverted text index + BM25 scoring |

### Performance Optimizations

- Inverted index: BM25/token from O(N) to O(terms)
- Lazy index rebuild: skip unused indexes on load
- Search short-circuit: skip vector computation when pre-filtering eliminates all entries

## Testing Strategy

- New tests per feature (subtopics, numeric operators, BM25, DSL, feedback, per-topic learning)
- All existing tests must pass (backward compatible)
- Performance benchmark: search 10K entries comparing BM25 vs fuzzy vs token

## Non-Goals

- Range indexing for numeric metadata (not worth complexity at typical sizes)
- Removing any existing API methods
- Changing the on-disk content file format (.md files)
