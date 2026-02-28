# simse-vector Extraction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract the entire library/memory/vector system from `src/ai/library/` into a standalone `simse-vector/` package with its own error layer, logger interface, and provider interfaces — zero imports from simse.

**Architecture:** Create `simse-vector/` at repo root with its own `package.json`, `tsconfig.json`, and Biome config. The package defines its own `VectorError` base, Logger interface, and provider interfaces. The main simse package depends on simse-vector and re-exports everything. All 26 library source files and 7 test files move.

**Tech Stack:** TypeScript, Bun (build + test), Biome (lint), picomatch (glob matching in librarian-definition)

---

### Task 1: Scaffold simse-vector package

**Files:**
- Create: `simse-vector/package.json`
- Create: `simse-vector/tsconfig.json`
- Create: `simse-vector/biome.json`

**Step 1: Create package.json**

Create `simse-vector/package.json`:

```json
{
	"name": "simse-vector",
	"version": "1.0.0",
	"type": "module",
	"main": "dist/lib.js",
	"types": "dist/lib.d.ts",
	"exports": {
		".": {
			"bun": "./src/lib.ts",
			"import": "./dist/lib.js",
			"types": "./dist/lib.d.ts"
		}
	},
	"engines": {
		"bun": ">=1.0.0"
	},
	"scripts": {
		"build": "bun build ./src/lib.ts --outdir ./dist --target bun && tsc --project tsconfig.build.json",
		"test": "bun test",
		"lint": "biome check .",
		"lint:fix": "biome check --write .",
		"typecheck": "bun x tsc --noEmit"
	},
	"dependencies": {
		"picomatch": "^4.0.3"
	},
	"devDependencies": {
		"@types/bun": "latest",
		"@types/picomatch": "^4.0.2",
		"typescript": "^5.7.0"
	}
}
```

**Step 2: Create tsconfig.json**

Create `simse-vector/tsconfig.json`:

```json
{
	"compilerOptions": {
		"lib": ["ESNext"],
		"target": "ESNext",
		"module": "ESNext",
		"moduleDetection": "force",
		"moduleResolution": "bundler",
		"allowImportingTsExtensions": true,
		"verbatimModuleSyntax": true,
		"noEmit": true,
		"strict": true,
		"skipLibCheck": true,
		"noFallthroughCasesInSwitch": true,
		"noUnusedLocals": true,
		"noUnusedParameters": true,
		"noPropertyAccessFromIndexSignature": false,
		"forceConsistentCasingInFileNames": true,
		"resolveJsonModule": true,
		"types": ["bun-types"]
	},
	"include": ["src/**/*"],
	"exclude": ["node_modules", "dist"]
}
```

**Step 3: Create biome.json**

Create `simse-vector/biome.json`:

```json
{
	"$schema": "https://biomejs.dev/schemas/2.3.12/schema.json",
	"vcs": {
		"enabled": true,
		"clientKind": "git",
		"useIgnoreFile": true
	},
	"formatter": {
		"enabled": true,
		"indentStyle": "tab"
	},
	"linter": {
		"enabled": true,
		"rules": {
			"recommended": true
		}
	},
	"javascript": {
		"formatter": {
			"quoteStyle": "single"
		}
	},
	"assist": {
		"enabled": true,
		"actions": {
			"source": {
				"organizeImports": "on"
			}
		}
	}
}
```

**Step 4: Install dependencies**

Run: `cd simse-vector && bun install`
Expected: Lockfile created, node_modules populated.

**Step 5: Commit**

```bash
git add simse-vector/package.json simse-vector/tsconfig.json simse-vector/biome.json simse-vector/bun.lock
git commit -m "feat(simse-vector): scaffold package with build config"
```

---

### Task 2: Create self-contained error layer

The library currently imports error factories from `../../errors/index.js`. We need a self-contained error module inside simse-vector with the same API surface.

**Files:**
- Create: `simse-vector/src/errors.ts`

**Step 1: Create errors.ts**

Create `simse-vector/src/errors.ts` with:

