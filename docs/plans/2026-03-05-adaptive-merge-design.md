# Merge simse-vector + simse-predictive-coding → simse-adaptive

**Date:** 2026-03-05
**Status:** Approved

## Overview

Rename `simse-vector/` to `simse-adaptive/` and deeply integrate `simse-predictive-coding/` into it. The result is a single crate (`simse-adaptive-engine`) with one binary and one combined JSON-RPC server exposing both vector store and predictive coding methods.

## Rename

- Directory: `simse-vector/` → `simse-adaptive/`
- Package name: `simse-adaptive-engine`
- Lib name: `simse_adaptive_engine`
- Binary name: `simse-adaptive-engine`

## Merge Strategy

PCN modules move into `simse-adaptive/src/` as top-level modules. Shared files use the vector store version as the base with PCN types merged in.

| Shared file | Merge approach |
|---|---|
| `error.rs` | Add PCN error variants, rename enum to `AdaptiveError` |
| `protocol.rs` | Add PCN request/response types alongside vector ones |
| `server.rs` | Single `AdaptiveServer` dispatcher handling both vector + PCN JSON-RPC methods |
| `transport.rs` | Keep as-is (identical NDJSON pattern) |
| `persistence.rs` | Vector's persistence stays, PCN's snapshot persistence merges in |
| `main.rs` | Single binary launching `AdaptiveServer` |

## New Modules (from PCN)

- `config.rs` — PCN model configuration
- `encoder.rs` — input encoding
- `vocabulary.rs` — token vocabulary management
- `network.rs` — neural network layers
- `layer.rs` — layer implementation
- `predictor.rs` — prediction engine (uses VectorStore embeddings directly)
- `trainer.rs` — model training
- `snapshot.rs` — model snapshots

## Deep Integration

The `predictor.rs` takes a reference to the `VectorStore` and uses its embeddings + similarity functions directly, rather than going through JSON-RPC.

## Dependency Updates

- `simse-adaptive/Cargo.toml`: Add `rand`, `tokio` from PCN deps
- `simse-core/Cargo.toml`: `simse-vector-engine` → `simse-adaptive-engine`
- Root `Cargo.toml`: `simse-vector` → `simse-adaptive`, remove `simse-predictive-coding`
- All `use simse_vector_engine::` → `use simse_adaptive_engine::` across simse-core and simse-bridge
- `CLAUDE.md`: Update all references
- Build commands: `build:vector-engine` → `build:adaptive-engine`

## Deletion

Remove `simse-predictive-coding/` entirely after merge.
