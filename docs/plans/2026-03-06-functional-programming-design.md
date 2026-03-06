# Functional Programming Refactor Design

**Date:** 2026-03-06
**Scope:** All 10 Rust crates — pure FP style with pragmatic exceptions

## Goals

- Eliminate `&mut self` methods across the codebase (~300+ methods)
- Replace std collections with `im` crate persistent data structures
- Make all business logic pure functions: `(State, Input) -> (State, Output)`
- Server dispatch follows `(State, Request) -> (State, Response)` pattern
- Use Effect enums for I/O-performing operations
- TDD: write FP-style tests first, then refactor code to pass

## Approach: Hybrid (`im` for data, owned-return for servers)

Use `im` crate for core data structures where structural sharing matters. Use owned-return `(State, Response)` pattern for server dispatch. Keep std types at I/O boundaries.

## Core FP Conventions

### Immutability Rules

- All struct fields use `im::HashMap`, `im::Vector`, `im::OrdMap` instead of std equivalents
- No `&mut self` methods. Every method either:
  - Takes `self` (owned) and returns `(Self, T)` for state transitions
  - Takes `&self` and returns a value for queries (already pure)
- `let mut` only allowed for loop accumulators being built before freeze, or iterator collection

### Pure Function Extraction

- Every business logic operation becomes a free function: `fn operation(input: &State, args: Args) -> NewState`
- Struct methods become thin wrappers that call the free function and return new `Self`

### Server Pattern

```rust
// Pure handler — free function, no self
fn handle_create(state: ServerState, params: CreateParams) -> (ServerState, Result<CreateResult, ServerError>) {
    let new_entry = Entry::new(params.name);
    let new_state = state.with_entry(new_entry);
    (new_state, Ok(CreateResult { id: new_entry.id }))
}

// Server event loop — single mutable binding
async fn run(mut state: ServerState, transport: Transport) {
    while let Some(request) = transport.read().await {
        let (new_state, response) = dispatch(state, request);
        state = new_state;
        transport.write(response).await;
    }
}

// Dispatch table — routes to pure handlers
fn dispatch(state: ServerState, request: Request) -> (ServerState, Response) {
    match request.method.as_str() {
        "session/create" => {
            let (s, r) = handle_create(state, parse(request.params));
            (s, to_response(r))
        }
        // ...
    }
}
```

### Effect Enum for I/O

For crates with async side effects (simse-vsh command execution, simse-acp outbound connections), handlers return `(State, Effect)` where `Effect` is an enum the loop interprets:

```rust
enum Effect {
    None,
    Respond(Response),
    Execute { command: String, then: Box<dyn FnOnce(ExecResult, ServerState) -> (ServerState, Response)> },
    Multi(Vec<Effect>),
}
```

### Serialization Boundaries

- `im` types serialize/deserialize natively with `im = { version = "15", features = ["serde"] }`
- `Vec<u8>` byte buffers stay std at I/O boundaries
- Tokio channels and oneshot senders stay std (infrastructure, not data)

### Pragmatic Exceptions (marked `// PERF:`)

- Hot-path math in simse-adaptive (cosine similarity inner loops, PCN layer forward pass) may use `&mut` locals
- Tokio process/IO handles in simse-vsh executor (inherently mutable OS resources)

## Data Layer: `im` Crate Integration

**Dependency:** `im = { version = "15", features = ["serde"] }` in all 10 crates.

### Type Replacements

| std type | im replacement |
|----------|---------------|
| `HashMap<K, V>` | `im::HashMap<K, V>` |
| `Vec<T>` | `im::Vector<T>` |
| `BTreeMap<K, V>` | `im::OrdMap<K, V>` |
| `HashSet<T>` | `im::HashSet<T>` |

### What Stays std

- `String` (already immutable via ownership)
- `Vec<u8>` byte buffers at I/O boundaries
- Small fixed-size collections (3-element tuples, enum variants)
- Tokio channels, oneshot senders (infrastructure)

## Crate-by-Crate Scope

### simse-adaptive (80+ methods)

- VolumeStore: all insert/update/delete return new store
- TopicIndex, MetadataIndex, InvertedIndex, MagnitudeCache: immutable rebuild on change
- PCN layers: `im::Vector` for weights, `// PERF:` exception for inner-loop dot products
- Trainer: returns new model snapshot, loop shell applies it
- Persistence: serde works natively with `im` feature flag

### simse-core (40+ methods)