```typescript
// ---------------------------------------------------------------------------
// VectorError — self-contained error hierarchy for simse-vector
// ---------------------------------------------------------------------------

/**
 * Structured error interface for simse-vector.
 * Compatible with simse's SimseError shape for seamless integration.
 */
export interface VectorError extends Error {
	readonly code: string;
	readonly statusCode: number;
	readonly metadata: Record<string, unknown>;
	readonly toJSON: () => Record<string, unknown>;
}

export interface VectorErrorOptions {
	readonly name?: string;
	readonly code?: string;
	readonly statusCode?: number;
	readonly cause?: unknown;
	readonly metadata?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------
// Base factory
// ---------------------------------------------------------------------------

export const createVectorError = (
	message: string,
	options: VectorErrorOptions = {},
): VectorError => {
	const err = new Error(message, { cause: options.cause }) as Error & {
		code: string;
		statusCode: number;
		metadata: Record<string, unknown>;
		toJSON: () => Record<string, unknown>;
	};

	err.name = options.name ?? 'VectorError';
	const code = options.code ?? 'VECTOR_ERROR';
	const statusCode = options.statusCode ?? 500;
	const metadata = options.metadata ?? {};

	Object.defineProperties(err, {
		code: { value: code, writable: false, enumerable: true },
		statusCode: { value: statusCode, writable: false, enumerable: true },
		metadata: { value: metadata, writable: false, enumerable: true },
		toJSON: {
			value: (): Record<string, unknown> => ({
				name: err.name,
				code,
				message: err.message,
				statusCode,
				metadata,
				cause:
					err.cause &&
					typeof err.cause === 'object' &&
					typeof (err.cause as Record<string, unknown>).message === 'string' &&
					typeof (err.cause as Record<string, unknown>).name === 'string'
						? {
								name: (err.cause as Record<string, unknown>).name,
								message: (err.cause as Record<string, unknown>).message,
							}
						: err.cause,
				stack: err.stack,
			}),
			writable: false,
			enumerable: false,
		},
	});

	return err as VectorError;
};

// ---------------------------------------------------------------------------
// Base type guard
// ---------------------------------------------------------------------------

export const isVectorError = (value: unknown): value is VectorError =>
	value instanceof Error &&
	typeof (value as unknown as Record<string, unknown>).code === 'string' &&
	typeof (value as unknown as Record<string, unknown>).statusCode ===
		'number' &&
	typeof (value as unknown as Record<string, unknown>).toJSON === 'function';

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

export const toError = (value: unknown): Error => {
	if (value instanceof Error) return value;
	if (typeof value === 'string') return new Error(value);
	return new Error(String(value));
};

// ---------------------------------------------------------------------------
// Library errors
// ---------------------------------------------------------------------------

export const createLibraryError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): VectorError =>
	createVectorError(message, {
		name: options.name ?? 'LibraryError',
		code: options.code ?? 'LIBRARY_ERROR',
		statusCode: 500,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createEmbeddingError = (
	message: string,
	options: { cause?: unknown; model?: string } = {},
): VectorError =>
	createLibraryError(message, {
		name: 'EmbeddingError',
		code: 'EMBEDDING_ERROR',
		cause: options.cause,
		metadata: options.model ? { model: options.model } : {},
	});

export const createStacksCorruptionError = (
	storePath: string,
	options: { cause?: unknown } = {},
): VectorError & { readonly storePath: string } => {
	const err = createLibraryError(`Stacks file is corrupted: ${storePath}`, {
		name: 'StacksCorruptionError',
		code: 'STACKS_CORRUPT',
		cause: options.cause,
		metadata: { storePath },
	}) as VectorError & { readonly storePath: string };

	Object.defineProperty(err, 'storePath', {
		value: storePath,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createStacksIOError = (
	storePath: string,
	operation: 'read' | 'write',
	options: { cause?: unknown } = {},
): VectorError & { readonly storePath: string } => {
	const err = createLibraryError(
		`Failed to ${operation} stacks: ${storePath}`,
		{
			name: 'StacksIOError',
			code: 'STACKS_IO',
			cause: options.cause,
			metadata: { storePath, operation },
		},
	) as VectorError & { readonly storePath: string };

	Object.defineProperty(err, 'storePath', {
		value: storePath,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createStacksError = (
	message: string,
	options: {
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): VectorError =>
	createLibraryError(message, {
		name: 'StacksError',
		code: options.code ?? 'STACKS_ERROR',
		cause: options.cause,
		metadata: options.metadata,
	});

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isLibraryError = (value: unknown): value is VectorError =>
	isVectorError(value) &&
	(value.code.startsWith('LIBRARY_') ||
		value.code.startsWith('EMBEDDING_') ||
		value.code.startsWith('STACKS_'));

export const isStacksError = (value: unknown): value is VectorError =>
	isVectorError(value) && value.code.startsWith('STACKS_');

export const isEmbeddingError = (value: unknown): value is VectorError =>
	isVectorError(value) && value.code === 'EMBEDDING_ERROR';

export const isStacksCorruptionError = (
	value: unknown,
): value is VectorError & { readonly storePath: string } =>
	isVectorError(value) && value.code === 'STACKS_CORRUPT';

export const isStacksIOError = (
	value: unknown,
): value is VectorError & { readonly storePath: string } =>
	isVectorError(value) && value.code === 'STACKS_IO';
```

