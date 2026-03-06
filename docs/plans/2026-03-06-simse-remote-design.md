# simse-remote Design

## Overview

simse-remote enables authenticated remote access to simse instances. Two modes:

1. **Local tunnel** - User runs simse locally, it opens a WebSocket tunnel to the cloud relay. Web/remote clients connect through the relay to reach the local instance.
2. **Cloud-hosted** - simse-core runs cloud-side, clients connect directly through the relay.

## Components

### 1. simse-remote/ (Rust crate)

Runs on the user's machine alongside simse-core. Standalone binary following the same JSON-RPC 2.0 / NDJSON stdio pattern as all other engine crates.

**Modules:**

- **auth** - Authenticates with simse-api using session token or API key, receives a tunnel token.
- **tunnel** - Opens a persistent WebSocket to the cloud relay, multiplexes JSON-RPC traffic bidirectionally.
- **router** - Forwards relayed JSON-RPC requests to the local simse-core instance (via stdio or in-process).
- **heartbeat** - Keepalive pings, reconnection with exponential backoff (1s, 2s, 4s, max 30s).
- **server** - JSON-RPC dispatcher exposing remote-specific methods over NDJSON stdio.

### 2. simse-relay/ (Cloudflare Worker + Durable Objects)

Cloud relay service. Dedicated Cloudflare Worker for WebSocket handling.

**Components:**

- **WebSocket endpoints** - `/ws/tunnel` for local simse instances, `/ws/client` for web clients.
- **Durable Object per tunnel** - Holds the WebSocket pair (local <-> client), routes JSON-RPC messages bidirectionally.
- **Auth validation** - Validates Bearer token via simse-api gateway before WebSocket upgrade.
- **Session registry** - Maps userId -> active tunnel(s), enables "connect to my machine" discovery.
- **Analytics Engine** - Same `simse-analytics` dataset pattern as other workers.

## Data Flow

```
Web Client <-> simse-relay (Durable Object) <-> simse-remote <-> simse-core
               [WebSocket]                      [WebSocket]      [stdio/in-proc]
```

1. Local simse-remote authenticates -> gets tunnel token.
2. Opens WebSocket to `wss://relay.simse.dev/ws/tunnel?token=...`.
3. Relay creates Durable Object, stores connection.
4. Web client connects to `wss://relay.simse.dev/ws/client?token=...`.
5. Relay pairs client <-> tunnel WebSocket, forwards JSON-RPC bidirectionally.
6. simse-remote receives JSON-RPC from relay, forwards to local simse-core, returns response.

## simse-remote JSON-RPC Methods

| Domain | Method | Description |
|--------|--------|-------------|
| `auth/` | `login` | Authenticate with email/password or API key |
| `auth/` | `logout` | Revoke session |
| `auth/` | `status` | Current auth state |
| `tunnel/` | `connect` | Open tunnel to relay |
| `tunnel/` | `disconnect` | Close tunnel |
| `tunnel/` | `status` | Tunnel state (connected/disconnected/reconnecting) |
| `remote/` | `health` | Health check |

## simse-relay Routes

| Route | Method | Description |
|-------|--------|-------------|
| `/ws/tunnel` | GET (upgrade) | WebSocket for local simse instances |
| `/ws/client` | GET (upgrade) | WebSocket for web/remote clients |
| `/tunnels` | GET | List active tunnels for authenticated user |
| `/health` | GET | Health check |

## Error Handling

- Auth failures -> clear error with retry guidance.
- WebSocket disconnect -> automatic reconnection with exponential backoff (1s, 2s, 4s, max 30s).
- Relay unavailable -> circuit breaker pattern (same as simse-acp).
- Tunnel token expiry -> re-authenticate automatically.

## Security

- All WebSocket traffic over TLS (wss://).
- Tunnel tokens are short-lived, scoped to userId.
- Relay validates token on every WebSocket upgrade.
- No raw TCP exposure - everything tunneled through authenticated WebSocket.

## Crate Structure

```
simse-remote/
  Cargo.toml
  src/
    lib.rs          # Module declarations, re-exports
    main.rs         # Binary entry point (simse-remote-engine)
    error.rs        # RemoteError enum (REMOTE_ prefix)
    protocol.rs     # JSON-RPC param/result types
    transport.rs    # NdjsonTransport (same pattern as other crates)
    server.rs       # RemoteServer: JSON-RPC dispatcher (7 methods)
    auth.rs         # Auth client (login, logout, token management)
    tunnel.rs       # WebSocket tunnel client (connect, reconnect, multiplex)
    router.rs       # Local router (forward relayed requests to simse-core)
    heartbeat.rs    # Keepalive + reconnection logic

simse-relay/
  src/
    index.ts        # Hono app entry, health + auth middleware
    types.ts        # Env interface (Durable Objects, SecretsStore, AnalyticsEngine)
    tunnel.ts       # Durable Object: TunnelSession (WebSocket pair management)
    routes/
      ws.ts         # WebSocket upgrade handlers (/ws/tunnel, /ws/client)
      tunnels.ts    # REST endpoint for listing active tunnels
  wrangler.toml     # Durable Object + Analytics Engine bindings
```

## Dependencies

### simse-remote (Rust)
- tokio (async runtime)
- tokio-tungstenite (WebSocket client)
- serde / serde_json (JSON serialization)
- tracing (structured logging to stderr)

### simse-relay (TypeScript)
- hono (HTTP framework)
- Cloudflare Workers API (Durable Objects, WebSocket)
