# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Structure

This is a monorepo using git submodules. Each submodule is a separate repository with its own README.md and CLAUDE.md — refer to those for detailed build commands, architecture, and module layouts.

```tree
simse-core/    # [submodule] Rust — orchestration + engine + adaptive + sandbox + remote
simse-cli/     # [submodule] Rust — terminal interface (ratatui, Elm Architecture)
simse-cloud/   # [submodule] TypeScript — Cloudflare Workers (9 nested service submodules)
simse-brand/   # [submodule] Brand assets
```

## Quick Commands

```bash
# Rust (see simse-core/CLAUDE.md and simse-cli/CLAUDE.md for details)
cd simse-core && cargo test       # All tests (default: all features enabled)
cd simse-cli && cargo test        # CLI tests

# TypeScript (see simse-cloud/CLAUDE.md for details)
cd simse-cloud/simse-cdn && npm run test
cd simse-cloud/simse-mailer && npm run test
```

## Key Facts

- **Single Rust crate**: `simse-core` uses feature flags (`engine`, `adaptive`, `sandbox`, `remote`) — not separate crates.
- **Submodule workflow**: Each submodule has its own git history. Commit and push within the submodule, then update the parent repo's submodule reference.
- **Formatting**: Rust uses `rustfmt` + `clippy -D warnings`. TypeScript uses Biome (tabs, single quotes, semicolons).
- **License**: Elastic License 2.0 (ELv2).