**Step 2: Verify typecheck**

Run: `cd simse-vector && bun x tsc --noEmit`

This will fail because there's no `lib.ts` yet — that's fine, but `errors.ts` itself should have no type errors. Create a minimal `src/lib.ts` stub:

```typescript
export * from './errors.js';
```

Run: `cd simse-vector && bun x tsc --noEmit`
Expected: PASS (0 errors)

**Step 3: Commit**

```bash
git add simse-vector/src/errors.ts simse-vector/src/lib.ts
git commit -m "feat(simse-vector): add self-contained error layer"
```

---

### Task 3: Create Logger and EventBus interfaces

The library imports `Logger` from `../../logger.js` and `EventBus` from `../../events/types.js`. simse-vector needs its own compatible interfaces.

**Files:**
- Create: `simse-vector/src/logger.ts`

**Step 1: Create logger.ts**

Create `simse-vector/src/logger.ts`:

```typescript
// ---------------------------------------------------------------------------
// Logger — interface for simse-vector
// ---------------------------------------------------------------------------

/**
 * Minimal logger interface.
 * Compatible with simse's Logger for seamless integration.
 */
export interface Logger {
	readonly debug: (
		message: string,
		metadata?: Readonly<Record<string, unknown>>,
	) => void;
	readonly info: (
		message: string,
		metadata?: Readonly<Record<string, unknown>>,
	) => void;
	readonly warn: (
		message: string,
		metadata?: Readonly<Record<string, unknown>>,
	) => void;
	readonly error: (
		message: string,
		errorOrMetadata?: Error | Readonly<Record<string, unknown>>,
	) => void;
	readonly child: (childContext: string) => Logger;
}

// ---------------------------------------------------------------------------
// No-op logger (default when none is provided)
// ---------------------------------------------------------------------------

const noop = (): void => {};

export const createNoopLogger = (): Logger =>
	Object.freeze({
		debug: noop,
		info: noop,
		warn: noop,
		error: noop,
		child: () => createNoopLogger(),
	});

// ---------------------------------------------------------------------------
// EventBus — minimal interface for library event publishing
// ---------------------------------------------------------------------------

/**
 * Library-specific event types.
 */
export interface LibraryEventMap {
	'library.shelve': { readonly id: string; readonly contentLength: number };
	'library.search': {
		readonly query: string;
		readonly resultCount: number;
		readonly durationMs: number;
	};
	'library.withdraw': { readonly id: string };
}

export type LibraryEventType = keyof LibraryEventMap;

/**
 * Minimal event bus interface.
 * simse passes its full EventBus which is a superset of this.
 */
export interface EventBus {
	readonly publish: <T extends string>(
		type: T,
		payload: unknown,
	) => void;
}
```

**Step 2: Update lib.ts**

Add export to `simse-vector/src/lib.ts`:

```typescript
export * from './errors.js';
export * from './logger.js';
```

**Step 3: Verify typecheck**

