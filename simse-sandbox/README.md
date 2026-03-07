# simse-sandbox

Unified sandbox engine combining VFS (virtual/disk filesystem), VSH (virtual shell), and VNet (virtual network) over JSON-RPC 2.0 / NDJSON stdio. Supports local and SSH backends via enum dispatch.

## Build

```bash
cargo build --release
```

## Test

```bash
cargo test
```

## Binary

`simse-sandbox-engine` — JSON-RPC server exposing 63 methods across 7 domains for filesystem operations, shell execution, and network access.
