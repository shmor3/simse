# Embedder Consolidation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove TypeScript local-embedder and TEI bridge, port TEI bridge to Rust engine, support `tei://` prefix routing.

**Architecture:** The Rust engine already handles local Candle embedding. We add a `TeiEmbedder` that proxies to external TEI servers via HTTP. The `ModelRegistry` gains `tei://` prefix routing. On the TS side, we delete both embedders and their tests, and remove the `@huggingface/transformers` dependency.

**Tech Stack:** Rust (ureq for HTTP), TypeScript (deletions only)

---

### Task 1: Add ureq dependency to Cargo.toml

**Files:**
- Modify: `simse-engine/Cargo.toml`

**Step 1: Add ureq dependency**

Add under `[dependencies]`:
```toml
# HTTP client (for TEI bridge)
ureq = { version = "3", features = ["json"] }
```

**Step 2: Verify it compiles**

Run: `cd simse-engine && cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add simse-engine/Cargo.toml
git commit -m "chore(engine): add ureq dependency for TEI bridge"
```

---

### Task 2: Create TeiEmbedder in Rust

**Files:**
- Create: `simse-engine/src/models/tei.rs`
- Modify: `simse-engine/src/models/mod.rs` (add `pub mod tei;`)

**Step 1: Create `tei.rs`**

```rust
//! TEI (Text Embeddings Inference) bridge.
//!
//! Implements the `Embedder` trait by proxying requests to an external
//! Hugging Face Text Embeddings Inference server via HTTP.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{EmbedResult, Embedder};

/// Configuration for the TEI HTTP bridge.
#[derive(Debug, Clone)]
pub struct TeiConfig {
    /// Base URL of the TEI server (e.g., `http://localhost:8080`).
    pub base_url: String,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
    /// Whether to request normalized embeddings.
    pub normalize: bool,
    /// Whether to truncate inputs exceeding the model's max length.
    pub truncate: bool,
}

impl Default for TeiConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".to_string(),
            timeout_secs: 30,
            normalize: true,
            truncate: false,
        }
    }
}

/// Request body for the TEI `/embed` endpoint.
#[derive(Serialize)]
struct TeiEmbedRequest<'a> {
    inputs: &'a [String],
    normalize: bool,
    truncate: bool,
}

/// TEI embedder that proxies to an external TEI server.
pub struct TeiEmbedder {
    url: String,
    config: TeiConfig,
}

impl TeiEmbedder {
    /// Create a new TEI embedder with the given configuration.
    pub fn new(config: TeiConfig) -> Self {
        let url = format!("{}/embed", config.base_url.trim_end_matches('/'));
        Self { url, config }
    }
}

