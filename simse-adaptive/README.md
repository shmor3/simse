# simse-adaptive

Adaptive learning engine with vector store and predictive coding network (PCN). Provides storage, search, cataloging, deduplication, recommendation, and summarization over JSON-RPC 2.0 / NDJSON stdio.

## Build

```bash
cargo build --release
```

## Test

```bash
cargo test
```

## Binary

`simse-adaptive-engine` — JSON-RPC server exposing store CRUD, cosine similarity search, BM25 text search, topic classification, graph indexing, and PCN prediction/training.
