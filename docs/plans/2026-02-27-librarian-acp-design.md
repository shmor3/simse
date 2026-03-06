# Librarian ACP Integration Design

**Goal:** Make the Librarian default to simse-engine ACP for fast per-turn extraction, support any ACP as a provider, and optimize/prune memory with a powerful model (Opus 4.6) when thresholds are exceeded.

**Architecture:** Extend `TextGenerationProvider` with optional `generateWithModel()` for model-switched generation. The Librarian uses `generate()` for fast daily work and `generateWithModel()` for periodic optimization. The CirculationDesk auto-escalates to optimization when per-topic or global thresholds are crossed.

**Tech Stack:** TypeScript, Bun, ACP JSON-RPC 2.0, simse-engine (default), any ACP server (pluggable)

---

## 1. Extended TextGenerationProvider

The `TextGenerationProvider` interface gains an optional `generateWithModel()` method:

```typescript
export interface TextGenerationProvider {
  readonly generate: (prompt: string, systemPrompt?: string) => Promise<string>;
  readonly generateWithModel?: (
    prompt: string,
    modelId: string,
    systemPrompt?: string,
  ) => Promise<string>;
}
```

- `generate()` uses the default model (unchanged contract)
- `generateWithModel()` switches to a specific model for that request
- Optional — providers that don't support model switching simply omit it
- Callers check `if (provider.generateWithModel)` before using

## 2. ACP Generator Model Switching

`createACPGenerator` in `acp-adapters.ts` implements `generateWithModel`:

```typescript
generateWithModel: async (prompt, modelId, systemPrompt?) => {
  // 1. Create or reuse a session
  // 2. setSessionModel(sessionId, modelId) — switch to requested model
  // 3. Generate with the switched model
  // 4. Return result
}
```

The ACP client already supports `setSessionModel(sessionId, modelId)`. The adapter wraps this into the clean `generateWithModel` interface.

## 3. Default Librarian Factory

New convenience factory that auto-creates a provider from an ACP client:

```typescript
export function createDefaultLibrarian(acpClient: ACPClient): Librarian {
  const provider = createACPGenerator({ client: acpClient });
  return createLibrarian(provider);
}
```

- Uses whatever default model the ACP server has (simse-engine = Llama 3.2 3B)
- The Librarian itself stays provider-agnostic
- Any ACP can be passed in — not locked to simse-engine

## 4. Librarian Optimization

New `optimize()` method on the Librarian interface:

```typescript
export interface Librarian {
  // ... existing methods (extract, summarize, classifyTopic, reorganize)
  readonly optimize: (
    volumes: readonly Volume[],
    topic: string,
    modelId: string,
  ) => Promise<OptimizationResult>;
}
```

```typescript
export interface OptimizationResult {
  readonly pruned: readonly string[];       // IDs of volumes to remove
  readonly summary: string;                  // Condensed summary of remaining
  readonly reorganization: ReorganizationPlan;
  readonly modelUsed: string;
}
```

`optimize()` calls `generateWithModel(prompt, modelId)` with a comprehensive prompt that asks the powerful model to:
1. **Prune** — identify redundant, outdated, or low-value volumes
2. **Summarize** — condense remaining volumes into a compendium
3. **Reorganize** — suggest topic restructuring

Falls back to no-op result if `generateWithModel` is not available on the provider.

## 5. Dual-Threshold Escalation

`CirculationDeskThresholds` gains optimization thresholds:

```typescript
export interface CirculationDeskThresholds {
  readonly compendium?: { ... };         // existing
  readonly reorganization?: { ... };     // existing
  readonly optimization?: {
    readonly topicThreshold?: number;    // default 50 — per-topic trigger
    readonly globalThreshold?: number;   // default 500 — total library trigger
    readonly modelId: string;            // e.g. 'claude-opus-4-6'
  };
}
```

The CirculationDesk checks thresholds after each extraction:
- If any topic exceeds `topicThreshold` → enqueue topic optimization
- If total volumes exceed `globalThreshold` → enqueue global optimization (all topics)

## 6. CirculationDesk Integration

New job type and method:

```typescript
type Job =
  | { type: 'extraction'; turn: TurnContext }
  | { type: 'compendium'; topic: string }
  | { type: 'reorganization'; topic: string }
  | { type: 'optimization'; topic: string; modelId: string };

export interface CirculationDesk {
  // ... existing methods
  readonly enqueueOptimization: (topic: string) => void;
}
```

Processing an `optimization` job:
1. Get all volumes for the topic
2. Call `librarian.optimize(volumes, topic, modelId)`
3. Remove pruned volumes
4. Add compendium summary as new volume
5. Apply reorganization plan via catalog

Auto-escalation happens in `processJob('extraction')`:
```
after extraction completes:
  topicVolumes = getVolumesForTopic(topic)
  if topicVolumes.length >= topicThreshold:
    enqueueOptimization(topic)
  totalVolumes = getTotalVolumeCount()
  if totalVolumes >= globalThreshold:
    for each topic with volumes:
      enqueueOptimization(topic)
```

## 7. Data Flow

```
Per-turn (fast):
  User turn → CirculationDesk.enqueueExtraction()
    → Librarian.extract() [generate() → default model]
    → Dedup check → Add volumes → Check thresholds

Periodic (powerful):
  Threshold exceeded → CirculationDesk.enqueueOptimization()
    → Librarian.optimize() [generateWithModel() → Opus 4.6]
    → Prune volumes → Add compendium → Reorganize topics
```

## 8. Files Changed

| File | Change |
|------|--------|
| `src/ai/library/types.ts` | Add `generateWithModel?` to `TextGenerationProvider`, add `OptimizationResult`, update `CirculationDeskThresholds`, add `optimize` to `Librarian`, add `enqueueOptimization` to `CirculationDesk` |
| `src/ai/acp/acp-adapters.ts` | Implement `generateWithModel` in `createACPGenerator` |
| `src/ai/library/librarian.ts` | Add `optimize()` method, add `createDefaultLibrarian()` factory |
| `src/ai/library/circulation-desk.ts` | Add `optimization` job type, auto-escalation logic, `enqueueOptimization()`, volume deletion/addition during optimization |
| `src/lib.ts` | Export `createDefaultLibrarian`, `OptimizationResult` |
| `tests/acp-adapters.test.ts` | Test `generateWithModel` |
| `tests/librarian.test.ts` | Test `optimize()` and `createDefaultLibrarian()` |
| `tests/circulation-desk.test.ts` | Test optimization job, auto-escalation, threshold logic |
