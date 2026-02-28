# simse-vector Rust Engine Design

## Summary

Refactor simse-vector from pure TypeScript to a Rust subprocess engine with a thin TS client, following the same subprocess + JSON-RPC 2.0 pattern used by simse-vfs. Rust owns all storage, computation, and file persistence. TS keeps LLM-dependent modules (librarian, circulation desk) and becomes a thin async client for everything else.

## Architecture

```
TypeScript (thin client)            JSON-RPC 2.0 / NDJSON stdio            Rust Engine
┌────────────────────────┐     ←────────────────────────────────→     ┌──────────────────────┐
│ Library facade          │                                           │ VolumeStore           │
│ Shelf wrapper           │                                           │ TopicIndex            │
│ Librarian (LLM)         │                                           │ MetadataIndex         │
│ LibrarianRegistry (ACP) │                                           │ MagnitudeCache        │
│ CirculationDesk (queue) │                                           │ InvertedIndex (BM25)  │
│ LibraryServices (mw)    │                                           │ TextSearch            │
│ Error mapping           │                                           │ Deduplication         │
│ Client (JSON-RPC)       │                                           │ Recommendation        │
└────────────────────────┘                                           │ PatronLearning        │
                                                                      │ TopicCatalog          │
                                                                      │ QueryDSL              │
                                                                      │ Persistence (gzip v2) │
                                                                      │ PromptInjection fmt   │
                                                                      └──────────────────────┘
```

## Decisions

- **Integration:** Subprocess + JSON-RPC (consistent with VFS)
- **Scope:** Everything possible to Rust (except LLM-dependent librarian/registry/circulation)
- **Persistence:** Rust owns file I/O directly (same v2 binary format for backward compat)
- **Embeddings:** TS obtains embeddings from provider, passes pre-computed arrays to Rust
- **Format compatibility:** Rust reads existing TS-written stores; zero migration needed