- Conversation: `add_user`, `add_assistant`, `compact` return new Conversation
- TaskList: CRUD returns new TaskList
- ToolRegistry: register/unregister return new registry, execute returns `(Registry, ToolResult)`
- HookSystem: register/unregister return new system
- EventBus: subscribe/unsubscribe return new bus, publish returns `(EventBus, Vec<Effect>)`
- SessionManager: create/fork/delete return new manager
- CoreContext: each operation returns new CoreContext
- ChainRunner + AgenticLoop: pure step functions, loop shell manages state + async

### simse-vfs (20+ methods)

- VfsNode tree becomes `im::HashMap` — structural sharing on writes
- `mkdir`, `write_file`, `delete` return new Vfs
- DiskFs: Effect enum for actual disk I/O
- Diff/glob/search: already mostly pure, minimal changes

### simse-acp (19+ methods)

- Connection pool as `im::HashMap<String, ConnectionState>`
- Session management returns new client state
- Resilience (circuit breaker, health): state threaded through, Effect for process spawning

### simse-mcp (30+ methods)

- Tool and resource registries as `im::HashMap`
- Transport connections: Effect enum for stdio/HTTP I/O
- Notification handlers return new state

### simse-vsh (14+ methods)

- Sessions as `im::HashMap`
- Session state (env, aliases, history) as `im` collections
- Command execution via Effect enum

### simse-vnet (18+ methods)

- Mock store and session manager as `im` types
- Mock HTTP responses: pure matching, return new state
- Real network calls via Effect enum

### simse-sandbox (60+ methods)

- Composed VFS+VSH+VNet state threaded through
- Backend trait: `fn op(self, ...) -> (Self, Result)` instead of `&mut self`
- SSH pool: Effect enum for russh channel operations

### simse-ui-core (25+ methods)

- ConversationBuffer, PermissionManager, KeybindingRegistry: return new state
- Settings/config schemas: mostly data, minimal change
- Command registry: pure lookup, no mutation needed

### simse-tui (50+ methods)

- App model: `update` returns new `App` instead of `&mut self`
- All overlays/dialogs: return new overlay state
- Autocomplete, at_mention, spinner: return new state
- Event loop: single `mut state` binding, reassigned each cycle

## TDD Strategy

**Approach:** Write FP-style tests first, then refactor code to pass them.

### Test Properties to Assert

- **Immutability:** Original state unchanged after operation
- **Determinism:** Same input always produces same output
- **No side effects:** Pure handlers produce Effect values, don't execute them
- **Structural sharing:** Clone is cheap — benchmark tests for perf-critical paths

### Test Migration Order (within each crate)

1. Pure utility functions (cosine, diff, glob) — tests barely change
2. Data structures (Conversation, TaskList, VolumeStore) — immutability assertions
3. Server handlers — test as `(State, Params) -> (State, Result)` functions
4. Effect interpretation — test effects are correctly produced, then loop executes them
5. Integration tests — JSON-RPC round-trips still work end-to-end

### Test Counts to Match or Exceed

| Crate | Current | Target |
|-------|---------|--------|
| simse-core | 779+ | 779+ |
| simse-adaptive | ~200 | ~200 |
| simse-vfs | ~50 | ~50 |
| simse-acp | 133 | 133 |
| simse-mcp | 163 | 163 |
| simse-vsh | 14 | 14 |
| simse-vnet | 16 | 16 |
| simse-sandbox | 31 | 31 |
| simse-ui-core | ~50 | ~50 |
| simse-tui | ~30 | ~30 |

## Execution Order

Respects crate dependency graph:

```
Phase 1: Leaf crates (no internal deps)
  simse-adaptive, simse-vfs, simse-vsh, simse-vnet

Phase 2: Composition crates (depend on Phase 1)
  simse-sandbox (uses vfs, vsh, vnet), simse-acp, simse-mcp

Phase 3: Orchestration
  simse-core (uses adaptive, acp, mcp)

Phase 4: UI layer
  simse-ui-core, simse-tui (uses ui-core)
```

### TDD Cycle Per Crate

1. Add `im = { version = "15", features = ["serde"] }` to `Cargo.toml`
2. Write FP-style tests for the first module (fails — methods still `&mut self`)
3. Refactor types: replace std collections with `im` equivalents
4. Refactor methods: `&mut self` → `self` + return `(Self, T)`
5. Extract pure free functions from impl blocks
6. Refactor server dispatch to `(State, Request) -> (State, Response)`
7. Add Effect enum for I/O-performing crates
8. Run tests — all green
9. Move to next module in the crate
10. Repeat for next crate in the phase

### Build Verification Between Phases

- After Phase 1: all 4 leaf crates compile and pass tests independently
- After Phase 2: simse-sandbox compiles against new APIs, ACP/MCP pass
- After Phase 3: simse-core compiles against new APIs
- After Phase 4: full `cargo build --release` and `cargo test` from workspace root
