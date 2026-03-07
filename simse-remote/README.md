# simse-remote

Remote access engine providing authentication, WebSocket tunneling, and request routing over JSON-RPC 2.0 / NDJSON stdio.

## Build

```bash
cargo build --release
```

## Test

```bash
cargo test
```

## Binary

`simse-remote-engine` — JSON-RPC server exposing 7 methods across `auth/`, `tunnel/`, and `remote/` domains.