Run: `cd simse-vector && bun x tsc --noEmit`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-vector/src/logger.ts simse-vector/src/lib.ts
git commit -m "feat(simse-vector): add Logger and EventBus interfaces"
```

---

### Task 4: Move types.ts and pure math modules

Move the foundational files that have zero or minimal internal dependencies.

**Files:**
- Move: `src/ai/library/types.ts` → `simse-vector/src/types.ts`
- Move: `src/ai/library/cosine.ts` → `simse-vector/src/cosine.ts`
- Move: `src/ai/library/preservation.ts` → `simse-vector/src/preservation.ts`
- Move: `src/ai/library/storage.ts` → `simse-vector/src/storage.ts`

**Step 1: Copy files**

```bash
cp src/ai/library/types.ts simse-vector/src/types.ts
cp src/ai/library/cosine.ts simse-vector/src/cosine.ts
cp src/ai/library/preservation.ts simse-vector/src/preservation.ts
cp src/ai/library/storage.ts simse-vector/src/storage.ts
```

**Step 2: Fix imports in types.ts**

`types.ts` has no external imports — it's pure type definitions. No changes needed.

**Step 3: Fix imports in preservation.ts**

`preservation.ts` only imports from `node:buffer` and `node:zlib`. No changes needed.

**Step 4: Update lib.ts**

Update `simse-vector/src/lib.ts` to export everything from the new modules. This should mirror the exports that the main `src/lib.ts` currently has for these files. Build up the full barrel export as files are moved.

**Step 5: Verify typecheck**

Run: `cd simse-vector && bun x tsc --noEmit`
Expected: PASS

**Step 6: Commit**

```bash
git add simse-vector/src/types.ts simse-vector/src/cosine.ts simse-vector/src/preservation.ts simse-vector/src/storage.ts simse-vector/src/lib.ts
git commit -m "feat(simse-vector): move types, cosine, preservation, storage"
```

---

### Task 5: Move search, indexing, and cataloging modules

Move all pure search/indexing files. These only import from sibling files already in simse-vector.

**Files:**
- Move: `src/ai/library/text-search.ts` → `simse-vector/src/text-search.ts`
- Move: `src/ai/library/text-cache.ts` → `simse-vector/src/text-cache.ts`
- Move: `src/ai/library/inverted-index.ts` → `simse-vector/src/inverted-index.ts`
- Move: `src/ai/library/cataloging.ts` → `simse-vector/src/cataloging.ts`
- Move: `src/ai/library/query-dsl.ts` → `simse-vector/src/query-dsl.ts`
- Move: `src/ai/library/deduplication.ts` → `simse-vector/src/deduplication.ts`
- Move: `src/ai/library/recommendation.ts` → `simse-vector/src/recommendation.ts`
- Move: `src/ai/library/prompt-injection.ts` → `simse-vector/src/prompt-injection.ts`
- Move: `src/ai/library/patron-learning.ts` → `simse-vector/src/patron-learning.ts`
- Move: `src/ai/library/topic-catalog.ts` → `simse-vector/src/topic-catalog.ts`

**Step 1: Copy all files**

```bash
for f in text-search text-cache inverted-index cataloging query-dsl deduplication recommendation prompt-injection patron-learning topic-catalog; do
  cp "src/ai/library/${f}.ts" "simse-vector/src/${f}.ts"
done
```

**Step 2: Fix imports**

These files only import from each other and from `types.ts` — all already in `simse-vector/src/`. The relative import paths stay the same (e.g., `./types.js`, `./cosine.js`). No changes needed.

**Step 3: Update lib.ts barrel exports**

**Step 4: Verify typecheck**

Run: `cd simse-vector && bun x tsc --noEmit`
Expected: PASS

**Step 5: Commit**

```bash
git add simse-vector/src/
git commit -m "feat(simse-vector): move search, indexing, cataloging, and utility modules"
```

---

### Task 6: Move stacks modules

Move the stacks (file-backed storage) layer. These import from errors and logger — point them to the simse-vector versions.

**Files:**
- Move: `src/ai/library/stacks-persistence.ts` → `simse-vector/src/stacks-persistence.ts`
- Move: `src/ai/library/stacks-serialize.ts` → `simse-vector/src/stacks-serialize.ts`
- Move: `src/ai/library/stacks-search.ts` → `simse-vector/src/stacks-search.ts`
- Move: `src/ai/library/stacks-recommend.ts` → `simse-vector/src/stacks-recommend.ts`
- Move: `src/ai/library/stacks.ts` → `simse-vector/src/stacks.ts`

**Step 1: Copy files**

```bash
for f in stacks-persistence stacks-serialize stacks-search stacks-recommend stacks; do
  cp "src/ai/library/${f}.ts" "simse-vector/src/${f}.ts"
