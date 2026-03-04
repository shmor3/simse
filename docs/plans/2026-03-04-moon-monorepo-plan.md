# Moon Monorepo Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers-extended-cc:executing-plans to implement this plan task-by-task.

**Goal:** Integrate moon as the monorepo task runner with four top-level commands (`moon run :build`, `moon run :test`, `moon run :dev`, `moon run :start`) using glob-based project discovery — no hardcoded project lists.

**Architecture:** Moon discovers all `simse-*` directories via glob pattern. Task inheritance files in `.moon/tasks/` define build/test/dev/start commands scoped by toolchain (rust vs typescript). Each project has a minimal `moon.yml` declaring only language and tags.

**Tech Stack:** moon (monorepo tool), proto (version manager), bun (JS runtime), cargo (Rust build)

---

### Task 0: Install moon and initialize workspace

**Files:**
- Modify: `.prototools`
- Create: `.moon/workspace.yml`
- Modify: `.gitignore`

**Step 1: Add moon to .prototools**

```toml
rust = "latest"
moon = "latest"
```

**Step 2: Install moon via proto**

Run: `proto install moon`
Expected: moon binary installed successfully

**Step 3: Create .moon/workspace.yml**

```yaml
projects:
  - 'simse-*'
```

**Step 4: Add moon cache to .gitignore**

Append to `.gitignore`:
```
.moon/cache
.moon/docker
```

**Step 5: Verify moon initializes**

Run: `moon setup`
Expected: moon recognizes the workspace (projects won't be found yet — no moon.yml files)

**Step 6: Commit**

```bash
git add .prototools .moon/workspace.yml .gitignore
git commit -m "feat: initialize moon monorepo workspace with glob discovery"
```

---

### Task 1: Create .moon/toolchains.yml

**Files:**
- Create: `.moon/toolchains.yml`

**Step 1: Create toolchains config**

```yaml
bun:
  version: 'latest'

rust: {}
```

Rust version is already pinned in `.prototools`. Moon picks it up automatically.

**Step 2: Commit**

```bash
git add .moon/toolchains.yml
git commit -m "feat: add moon toolchain config for bun and rust"
```

---

### Task 2: Create .moon/tasks/rust.yml inherited tasks

**Files:**
- Create: `.moon/tasks/rust.yml`

**Step 1: Create the inherited tasks file**

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

**Step 2: Commit**

```bash
git add .moon/tasks/rust.yml
git commit -m "feat: add moon inherited tasks for Rust projects"
```

---

### Task 3: Create .moon/tasks/typescript.yml inherited tasks

**Files:**
- Create: `.moon/tasks/typescript.yml`

**Step 1: Create the inherited tasks file**

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

**Step 2: Commit**

```bash
git add .moon/tasks/typescript.yml
git commit -m "feat: add moon inherited tasks for TypeScript projects"
```

---

### Task 4: Create moon.yml for all 11 Rust packages

**Files:**
- Create: `simse-acp/moon.yml`
- Create: `simse-bridge/moon.yml`
- Create: `simse-core/moon.yml`
- Create: `simse-engine/moon.yml`
- Create: `simse-mcp/moon.yml`
- Create: `simse-tui/moon.yml`
- Create: `simse-ui-core/moon.yml`
- Create: `simse-vector/moon.yml`
- Create: `simse-vfs/moon.yml`
- Create: `simse-vnet/moon.yml`
- Create: `simse-vsh/moon.yml`

**Step 1: Create moon.yml for each Rust package**

All engine crates (`simse-acp`, `simse-bridge`, `simse-core`, `simse-engine`, `simse-mcp`, `simse-vector`, `simse-vfs`, `simse-vnet`, `simse-vsh`):

```yaml
language: 'rust'
tags: ['engine']
```

`simse-tui`:

```yaml
language: 'rust'
tags: ['app']
```

`simse-ui-core`:

```yaml
language: 'rust'
tags: ['lib']
```

**Step 2: Verify discovery**

Run: `moon project --list`
Expected: All 11 Rust projects listed

**Step 3: Commit**

```bash
git add simse-*/moon.yml
git commit -m "feat: add moon.yml for all Rust packages"
```

---

### Task 5: Create moon.yml for all 4 TypeScript packages

**Files:**
- Create: `simse-cloud/moon.yml`
- Create: `simse-code/moon.yml`
- Create: `simse-landing/moon.yml`
- Create: `simse-mailer/moon.yml`

**Step 1: Create moon.yml for standard TS packages**

`simse-cloud/moon.yml`:
```yaml
language: 'typescript'
tags: ['app']
```

`simse-landing/moon.yml`:
```yaml
language: 'typescript'
tags: ['app']
```

`simse-mailer/moon.yml`:
```yaml
language: 'typescript'
tags: ['app']
```

**Step 2: Create moon.yml for simse-code (special case)**

`simse-code` has no `dev` script, its test is `bun test` not `bun run typecheck`, and start is `bun run start`.

```yaml
language: 'typescript'
tags: ['app']

tasks:
  dev:
    command: 'bun run start'
    options:
      persistent: true
      runInCI: false
  test:
    command: 'bun test'
```

**Step 3: Verify discovery**

Run: `moon project --list`
Expected: All 16 projects listed (11 Rust + 4 TS + 1 brand)

**Step 4: Commit**

```bash
git add simse-cloud/moon.yml simse-code/moon.yml simse-landing/moon.yml simse-mailer/moon.yml
git commit -m "feat: add moon.yml for all TypeScript packages"
```

---

### Task 6: Create moon.yml for simse-brand (assets)

**Files:**
- Create: `simse-brand/moon.yml`

**Step 1: Create moon.yml excluding all tasks**

```yaml
language: 'other'
tags: ['assets']
```

Since `simse-brand` has `language: other`, it won't match any `inheritedBy` toolchain condition, so no tasks are inherited automatically.

**Step 2: Commit**

```bash
git add simse-brand/moon.yml
git commit -m "feat: add moon.yml for simse-brand assets package"
```

---

### Task 7: Verify all four moon commands work

**Step 1: Verify project discovery**

Run: `moon project --list`
Expected: All 16 projects listed with correct languages and tags

**Step 2: Verify build**

Run: `moon run :build`
Expected: All Rust crates build with `cargo build --release`, all TS packages run `bun run build`

**Step 3: Verify test**

Run: `moon run :test`
Expected: All Rust crates run `cargo test`, all TS packages run `bun run typecheck` (except simse-code which runs `bun test`)

**Step 4: Verify dev**

Run: `moon run :dev`
Expected: Rust crates run `cargo build` (debug), TS packages run `bun run dev`

**Step 5: Fix any issues found during verification**

**Step 6: Final commit if any fixes needed**

```bash
git commit -m "fix: resolve moon task execution issues"
```
