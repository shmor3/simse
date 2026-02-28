# Embedder Consolidation: Remove TS, Port TEI to Rust

**Date:** 2026-02-28

## Goal

Remove TypeScript local-embedder and TEI bridge from the simse package. The Rust engine (`simse-engine`) already handles local embedding via Candle/BertEmbedder. Add TEI HTTP bridge support to the Rust engine so both embedding paths are available server-side.

## Current State

### TypeScript (to remove)
- `src/ai/acp/local-embedder.ts` — In-process ONNX via `@huggingface/transformers`
- `src/ai/acp/tei-bridge.ts` — HTTP bridge to external TEI server
- Both exported from `src/lib.ts` as public API
- Tests: `local-embedder.test.ts`, `tei-bridge.test.ts`, `e2e-local-embedder.test.ts`, `e2e-library-pipeline.test.ts`

### Rust Engine (already exists)
- `BertEmbedder` in `src/models/bert.rs` — Candle-based local embedding
- `NomicBERT` in `src/models/nomic_bert.rs` — NomicBERT architecture
- `Embedder` trait in `src/models/mod.rs`
- `ModelRegistry` manages loaded embedders
- `--embedding-model` CLI flag for local model selection

## Changes

### Rust: Add TEI Bridge

**New file:** `simse-engine/src/models/tei.rs`

`TeiEmbedder` struct implementing `Embedder` trait:
- Sends `POST /embed` with `{ inputs, normalize, truncate }` to TEI server
- Parses `Vec<Vec<f32>>` response
- Configurable URL, normalize, truncate, timeout
- Uses `ureq` for synchronous HTTP (matches the sync server architecture)

**Config:** New `--tei-url` CLI flag in `config.rs`

**Routing:** `tei://` model ID prefix routes to TEI embedder; bare model IDs route to local Candle embedder. Both can coexist.

**Startup:** When `--tei-url` is set, register a TEI embedder under `"tei://default"` key in `ModelRegistry`.

### TypeScript: Remove Embedders

- Delete `src/ai/acp/local-embedder.ts` and `src/ai/acp/tei-bridge.ts`
- Remove exports from `src/lib.ts`
- Delete unit tests (`local-embedder.test.ts`, `tei-bridge.test.ts`)
- Delete `e2e-local-embedder.test.ts`
- Rewrite `e2e-library-pipeline.test.ts` to use a mock embedder
- Remove `@huggingface/transformers` from `package.json` if unused elsewhere

### Embedding path after migration

```
User code → createACPEmbedder(client) → ACP protocol → simse-engine
                                                          ├─ BertEmbedder (local Candle)
                                                          └─ TeiEmbedder (HTTP → TEI server)
```

## API Impact

Breaking change: `createLocalEmbedder()` and `createTEIEmbedder()` removed from public API. Users migrate to `createACPEmbedder()` wrapping the Rust engine (which now handles both local and TEI embedding).
