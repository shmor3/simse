# Monorepo Breakup Design

**Date:** 2026-03-07
**Status:** Approved

## Goal

Break up the simse monorepo into individual private repos, create a cloud aggregator repo with service submodules, and restructure the main repo to use submodules.

## Repos to Create (all private, under shmor3/)

### Cloud service repos (9)
- simse-api, simse-app, simse-auth, simse-bi, simse-cdn
- simse-landing, simse-mailer, simse-payments, simse-status

### Standalone repos (3)
- simse-core (Rust library)
- simse-cli (Rust CLI)
- simse-brand (brand assets)

### Aggregator repo (1)
- simse-cloud (submodules only — the 9 cloud service repos)

## Process

For each folder with source code:
1. `git subtree split --prefix=simse-X -b split-simse-X` to extract history
2. `gh repo create shmor3/simse-X --private`
3. Push split branch to new remote
4. Remove folder from monorepo

For simse-cloud aggregator:
1. Create empty repo
2. Add 9 cloud service repos as submodules

## Final Main Repo Structure

```
simse/
  CLAUDE.md
  README.md
  LICENSE
  Cargo.toml          # workspace: simse-core, simse-cli
  deployment/
  docs/
  simse-core/         # submodule -> shmor3/simse-core
  simse-cli/          # submodule -> shmor3/simse-cli
  simse-brand/        # submodule -> shmor3/simse-brand
  simse-cloud/        # submodule -> shmor3/simse-cloud
```

## Cleanup

Delete stale folders: simse-cloud (current), simse-mcp, simse-analytics