done
```

**Step 2: Fix imports in stacks.ts**

Replace:
```typescript
import { createLibraryError, createStacksCorruptionError } from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
```
With:
```typescript
import { createLibraryError, createStacksCorruptionError } from './errors.js';
import type { Logger } from './logger.js';
```

Remove all `getDefaultLogger` usages — the Logger must be passed in via options. If stacks currently uses `getDefaultLogger()` as a fallback when no logger is passed, replace with `createNoopLogger()` from `./logger.js`.

**Step 3: Fix imports in stacks-persistence.ts**

If it imports from `../../errors/`, change to `./errors.js`.

**Step 4: Fix imports in stacks-serialize.ts**

If it imports from `../../errors/`, change to `./errors.js`.

**Step 5: Verify typecheck**

Run: `cd simse-vector && bun x tsc --noEmit`
Expected: PASS

**Step 6: Commit**

```bash
git add simse-vector/src/stacks*.ts
git commit -m "feat(simse-vector): move stacks storage layer with self-contained errors"
```

---

### Task 7: Move shelf and library modules

Move the high-level Library factory and Shelf.

**Files:**
- Move: `src/ai/library/shelf.ts` → `simse-vector/src/shelf.ts`
- Move: `src/ai/library/library.ts` → `simse-vector/src/library.ts`

**Step 1: Copy files**

```bash
cp src/ai/library/shelf.ts simse-vector/src/shelf.ts
cp src/ai/library/library.ts simse-vector/src/library.ts
```

**Step 2: Fix imports in library.ts**

Replace:
```typescript
import { createEmbeddingError, createLibraryError, isEmbeddingError, toError } from '../../errors/index.js';
import type { EventBus } from '../../events/types.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
```
With:
```typescript
import { createEmbeddingError, createLibraryError, isEmbeddingError, toError } from './errors.js';
import type { EventBus } from './logger.js';
import type { Logger } from './logger.js';
```

Replace any `getDefaultLogger()` fallback with `createNoopLogger()`:
```typescript
import { createNoopLogger, type Logger } from './logger.js';
```

And in the factory: `const log = options.logger ?? createNoopLogger();`

**Step 3: Verify typecheck**

Run: `cd simse-vector && bun x tsc --noEmit`
Expected: PASS

**Step 4: Commit**

```bash
git add simse-vector/src/shelf.ts simse-vector/src/library.ts
git commit -m "feat(simse-vector): move library and shelf"
```

---

### Task 8: Move librarian, circulation-desk, and library-services

These have the most external dependencies. `librarian.ts` imports from ACP — we need to remove that and make the TextGenerationProvider a constructor parameter only.

**Files:**
- Move: `src/ai/library/librarian.ts` → `simse-vector/src/librarian.ts`
- Move: `src/ai/library/librarian-definition.ts` → `simse-vector/src/librarian-definition.ts`
- Move: `src/ai/library/librarian-registry.ts` → `simse-vector/src/librarian-registry.ts`
- Move: `src/ai/library/circulation-desk.ts` → `simse-vector/src/circulation-desk.ts`
- Move: `src/ai/library/library-services.ts` → `simse-vector/src/library-services.ts`

**Step 1: Copy files**

```bash
for f in librarian librarian-definition librarian-registry circulation-desk library-services; do
  cp "src/ai/library/${f}.ts" "simse-vector/src/${f}.ts"
