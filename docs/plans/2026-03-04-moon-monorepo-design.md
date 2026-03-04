# Moon Monorepo Design

**Date**: 2026-03-04
**Goal**: Integrate moon as the monorepo task runner with four top-level commands (build, test, dev, start) using automatic project discovery — no hardcoded project lists.

## Project Discovery

```yaml
# .moon/workspace.yml
projects:
  - 'simse-*'
```

Every `simse-*` directory containing a `moon.yml` is auto-discovered. Adding a new package just means dropping a `moon.yml` in it.

## Per-Project moon.yml (Minimal)

Each package declares only its language and tags:

| Package | Language | Tags |
|---------|----------|------|
| simse-acp | rust | engine |
| simse-bridge | rust | engine |
| simse-core | rust | engine |
| simse-engine | rust | engine |
| simse-mcp | rust | engine |
| simse-tui | rust | app |
| simse-ui-core | rust | lib |
| simse-vector | rust | engine |
| simse-vfs | rust | engine |
| simse-vnet | rust | engine |
| simse-vsh | rust | engine |
| simse-cloud | typescript | app |
| simse-code | typescript | app |
| simse-landing | typescript | app |
| simse-mailer | typescript | app |
| simse-brand | other | assets |

## Task Inheritance

### `.moon/tasks/rust.yml`

```yaml
inheritedBy:
  toolchains: ['rust']

fileGroups:
  sources:
    - 'src/**/*'
    - 'Cargo.toml'

tasks:
  build:
    command: 'cargo build --release'
    inputs:
      - '@globs(sources)'
  dev:
    command: 'cargo build'
    inputs:
      - '@globs(sources)'
  test:
    command: 'cargo test'
    inputs:
      - '@globs(sources)'
  start:
    command: 'cargo run --release'
    inputs:
      - '@globs(sources)'
```

### `.moon/tasks/typescript.yml`

```yaml
inheritedBy:
  toolchains: ['javascript', 'typescript']

tasks:
  build:
    command: 'bun run build'
  dev:
    command: 'bun run dev'
    options:
      persistent: true
      runInCI: false
  test:
    command: 'bun run typecheck'
  start:
    command: 'bun run start'
    options:
      persistent: true
```

### `simse-brand`

Excluded from task inheritance (no build/test/dev/start tasks) — static assets only.

## Toolchain Configuration

```yaml
# .moon/toolchains.yml
bun:
  version: 'latest'

rust: {}
```

Rust version pinned in existing `.prototools` file, which moon respects.

## Top-Level Commands

| Goal | Command | Effect |
|------|---------|--------|
| Build all | `moon run :build` | `cargo build --release` for Rust, `bun run build` for TS |
| Test all | `moon run :test` | `cargo test` for Rust, `bun run typecheck` for TS |
| Dev all | `moon run :dev` | `cargo build` (debug) for Rust, `bun run dev` for TS |
| Prod all | `moon run :start` | `cargo run --release` for Rust, `bun run start` for TS |

Targeted runs: `moon run :build --query "language=rust"` to build only Rust crates.
