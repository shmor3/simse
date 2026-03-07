# simse-core

Orchestration library and JSON-RPC binary server. Ties together the event bus, logger, config, task list, hook system, session manager, tool registry, and library into `CoreContext`.

## Build

```bash
cargo build --release
```

## Test

```bash
cargo test
```

## Binary

`simse-core-engine` — JSON-RPC 2.0 / NDJSON stdio server exposing 48 methods across 9 domains (`core/`, `session/`, `conversation/`, `task/`, `event/`, `hook/`, `tool/`, `chain/`, `loop/`).