done
```

**Step 2: Fix librarian.ts**

This is the critical one. Currently imports:
```typescript
import { createACPGenerator } from '../acp/acp-adapters.js';
import type { ACPClient } from '../acp/acp-client.js';
```

Remove both imports. The `createLibrarian` factory already accepts a `TextGenerationProvider` — that's the interface. The `createACPGenerator` call that wraps ACPClient into a TextGenerationProvider should move to the simse side (in `acp-adapters.ts` or where librarians are constructed).

If `librarian.ts` has a `createDefaultLibrarian(acpClient)` convenience function that calls `createACPGenerator` internally, either:
- Remove it from simse-vector (simse provides its own wrapper), or
- Change it to accept `TextGenerationProvider` instead of `ACPClient`

**Step 3: Fix librarian-registry.ts**

Replace:
```typescript
import { toError } from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import type { ACPConnection } from '../acp/acp-connection.js';
```
With:
```typescript
import { toError } from './errors.js';
import { createNoopLogger, type Logger } from './logger.js';
```

Remove `ACPConnection` import. If the registry uses ACPConnection to create librarians with ACP adapters, refactor so the caller passes pre-constructed `TextGenerationProvider` instances instead. The registry should only depend on `TextGenerationProvider` from `types.ts`.

**Step 4: Fix library-services.ts**

Replace:
```typescript
import type { Logger } from '../../logger.js';
```
With:
```typescript
import type { Logger } from './logger.js';
```

**Step 5: Fix librarian-definition.ts**

This file only imports `picomatch` (npm) and `types.ts` (local). No changes needed except verifying `picomatch` is in `simse-vector/package.json` (already added in Task 1).

**Step 6: Verify typecheck**

Run: `cd simse-vector && bun x tsc --noEmit`
Expected: PASS (may need iteration on librarian.ts/librarian-registry.ts)

**Step 7: Commit**

```bash
git add simse-vector/src/librarian*.ts simse-vector/src/circulation-desk.ts simse-vector/src/library-services.ts
git commit -m "feat(simse-vector): move librarian, circulation-desk, library-services"
```

---

### Task 9: Finalize simse-vector barrel exports

Build the complete `lib.ts` that mirrors what `src/lib.ts` exports from the library.

**Files:**
- Modify: `simse-vector/src/lib.ts`

**Step 1: Write complete lib.ts**

The barrel export should re-export everything from all modules. Mirror the exact exports from the main `src/lib.ts` lines 123–228 (library section) plus error exports.

Include all type exports and value exports. Make sure every public type and factory that simse currently exports from `src/ai/library/` is available from `simse-vector`.

**Step 2: Verify typecheck**

Run: `cd simse-vector && bun x tsc --noEmit`
Expected: PASS

**Step 3: Verify lint**

Run: `cd simse-vector && bun x biome check .`
Expected: PASS (or run `bun x biome check --write .` to auto-fix)

**Step 4: Commit**

```bash
git add simse-vector/src/lib.ts
git commit -m "feat(simse-vector): complete barrel exports"
```

---

### Task 10: Move tests to simse-vector

**Files:**
- Move: `tests/library.test.ts` → `simse-vector/tests/library.test.ts`
- Move: `tests/stacks.test.ts` → `simse-vector/tests/stacks.test.ts`
- Move: `tests/library-types.test.ts` → `simse-vector/tests/library-types.test.ts`
- Move: `tests/library-services.test.ts` → `simse-vector/tests/library-services.test.ts`
- Move: `tests/library-errors.test.ts` → `simse-vector/tests/library-errors.test.ts`
- Move: `tests/e2e-library-pipeline.test.ts` → `simse-vector/tests/e2e-library-pipeline.test.ts`
- Move: `tests/hierarchical-library-integration.test.ts` → `simse-vector/tests/hierarchical-library-integration.test.ts`

**Step 1: Copy test files**

```bash
mkdir -p simse-vector/tests
for f in library stacks library-types library-services library-errors e2e-library-pipeline hierarchical-library-integration; do
  cp "tests/${f}.test.ts" "simse-vector/tests/${f}.test.ts"
done
```

**Step 2: Fix imports in all test files**

All tests currently import from `../src/ai/library/...` or `../src/lib.js`. Change to:

```typescript
// From:
import { createLibrary } from '../src/lib.js';
// To:
import { createLibrary } from '../src/lib.js';
```

The path is the same relative structure (`tests/` → `src/lib.ts`) so most imports just need the library path adjusted. Tests that import error factories from `../src/errors/` need to change to `../src/errors.js`.

**Step 3: Run tests**

Run: `cd simse-vector && bun test`
Expected: All library tests pass.

**Step 4: Commit**

```bash
git add simse-vector/tests/
git commit -m "feat(simse-vector): move all library test files"
```

---

### Task 11: Delete original library files from simse

**Files:**
- Delete: All 26 files in `src/ai/library/`
- Delete: Original test files from `tests/`
- Modify: `src/errors/library.ts` — keep as re-export from simse-vector
- Modify: `src/errors/index.ts` — update re-exports

**Step 1: Delete source files**

```bash
rm -rf src/ai/library/
```

**Step 2: Delete test files**

```bash
for f in library stacks library-types library-services library-errors e2e-library-pipeline hierarchical-library-integration; do
  rm -f "tests/${f}.test.ts"
