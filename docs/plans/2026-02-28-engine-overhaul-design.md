# simse-engine Full Overhaul Design

**Date:** 2026-02-28

## Goal

Make the Rust engine production-grade: fix all critical safety issues, add proper randomness, implement missing ACP protocol features, add tests, improve documentation and code quality.

## Batches

### Batch 1: Critical Safety Fixes
- Replace `.unwrap()` panics in `server.rs` with `match` + error response
- Fix JSON-RPC notification handling: don't respond to messages without `id`
- Add configurable generation timeout (`--generation-timeout`, default 300s)
- Fix stop sequence handling: check after each token emission, not at end
- Add input validation: temperature (0.0-10.0), max_tokens (1-context_size), batch size limits

### Batch 2: Proper PRNG + Sampling
- Add `rand` crate dependency
- Replace `SystemTime`-based PRNG with `rand::thread_rng()`
- Add epsilon to softmax to prevent NaN on zero logits

### Batch 3: Protocol Compliance
- Implement `session/delete` method
- Add conversation history per session (Vec of messages, multi-turn)
- Enrich response metadata: `model_id`, `generated_at`, proper `stop_reason`
- Move embed detection from content-sniffing to metadata `action` field (keep content-sniffing as fallback)

### Batch 4: Robustness + Documentation
- Document unsafe blocks with SAFETY comments
- Add file size check before model loading (10GB default max)
- Improve error messages with source context
- Add `///` doc comments on all public types and methods
- Extract hardcoded values to named constants

### Batch 5: Unit Tests
- Server: `is_embed_request`, `extract_embed_texts`, `extract_text_from_content`, `extract_sampling_params`
- Sampling: temperature scaling, top-p/top-k filtering, stop sequences
- TEI: config defaults, URL construction
- Registry: load, get, `tei://` routing, `available_models`

### Batch 6: Code Quality
- Replace `Box::leak` with `OnceLock` for stdout
- Reduce clones: `Arc<str>` for session IDs and model names
- Consistent logging levels (debug for startup, info for runtime, warn for recoverable)
- Remove dead code / unused imports