impl Embedder for TeiEmbedder {
    fn embed(&self, texts: &[String]) -> Result<EmbedResult> {
        if texts.is_empty() {
            return Ok(EmbedResult {
                embeddings: vec![],
                prompt_tokens: 0,
            });
        }

        tracing::debug!(
            batch_size = texts.len(),
            url = %self.url,
            "Sending embedding request to TEI server"
        );

        let body = TeiEmbedRequest {
            inputs: texts,
            normalize: self.config.normalize,
            truncate: self.config.truncate,
        };

        let response = ureq::post(&self.url)
            .header("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(self.config.timeout_secs))
            .send_json(&body)
            .map_err(|e| anyhow::anyhow!("TEI request failed: {}", e))?;

        let embeddings: Vec<Vec<f32>> = response.body_mut()
            .read_json()
            .map_err(|e| anyhow::anyhow!("TEI response parse error: {}", e))?;

        // Estimate prompt tokens from input character count (rough approximation)
        let prompt_tokens: u64 = texts.iter().map(|t| (t.len() / 4) as u64).sum();

        tracing::debug!(
            batch_size = texts.len(),
            embedding_dim = embeddings.first().map_or(0, |e| e.len()),
            "TEI embeddings received"
        );

        Ok(EmbedResult {
            embeddings,
            prompt_tokens,
        })
    }
}
```

**Step 2: Add module declaration to `mod.rs`**

In `simse-engine/src/models/mod.rs`, add after the existing module declarations:

```rust
pub mod tei;
```

**Step 3: Verify it compiles**

Run: `cd simse-engine && cargo check`
Expected: compiles with no errors

**Step 4: Commit**

```bash
git add simse-engine/src/models/tei.rs simse-engine/src/models/mod.rs
git commit -m "feat(engine): add TEI bridge embedder"
```

---

### Task 3: Add `--tei-url` CLI flag and `tei://` routing

**Files:**
- Modify: `simse-engine/src/config.rs`
- Modify: `simse-engine/src/models/mod.rs`
- Modify: `simse-engine/src/main.rs`
- Modify: `simse-engine/src/server.rs`

**Step 1: Add CLI flag to `config.rs`**

Add to `CliArgs` struct after the `embedding_model` field:

```rust
    /// TEI server URL for remote embeddings (e.g., http://localhost:8080)
    #[arg(long, env = "SIMSE_ENGINE_TEI_URL")]
    pub tei_url: Option<String>,
```

**Step 2: Add `load_tei_embedder` and `tei://` routing to `ModelRegistry` in `mod.rs`**

Add import at top:
```rust
use self::tei::{TeiConfig, TeiEmbedder};
```

Add method to `impl ModelRegistry`:
```rust
    /// Load a TEI bridge embedder.
    pub fn load_tei_embedder(&mut self, key: &str, config: TeiConfig) -> Result<()> {
        tracing::info!(key, url = %config.base_url, "Loading TEI bridge embedder");
        let embedder = TeiEmbedder::new(config);
        self.embedders.insert(key.to_string(), Box::new(embedder));
        Ok(())
    }
```

Modify `get_embedder` to handle `tei://` prefix:
```rust
    /// Get a reference to an embedder.
    /// Supports `tei://` prefix to select TEI embedder (maps to `tei://default` key).
    pub fn get_embedder(&self, model_id: &str) -> Option<&dyn Embedder> {
        if model_id.starts_with("tei://") {
            // Route tei:// prefixed IDs to the TEI embedder
            let key = if model_id == "tei://" { "tei://default" } else { model_id };
            return self.embedders.get(key).map(|e| &**e as &dyn Embedder);
        }
        self.embedders.get(model_id).map(|e| &**e as &dyn Embedder)
    }
```

**Step 3: Wire TEI loading in `main.rs`**

After the existing `registry.load_embedder(...)` block, add:

```rust
    // Load TEI bridge if configured
    if let Some(ref tei_url) = args.tei_url {
        tracing::info!(url = %tei_url, "Loading TEI bridge embedder");
        registry.load_tei_embedder(
            "tei://default",
            simse_engine::models::tei::TeiConfig {
                base_url: tei_url.clone(),
                ..Default::default()
            },
        )?;
    }
```

**Step 4: Pass TEI URL through to `ServerConfig` in `server.rs`**

Add field to `ServerConfig`:
```rust
    pub tei_url: Option<String>,
```

In `main.rs`, add to config construction:
```rust
        tei_url: args.tei_url.clone(),
```

**Step 5: Route `tei://` in `handle_session_prompt`**

In `server.rs` `handle_session_prompt`, modify the embedding dispatch to support `tei://`:

Replace the embedding branch:
```rust
        if Self::is_embed_request(&prompt_params.prompt) {
            let texts = Self::extract_embed_texts(&prompt_params.prompt);
            // Check if prompt requests TEI specifically via model metadata
            let embed_model = prompt_params.metadata
                .as_ref()
                .and_then(|m| m.get("model"))
                .and_then(|m| m.as_str())
                .unwrap_or(&self.config.embedding_model);

            let result = inference::embedding::run_embedding(
                &self.registry,
                embed_model,
                &texts,
            )?;
            self.transport.write_response(id, serde_json::to_value(result)?);
        }
```

**Step 6: Add TEI model to available_models when loaded**

In `ModelRegistry::available_models()`, the TEI embedder is already included since it's stored in the `embedders` HashMap. No change needed.

**Step 7: Verify it compiles**

Run: `cd simse-engine && cargo check`
Expected: compiles with no errors

**Step 8: Commit**

```bash
git add simse-engine/src/config.rs simse-engine/src/models/mod.rs simse-engine/src/main.rs simse-engine/src/server.rs
git commit -m "feat(engine): add --tei-url flag and tei:// routing"
```

---

### Task 4: Delete TypeScript embedder files

**Files:**
- Delete: `src/ai/acp/local-embedder.ts`
- Delete: `src/ai/acp/tei-bridge.ts`
- Delete: `tests/local-embedder.test.ts`
- Delete: `tests/tei-bridge.test.ts`
- Delete: `tests/e2e-local-embedder.test.ts`

**Step 1: Delete the files**

```bash
rm src/ai/acp/local-embedder.ts
rm src/ai/acp/tei-bridge.ts
rm tests/local-embedder.test.ts
rm tests/tei-bridge.test.ts
rm tests/e2e-local-embedder.test.ts
```

**Step 2: Commit**

```bash
git add -u src/ai/acp/local-embedder.ts src/ai/acp/tei-bridge.ts tests/local-embedder.test.ts tests/tei-bridge.test.ts tests/e2e-local-embedder.test.ts
git commit -m "feat: remove TypeScript local-embedder and tei-bridge"
```

---

### Task 5: Update `src/lib.ts` exports

**Files:**
- Modify: `src/lib.ts:36-39`

**Step 1: Remove the 4 export lines**

Remove these lines from `src/lib.ts`:
```typescript
export type { LocalEmbedderOptions } from './ai/acp/local-embedder.js';
export { createLocalEmbedder } from './ai/acp/local-embedder.js';
export type { TEIEmbedderOptions } from './ai/acp/tei-bridge.js';
export { createTEIEmbedder } from './ai/acp/tei-bridge.js';
```

**Step 2: Run typecheck**

Run: `bun run typecheck`
Expected: no errors

**Step 3: Commit**

```bash
git add src/lib.ts
git commit -m "feat: remove local-embedder and tei-bridge from public API"
```

---

### Task 6: Rewrite e2e-library-pipeline test

**Files:**
- Modify: `tests/e2e-library-pipeline.test.ts`

**Step 1: Replace `createLocalEmbedder` with a deterministic mock embedder**

The test currently uses a real ONNX model which takes 120s timeout. Replace with a simple hash-based mock that produces consistent embeddings for semantic testing.

```typescript
/**
 * E2E test: Full library pipeline — embed → store → search → dedup.
 *
 * Uses a deterministic mock embedder that produces consistent vectors
 * from text content for testing library semantics.
 */
import { describe, expect, it } from 'bun:test';
import type { Buffer } from 'node:buffer';
import { createLibrary } from '../src/ai/library/library.js';
import type { StorageBackend } from '../src/ai/library/storage.js';
import type { EmbeddingProvider } from '../src/ai/library/types.js';

// ---------------------------------------------------------------------------
// In-memory storage backend for tests
// ---------------------------------------------------------------------------

function createMemoryStorage(): StorageBackend {
	const data = new Map<string, Buffer>();
	return Object.freeze({
		load: async () => new Map(data),
		save: async (snapshot: Map<string, Buffer>) => {
			data.clear();
			for (const [k, v] of snapshot) data.set(k, v);
		},
		close: async () => {},
	});
}

// ---------------------------------------------------------------------------
// Deterministic mock embedder
//
// Produces 64-dimensional vectors seeded from text content.
// Similar texts produce similar vectors (word overlap → dimension overlap).
// ---------------------------------------------------------------------------

function createMockEmbedder(): EmbeddingProvider {
	const DIM = 64;

	function hashText(text: string): number[] {
		const vec = new Array(DIM).fill(0);
		const words = text.toLowerCase().split(/\s+/);
		for (const word of words) {
			// Use char codes to seed dimensions deterministically
			for (let i = 0; i < word.length; i++) {
				const idx = (word.charCodeAt(i) * 7 + i * 13) % DIM;
				vec[idx] += 1;
			}
		}
		// L2 normalize
		const mag = Math.sqrt(vec.reduce((s: number, v: number) => s + v * v, 0)) || 1;
		return vec.map((v: number) => v / mag);
	}

	return Object.freeze({
		embed: async (input: string | readonly string[]) => {
			const texts = typeof input === 'string' ? [input] : [...input];
			return { embeddings: texts.map(hashText) };
		},
	});
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Library pipeline E2E', () => {
	const embedder = createMockEmbedder();

	it('adds volumes and searches semantically', async () => {
		const library = createLibrary(
			embedder,
			{},
			{
				storage: createMemoryStorage(),
			},
		);
		await library.initialize();

		await library.add('TypeScript is a typed superset of JavaScript', {
			topic: 'programming',
		});
		await library.add('Python is great for data science and machine learning', {
			topic: 'programming',
		});
		await library.add('The weather in London is often rainy and cold', {
			topic: 'weather',
		});

		const results = await library.search(
			'What programming languages are useful?',
			3,
			0.0,
		);

		expect(results.length).toBeGreaterThanOrEqual(2);

		// Programming results should rank above weather
		const topics = results.map((r) => r.volume.metadata?.topic);
		expect(topics[0]).toBe('programming');
		expect(topics[1]).toBe('programming');
	});

	it('detects near-duplicate text', async () => {
		const library = createLibrary(
			embedder,
			{ duplicateThreshold: 0.9 },
			{ storage: createMemoryStorage() },
		);
		await library.initialize();

		await library.add('TypeScript is a typed superset of JavaScript');

		const dupeResult = await library.checkDuplicate(
			'TypeScript is a typed superset of JavaScript language',
		);
		expect(dupeResult.isDuplicate).toBe(true);
		expect(dupeResult.similarity).toBeGreaterThan(0.8);
	});

	it('stores and retrieves by topic', async () => {
		const library = createLibrary(
			embedder,
			{},
			{
				storage: createMemoryStorage(),
			},
		);
		await library.initialize();

		await library.add('React is a UI library', { topic: 'frontend' });
		await library.add('Express is a Node framework', { topic: 'backend' });
		await library.add('Vue is another UI framework', { topic: 'frontend' });

		const frontend = library.filterByTopic(['frontend']);
		expect(frontend).toHaveLength(2);

		const backend = library.filterByTopic(['backend']);
		expect(backend).toHaveLength(1);

		const topics = library.getTopics();
		expect(topics.length).toBe(2);
	});
});
```

**Step 2: Run the test**

Run: `bun test tests/e2e-library-pipeline.test.ts`
Expected: all 3 tests pass (no 120s timeouts now)

**Step 3: Commit**

```bash
git add tests/e2e-library-pipeline.test.ts
git commit -m "test: replace ONNX embedder with mock in library pipeline test"
```

---

### Task 7: Remove `@huggingface/transformers` from package.json

**Files:**
- Modify: `package.json`

**Step 1: Remove from optionalDependencies**

Remove the entire `optionalDependencies` block:
```json
	"optionalDependencies": {
		"@huggingface/transformers": "^3.8.1"
	},
```

Also remove `"onnxruntime-node"` from `trustedDependencies` (it was only needed for `@huggingface/transformers`):
```json
	"trustedDependencies": [
		"node-pty",
		"protobufjs"
	]
```

**Step 2: Run full verification**

Run: `bun run typecheck && bun run lint && bun test`
Expected: 0 typecheck errors, 0 lint errors, all tests pass

**Step 3: Commit**

```bash
git add package.json
git commit -m "chore: remove @huggingface/transformers dependency"
```

---

### Task 8: Final verification

**Step 1: Rust engine compiles**

Run: `cd simse-engine && cargo check`
Expected: no errors

**Step 2: TypeScript checks pass**

Run: `bun run typecheck`
Expected: no errors

**Step 3: Lint is clean**

Run: `bun run lint`
Expected: no errors

**Step 4: All tests pass**

Run: `bun test`
Expected: all tests pass, test count drops by ~25 (removed embedder tests)

**Step 5: Verify no remaining imports**

Run: `grep -r "local-embedder\|tei-bridge\|createLocalEmbedder\|createTEIEmbedder\|LocalEmbedderOptions\|TEIEmbedderOptions" src/ tests/ --include="*.ts" --include="*.tsx"`
Expected: no matches (only docs/plans may reference these)