done
```

**Step 3: Verify no orphan imports**

Run: `grep -r "from '.*library/" src/ --include="*.ts" | grep -v node_modules`

This shows all remaining imports from the deleted library directory. Each needs to be updated in Task 12.

**Step 4: Commit**

```bash
git add -A src/ai/library/ tests/
git commit -m "refactor: delete original library files (moved to simse-vector)"
```

---

### Task 12: Wire simse to depend on simse-vector

Update the main simse package to import from simse-vector.

**Files:**
- Modify: `package.json` — add workspace dep on simse-vector
- Modify: `src/lib.ts` — replace library exports with re-exports from simse-vector
- Modify: `src/errors/library.ts` — re-export from simse-vector
- Modify: `src/errors/index.ts` — update if needed
- Modify: `src/ai/acp/acp-adapters.ts` — import types from simse-vector
- Modify: `src/ai/tools/builtin-tools.ts` — import Library from simse-vector
- Modify: `src/ai/tools/subagent-tools.ts` — import Library from simse-vector
- Modify: `src/ai/mcp/mcp-server.ts` — import Library from simse-vector
- Modify: `src/ai/chain/chain.ts` — import Library from simse-vector
- Modify: `src/ai/chain/format.ts` — import Lookup from simse-vector
- Modify: `src/ai/loop/types.ts` — import LibraryServices from simse-vector
- Modify: `src/ai/agent/agent-executor.ts` — import Library from simse-vector
- Modify: `src/config/settings.ts` — import LibraryConfig from simse-vector

**Step 1: Add dependency**

Add to `package.json`:
```json
"dependencies": {
  "simse-vector": "workspace:*",
  ...
}
```

Run: `bun install`

**Step 2: Update src/lib.ts**

Replace all library export lines (the block from `src/ai/library/`) with:

```typescript
// Library / Vector Store (from simse-vector)
export * from 'simse-vector';
```

Or if selective re-exports are preferred, explicitly re-export each item from `simse-vector`.

**Step 3: Update src/errors/library.ts**

Replace entire file with re-exports from simse-vector:

```typescript
export {
	createEmbeddingError,
	createLibraryError,
	createStacksCorruptionError,
	createStacksError,
	createStacksIOError,
	isEmbeddingError,
	isLibraryError,
	isStacksCorruptionError,
	isStacksError,
	isStacksIOError,
} from 'simse-vector';
```

Also re-export `VectorError` as `SimseError` if needed for type compatibility (the error shapes are identical).

**Step 4: Update acp-adapters.ts**

Change:
```typescript
import type { EmbeddingProvider, TextGenerationProvider } from '../library/types.js';
```
To:
```typescript
import type { EmbeddingProvider, TextGenerationProvider } from 'simse-vector';
```

**Step 5: Update all other consumer files**

For each file that imports from `../library/`:
- Replace `../library/library.js` with `simse-vector`
- Replace `../library/types.js` with `simse-vector`
- Replace `../library/shelf.js` with `simse-vector`

**Step 6: Verify typecheck**

Run: `bun run typecheck`
Expected: PASS

**Step 7: Verify lint**

Run: `bun run lint`
Expected: PASS

**Step 8: Verify tests**

Run: `bun test`
Expected: All remaining tests pass (library tests now run from simse-vector).

**Step 9: Commit**

```bash
git add package.json bun.lock src/
git commit -m "refactor: wire simse to depend on simse-vector for library system"
```

---

### Task 13: Final verification

**Step 1: Full typecheck both packages**

```bash
cd simse-vector && bun x tsc --noEmit && cd .. && bun run typecheck
```
Expected: Both PASS

**Step 2: Full test both packages**

```bash
cd simse-vector && bun test && cd .. && bun test
```
Expected: All tests pass in both packages

**Step 3: Full lint both packages**

```bash
cd simse-vector && bun x biome check . && cd .. && bun run lint
```
Expected: Both PASS

**Step 4: Verify no circular imports**

```bash
grep -r "from 'simse'" simse-vector/src/ --include="*.ts"
```
Expected: Zero matches (simse-vector never imports from simse)

**Step 5: Commit**

```bash
git commit --allow-empty -m "chore: verify simse-vector extraction complete"
```
