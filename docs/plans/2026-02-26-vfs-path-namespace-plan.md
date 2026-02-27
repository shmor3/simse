# VFS Path Namespace Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Adopt vfs:// URI scheme for all VFS paths to prevent ACP agent confusion between virtual and real filesystem paths.

**Architecture:** Modify path-utils.ts to enforce vfs:// scheme in normalizePath(), add toLocalPath() for disk conversions. Update VFS core, all tool boundaries, CLI display, and tests.

**Tech Stack:** TypeScript, Bun test runner, Biome linter

---

### Task 1: path-utils.ts -- Add VFS_SCHEME constant and toLocalPath()

**Files:**
- Modify: `src/ai/vfs/path-utils.ts` (top of file, after imports)
- Modify: `src/ai/vfs/index.ts` (barrel re-export)
- Modify: `tests/vfs.test.ts` (add new tests)

These are the foundational exports everything else depends on. `VFS_SCHEME` is the `'vfs://'` constant. `toLocalPath()` strips `vfs://` and returns a bare `/path` for disk operations.

**Step 1: Write the failing test**

Add to `tests/vfs.test.ts` inside the existing `describe('path-utils', ...)` block, after the `pathDepth` describe:

```typescript
	describe('VFS_SCHEME', () => {
		it('equals vfs://', () => {
			expect(VFS_SCHEME).toBe('vfs://');
		});
	});

	describe('toLocalPath', () => {
		it('strips vfs:// prefix from root', () => {
			expect(toLocalPath('vfs:///')).toBe('/');
		});

		it('strips vfs:// prefix from file path', () => {
			expect(toLocalPath('vfs:///hello.js')).toBe('/hello.js');
		});

		it('strips vfs:// prefix from nested path', () => {
			expect(toLocalPath('vfs:///src/components/Button.tsx')).toBe('/src/components/Button.tsx');
		});

		it('throws on path without vfs:// prefix', () => {
			expect(() => toLocalPath('/hello.js')).toThrow();
		});

		it('throws on bare path', () => {
			expect(() => toLocalPath('hello.js')).toThrow();
		});
	});
```

Update the import at the top of `tests/vfs.test.ts` to include the new exports:

```typescript
import {
	VFS_SCHEME,
	ancestorPaths,
	baseName,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
} from '../src/ai/vfs/path-utils.js';
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/vfs.test.ts`

Expected: FAIL -- `VFS_SCHEME` and `toLocalPath` not exported from `path-utils.js`

**Step 3: Write the implementation**

In `src/ai/vfs/path-utils.ts`, add after the `hasForbiddenChars` helper (after line 13), before `normalizePath`:

```typescript
export const VFS_SCHEME = 'vfs://';

export const toLocalPath = (vfsPath: string): string => {
	if (!vfsPath.startsWith(VFS_SCHEME)) {
		throw new Error(`Path must start with ${VFS_SCHEME}: ${vfsPath}`);
	}
	return vfsPath.slice(VFS_SCHEME.length) || '/';
};
```

In `src/ai/vfs/index.ts`, update the path-utils re-export:

```typescript
export {
	VFS_SCHEME,
	ancestorPaths,
	baseName,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
	validatePath,
	validateSegment,
} from './path-utils.js';
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/vfs.test.ts`

Expected: PASS -- new `VFS_SCHEME` and `toLocalPath` tests pass, existing tests still pass (normalizePath not yet changed)

**Step 5: Commit**

```
feat(vfs): add VFS_SCHEME constant and toLocalPath() helper
```

---

### Task 2: path-utils.ts -- Rewrite normalizePath() for vfs:// scheme

**Files:**
- Modify: `src/ai/vfs/path-utils.ts` (lines 15-32, the `normalizePath` function)
- Modify: `tests/vfs.test.ts` (update existing normalizePath tests)

The new `normalizePath()` requires `vfs://` prefix on input, normalizes the path portion (collapse `//`, strip trailing `/`, resolve `.`/`..`), and returns `vfs:///path`.

**Step 1: Write the failing tests**

Replace the entire `describe('normalizePath', ...)` block in `tests/vfs.test.ts`:

```typescript
	describe('normalizePath', () => {
		it('returns vfs:/// for vfs:// root input', () => {
			expect(normalizePath('vfs:///')).toBe('vfs:///');
			expect(normalizePath('vfs://')).toBe('vfs:///');
		});

		it('normalizes vfs:// path with segments', () => {
			expect(normalizePath('vfs:///foo/bar')).toBe('vfs:///foo/bar');
		});

		it('resolves . segments', () => {
			expect(normalizePath('vfs:///foo/./bar')).toBe('vfs:///foo/bar');
		});

		it('resolves .. segments', () => {
			expect(normalizePath('vfs:///foo/bar/../baz')).toBe('vfs:///foo/baz');
		});

		it('does not go above root with ..', () => {
			expect(normalizePath('vfs:///../../foo')).toBe('vfs:///foo');
		});

		it('converts backslashes in path portion', () => {
			expect(normalizePath('vfs:///foo\\bar')).toBe('vfs:///foo/bar');
		});

		it('collapses multiple slashes', () => {
			expect(normalizePath('vfs:////foo///bar///')).toBe('vfs:///foo/bar');
		});

		it('removes trailing slashes', () => {
			expect(normalizePath('vfs:///foo/bar/')).toBe('vfs:///foo/bar');
		});

		it('throws on path without vfs:// prefix', () => {
			expect(() => normalizePath('/foo/bar')).toThrow();
		});

		it('throws on bare path', () => {
			expect(() => normalizePath('foo/bar')).toThrow();
		});

		it('throws on empty string', () => {
			expect(() => normalizePath('')).toThrow();
		});
	});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/vfs.test.ts`

Expected: FAIL -- old normalizePath does not require vfs:// prefix, returns `/foo/bar` instead of `vfs:///foo/bar`

**Step 3: Write the implementation**

Replace `normalizePath` in `src/ai/vfs/path-utils.ts` (lines 15-32):

```typescript
export const normalizePath = (input: string): string => {
	if (!input.startsWith(VFS_SCHEME)) {
		throw new Error(`Path must start with ${VFS_SCHEME}: ${input}`);
	}

	let p = input.slice(VFS_SCHEME.length).replace(/\\/g, '/');
	if (!p.startsWith('/')) p = `/${p}`;

	const segments = p.split('/');
	const resolved: string[] = [];

	for (const seg of segments) {
		if (seg === '' || seg === '.') continue;
		if (seg === '..') {
			resolved.pop();
		} else {
			resolved.push(seg);
		}
	}

	return resolved.length === 0
		? `${VFS_SCHEME}/`
		: `${VFS_SCHEME}/${resolved.join('/')}`;
};
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/vfs.test.ts`

Expected: The normalizePath tests pass. Many other VFS tests will fail because they still use `/path` -- that is expected and will be fixed in subsequent tasks.

**Step 5: Commit**

```
feat(vfs): rewrite normalizePath() to require vfs:// prefix
```

---

### Task 3: path-utils.ts -- Update parentPath, baseName, ancestorPaths, pathDepth

**Files:**
- Modify: `src/ai/vfs/path-utils.ts` (parentPath, baseName, ancestorPaths, pathDepth)
- Modify: `tests/vfs.test.ts` (update test assertions)

These utility functions must now operate on the path portion after the `vfs://` scheme, and return `vfs://`-prefixed results.

**Step 1: Write the failing tests**

Replace the `parentPath`, `baseName`, `ancestorPaths`, and `pathDepth` describe blocks in `tests/vfs.test.ts`:

```typescript
	describe('parentPath', () => {
		it('returns undefined for vfs root', () => {
			expect(parentPath('vfs:///')).toBeUndefined();
		});

		it('returns vfs:/// for direct child of root', () => {
			expect(parentPath('vfs:///foo')).toBe('vfs:///');
		});

		it('returns parent for nested path', () => {
			expect(parentPath('vfs:///foo/bar/baz')).toBe('vfs:///foo/bar');
		});
	});

	describe('baseName', () => {
		it('returns empty string for root', () => {
			expect(baseName('vfs:///')).toBe('');
		});

		it('returns last segment', () => {
			expect(baseName('vfs:///foo/bar')).toBe('bar');
		});
	});

	describe('ancestorPaths', () => {
		it('returns [vfs:///] for root', () => {
			expect(ancestorPaths('vfs:///')).toEqual(['vfs:///']);
		});

		it('returns [vfs:///] for direct child of root', () => {
			expect(ancestorPaths('vfs:///foo')).toEqual(['vfs:///']);
		});

		it('returns all ancestors for nested path', () => {
			expect(ancestorPaths('vfs:///a/b/c')).toEqual([
				'vfs:///',
				'vfs:///a',
				'vfs:///a/b',
			]);
		});
	});

	describe('pathDepth', () => {
		it('returns 0 for root', () => {
			expect(pathDepth('vfs:///')).toBe(0);
		});

		it('returns 1 for vfs:///foo', () => {
			expect(pathDepth('vfs:///foo')).toBe(1);
		});

		it('returns 3 for vfs:///a/b/c', () => {
			expect(pathDepth('vfs:///a/b/c')).toBe(3);
		});
	});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/vfs.test.ts`

Expected: FAIL -- old functions do not handle vfs:// prefix

**Step 3: Write the implementation**

Replace `parentPath`, `baseName`, `ancestorPaths`, `pathDepth` in `src/ai/vfs/path-utils.ts`:

```typescript
const VFS_ROOT = `${VFS_SCHEME}/`;

export const parentPath = (normalizedPath: string): string | undefined => {
	if (normalizedPath === VFS_ROOT) return undefined;
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	const lastSlash = localPart.lastIndexOf('/');
	if (lastSlash === 0) return VFS_ROOT;
	return `${VFS_SCHEME}${localPart.slice(0, lastSlash)}`;
};

export const baseName = (normalizedPath: string): string => {
	if (normalizedPath === VFS_ROOT) return '';
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	const lastSlash = localPart.lastIndexOf('/');
	return localPart.slice(lastSlash + 1);
};

export const ancestorPaths = (normalizedPath: string): string[] => {
	const result: string[] = [VFS_ROOT];
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	const segments = localPart.split('/').filter(Boolean);
	for (let i = 0; i < segments.length - 1; i++) {
		result.push(`${VFS_SCHEME}/${segments.slice(0, i + 1).join('/')}`);
	}
	return result;
};

export const pathDepth = (normalizedPath: string): number => {
	if (normalizedPath === VFS_ROOT) return 0;
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	return localPart.split('/').filter(Boolean).length;
};
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/vfs.test.ts`

Expected: The path utility tests pass. VFS core tests still fail (expected -- fixed in Task 5).

**Step 5: Commit**

```
feat(vfs): update parentPath, baseName, ancestorPaths, pathDepth for vfs:// scheme
```

---

### Task 4: path-utils.ts -- Update validatePath

**Files:**
- Modify: `src/ai/vfs/path-utils.ts` (validatePath function, lines 74-91)

`validatePath` must extract the local path portion (after `vfs://`) before checking path length, depth, and segment names. The path length check should measure only the local portion so existing limits are not affected by the scheme prefix.

**Step 1: Write the failing test**

Add to `tests/vfs.test.ts` inside the `describe('path-utils', ...)` block:

```typescript
	describe('validatePath', () => {
		it('accepts a valid vfs:// path', () => {
			const result = validatePath('vfs:///foo/bar', {
				maxFileSize: 10_485_760,
				maxTotalSize: 104_857_600,
				maxPathDepth: 32,
				maxNameLength: 255,
				maxNodeCount: 10_000,
				maxPathLength: 1024,
				maxDiffLines: 10_000,
			});
			expect(result).toBeUndefined();
		});

		it('rejects path exceeding maxPathDepth', () => {
			const result = validatePath('vfs:///a/b/c/d', {
				maxFileSize: 10_485_760,
				maxTotalSize: 104_857_600,
				maxPathDepth: 2,
				maxNameLength: 255,
				maxNodeCount: 10_000,
				maxPathLength: 1024,
				maxDiffLines: 10_000,
			});
			expect(result).toContain('max depth');
		});
	});
```

Update the import to include `validatePath`:

```typescript
import {
	VFS_SCHEME,
	ancestorPaths,
	baseName,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
	validatePath,
} from '../src/ai/vfs/path-utils.js';
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/vfs.test.ts`

Expected: FAIL -- `validatePath('vfs:///foo/bar', ...)` tries to split on `/` and gets confused by the scheme prefix

**Step 3: Write the implementation**

Replace `validatePath` in `src/ai/vfs/path-utils.ts`:

```typescript
export const validatePath = (
	normalizedPath: string,
	limits: Required<VFSLimits>,
): string | undefined => {
	const localPart = normalizedPath.slice(VFS_SCHEME.length);
	if (localPart.length > limits.maxPathLength) {
		return `Path exceeds max length (${limits.maxPathLength})`;
	}
	const depth = pathDepth(normalizedPath);
	if (depth > limits.maxPathDepth) {
		return `Path exceeds max depth (${limits.maxPathDepth})`;
	}
	const segments = localPart.split('/').filter(Boolean);
	for (const seg of segments) {
		const segError = validateSegment(seg, limits);
		if (segError) return segError;
	}
	return undefined;
};
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/vfs.test.ts`

Expected: validatePath tests pass. Other VFS tests still fail (expected).

**Step 5: Commit**

```
feat(vfs): update validatePath to operate on local path portion after vfs:// scheme
```

---

### Task 5: vfs.ts -- Update internal Map keys and resolve/assertValidPath

**Files:**
- Modify: `src/ai/vfs/vfs.ts` (initRoot, resolve, assertValidPath, getDirectChildren, getDescendants, readdir relative prefix, globFn root skip, writeFile root check, rmdir root check, rename root check, snapshot root skip, clear reinit)
- Modify: `tests/vfs.test.ts` (update all VFS path assertions)

This is the biggest task. Every internal path stored in the `nodes` Map switches from `/path` to `vfs:///path`. All call sites that pass bare `/path` strings must use `vfs:///path`.

**Step 1: Update the test assertions**

This is a mechanical find-and-replace across `tests/vfs.test.ts`. Every call to VFS methods that takes a path argument must use `vfs:///` prefix. Every assertion that checks a returned path must expect `vfs:///` prefix.

Changes to apply in `tests/vfs.test.ts` (comprehensive list):

1. All `vfs.writeFile('/...')` calls become `vfs.writeFile('vfs:///...')`
2. All `vfs.readFile('/...')` calls become `vfs.readFile('vfs:///...')`
3. All `vfs.exists('/...')` calls become `vfs.exists('vfs:///...')`
4. All `vfs.stat('/...')` calls become `vfs.stat('vfs:///...')`
5. All `vfs.mkdir('/...')` calls become `vfs.mkdir('vfs:///...')`
6. All `vfs.readdir('/...')` calls become `vfs.readdir('vfs:///...')`
7. All `vfs.rmdir('/...')` calls become `vfs.rmdir('vfs:///...')`
8. All `vfs.rename('/...', '/...')` calls become `vfs.rename('vfs:///...', 'vfs:///...')`
9. All `vfs.copy('/...', '/...')` calls become `vfs.copy('vfs:///...', 'vfs:///...')`
10. All `vfs.glob('/...')` calls become `vfs.glob('vfs:///...')`
11. All `vfs.tree('/...')` calls become `vfs.tree('vfs:///...')`  (or `vfs.tree()` with no arg)
12. All `vfs.du('/...')` calls become `vfs.du('vfs:///...')`
13. All `vfs.search(query, { glob: '/...' })` patterns become `{ glob: 'vfs:///...' }`
14. All `vfs.history('/...')` calls become `vfs.history('vfs:///...')`
15. All `vfs.diff('/...', '/...')` calls become `vfs.diff('vfs:///...', 'vfs:///...')`
16. All `vfs.diffVersions('/...')` calls become `vfs.diffVersions('vfs:///...')`
17. All `vfs.checkout('/...')` calls become `vfs.checkout('vfs:///...')`
18. All `.path` assertions like `expect(s.path).toBe('/f.txt')` become `expect(s.path).toBe('vfs:///f.txt')`
19. All `results[0].path` assertions like `expect(results[0].path).toBe('/f.txt')` become `expect(results[0].path).toBe('vfs:///f.txt')`
20. All `events[0].path` assertions like `expect(events[0].path).toBe('/f.txt')` become `expect(events[0].path).toBe('vfs:///f.txt')`
21. All glob result assertions like `expect(results).toEqual(['/src/a.ts'])` become `expect(results).toEqual(['vfs:///src/a.ts'])`
22. The `tree()` root label assertion `expect(output).toContain('/')` stays (the tree still shows `vfs:///` which contains `/`) but see Task 6 for the root label change
23. The onFileWrite callback default path `String(args.path ?? '/')` becomes `String(args.path ?? 'vfs:///')`
24. Error message checks with `expectGuardedThrow` that pass `/path` must be updated to `vfs:///path`

Apply these changes as a global find-and-replace. Use the pattern: for every test that does a VFS API call with a string starting with `'/'` or `"/"`, prepend `vfs://`. For every assertion that expects a VFS path starting with `/`, prepend `vfs://`.

**Step 2: Run test to verify it fails**

Run: `bun test tests/vfs.test.ts`

Expected: FAIL -- VFS internals still use `/` root, so `vfs:///` paths will fail

**Step 3: Write the VFS implementation changes**

Apply the following changes to `src/ai/vfs/vfs.ts`:

**3a.** Add `VFS_SCHEME` import and define `VFS_ROOT`:

At the top of the file, update the import from `./path-utils.js`:

```typescript
import {
	VFS_SCHEME,
	ancestorPaths,
	baseName,
	normalizePath,
	parentPath,
	pathDepth,
	toLocalPath,
	validatePath,
} from './path-utils.js';
```

Add a constant after the imports:

```typescript
const VFS_ROOT = `${VFS_SCHEME}/`;
```

**3b.** Update `initRoot()` (line 228-240):

Change:
```typescript
	const initRoot = (): void => {
		const now = Date.now();
		nodes.set('/', {
```
To:
```typescript
	const initRoot = (): void => {
		const now = Date.now();
		nodes.set(VFS_ROOT, {
```

**3c.** Update `getDirectChildren` (line 369-381):

Change:
```typescript
	const getDirectChildren = (dirPath: string): string[] => {
		const prefix = dirPath === '/' ? '/' : `${dirPath}/`;
```
To:
```typescript
	const getDirectChildren = (dirPath: string): string[] => {
		const prefix = dirPath === VFS_ROOT ? VFS_ROOT : `${dirPath}/`;
```

**3d.** Update `getDescendants` (line 383-393):

Change:
```typescript
	const getDescendants = (dirPath: string): string[] => {
		const prefix = dirPath === '/' ? '/' : `${dirPath}/`;
```
To:
```typescript
	const getDescendants = (dirPath: string): string[] => {
		const prefix = dirPath === VFS_ROOT ? VFS_ROOT : `${dirPath}/`;
```

**3e.** Update `writeFile` root check (line 462):

Change:
```typescript
		if (normalized === '/') {
```
To:
```typescript
		if (normalized === VFS_ROOT) {
```

**3f.** Update `readdir` relative path prefix (line 629):

Change:
```typescript
				const prefix = normalized === '/' ? '/' : `${normalized}/`;
```
To:
```typescript
				const prefix = normalized === VFS_ROOT ? VFS_ROOT : `${normalized}/`;
```

**3g.** Update `rmdir` root check (line 647):

Change:
```typescript
		if (normalized === '/') {
```
To:
```typescript
		if (normalized === VFS_ROOT) {
```

**3h.** Update `rename` root check (line 711):

Change:
```typescript
		if (normalizedOld === '/') {
```
To:
```typescript
		if (normalizedOld === VFS_ROOT) {
```

**3i.** Update `globFn` root skip (line 850-851):

Change:
```typescript
		for (const [path, node] of nodes) {
			if (path === '/') continue;
```
To:
```typescript
		for (const [path, node] of nodes) {
			if (path === VFS_ROOT) continue;
```

**3j.** Update `snapshot` root skip (line 1376):

Change:
```typescript
		for (const [path, node] of nodes) {
			if (path === '/' && node.type === 'directory') continue;
```
To:
```typescript
		for (const [path, node] of nodes) {
			if (path === VFS_ROOT && node.type === 'directory') continue;
```

**3k.** Update `clear` / `doClear` -- wherever the clear function re-initializes. Find the `doClear` or `clear` function:

It calls `initRoot()` which already uses `VFS_ROOT` from step 3b, so no extra change needed there.

**3l.** Update `matchGlob` -- the `normalizePath` call inside `matchGlob` (line 161) will now require `vfs://` prefix. Update:

Change:
```typescript
const matchGlob = (filePath: string, pattern: string): boolean => {
	const normalizedPattern = normalizePath(pattern);
	const pathParts = filePath.split('/').filter(Boolean);
	const patternParts = normalizedPattern.split('/').filter(Boolean);

	return matchParts(pathParts, 0, patternParts, 0);
};
```
To:
```typescript
const matchGlob = (filePath: string, pattern: string): boolean => {
	const normalizedPattern = normalizePath(pattern);
	const pathParts = toLocalPath(filePath).split('/').filter(Boolean);
	const patternParts = toLocalPath(normalizedPattern).split('/').filter(Boolean);

	return matchParts(pathParts, 0, patternParts, 0);
};
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/vfs.test.ts`

Expected: PASS -- all VFS tests use `vfs:///` paths and the internals handle them correctly

**Step 5: Commit**

```
feat(vfs): switch internal Map keys and all APIs to vfs:// scheme
```

---

### Task 6: vfs.ts -- Update tree() root label

**Files:**
- Modify: `src/ai/vfs/vfs.ts` (tree function, line 877-907)
- Modify: `tests/vfs.test.ts` (tree output assertions)

The `tree()` function should show `vfs:///` as the root label instead of `/`.

**Step 1: Write the failing test**

Update the tree test in `tests/vfs.test.ts` to check for the `vfs:///` root label:

```typescript
	describe('tree', () => {
		it('shows vfs:/// as root label', () => {
			const vfs = createFS();
			vfs.writeFile('vfs:///src/index.ts', 'code', { createParents: true });

			const output = vfs.tree();
			const firstLine = output.split('\n')[0];
			expect(firstLine).toBe('vfs:///');
		});
	});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/vfs.test.ts`

Expected: FAIL -- tree() currently shows `/` as root label, not `vfs:///`

**Step 3: Write the implementation**

In `src/ai/vfs/vfs.ts`, update the `tree` function (line 877-884):

Change:
```typescript
	const tree = (path?: string): string => {
		const normalized = path ? assertValidPath(path) : '/';
		const node = assertNodeExists(normalized);
		assertIsDirectory(normalized, node);

		const lines: string[] = [];
		const rootName = normalized === '/' ? '/' : baseName(normalized);
		lines.push(rootName);
```
To:
```typescript
	const tree = (path?: string): string => {
		const normalized = path ? assertValidPath(path) : VFS_ROOT;
		const node = assertNodeExists(normalized);
		assertIsDirectory(normalized, node);

		const lines: string[] = [];
		const rootName = normalized === VFS_ROOT ? VFS_ROOT : baseName(normalized);
		lines.push(rootName);
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/vfs.test.ts`

Expected: PASS

**Step 5: Commit**

```
feat(vfs): show vfs:/// as root label in tree() output
```

---

### Task 7: vfs-disk.ts -- Update commit() path conversion

**Files:**
- Modify: `src/ai/vfs/vfs-disk.ts` (commit function, lines 157-189)
- Modify: `tests/e2e-vfs-tool.test.ts` (update paths in test assertions)

The `commit()` function reads `snap.directories[].path` and `snap.files[].path` which are now `vfs:///path`. Before joining with the disk target directory, it must call `toLocalPath()` to strip the `vfs://` prefix.

**Step 1: Write the failing test**

Update `tests/e2e-vfs-tool.test.ts`. The VFS paths in tool call arguments and assertions must change to `vfs:///` format. Update:

In the `vfs_write` tool handler:
```typescript
			async (args) => {
				const path = String(args.path ?? '');
				const content = String(args.content ?? '');
				vfs.writeFile(path, content, { createParents: true });
				return `Wrote ${Buffer.byteLength(content, 'utf-8')} bytes to ${path}`;
			},
```

In the mock ACP responses:
```typescript
		const mockResponse = `I'll create that file for you.

<tool_use>
{"id": "call_1", "name": "vfs_write", "arguments": {"path": "vfs:///hello.txt", "content": "Hello from VFS!"}}
</tool_use>

The file has been created.`;
```

In the assertions:
```typescript
		expect(result.output).toContain('vfs:///hello.txt');

		// 6. Verify VFS has the file
		expect(vfs.exists('vfs:///hello.txt')).toBe(true);
		const vfsContent = vfs.readFile('vfs:///hello.txt');
		expect(vfsContent.text).toBe('Hello from VFS!');
		expect(writeEvents).toHaveLength(1);
		expect(writeEvents[0].path).toBe('vfs:///hello.txt');
		expect(writeEvents[0].isNew).toBe(true);
```

Similarly for the nested directory test:
```typescript
		const mockResponse = `<tool_use>
{"id": "call_1", "name": "vfs_write", "arguments": {"path": "vfs:///src/components/Button.tsx", "content": "export const Button = () => <button>Click</button>;"}}
</tool_use>`;
```

And:
```typescript
		expect(vfs.exists('vfs:///src/components/Button.tsx')).toBe(true);
```

For the commit result test:
```typescript
		vfs.writeFile('vfs:///readme.md', '# Hello', { createParents: true });
		vfs.writeFile('vfs:///src/index.ts', 'console.log("hi")', {
			createParents: true,
		});
```

For the parseToolCalls multiple tool calls test:
```typescript
		expect(parsed.toolCalls[0].arguments.path).toBe('vfs:///a.txt');
		expect(parsed.toolCalls[1].arguments.path).toBe('vfs:///b.txt');
```

And the mock response paths:
```typescript
		const response = `Let me create two files.

<tool_use>
{"id": "call_1", "name": "vfs_write", "arguments": {"path": "vfs:///a.txt", "content": "file A"}}
</tool_use>

<tool_use>
{"id": "call_2", "name": "vfs_write", "arguments": {"path": "vfs:///b.txt", "content": "file B"}}
</tool_use>

Both files created.`;
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/e2e-vfs-tool.test.ts`

Expected: FAIL -- commit() tries to join `vfs:///hello.txt` with disk path, producing an invalid filesystem path

**Step 3: Write the implementation**

In `src/ai/vfs/vfs-disk.ts`, add `toLocalPath` to the import from `./path-utils.js`:

At the top of the file, the `vfs-disk.ts` does not currently import from `path-utils.ts`. Add:

```typescript
import { toLocalPath } from './path-utils.js';
```

Update the commit function's directory and file path construction (lines 164-167 and 187-189):

Change directory path construction:
```typescript
			const diskPath = join(
				resolvedTarget,
				...dir.path.split('/').filter(Boolean),
			);
```
To:
```typescript
			const diskPath = join(
				resolvedTarget,
				...toLocalPath(dir.path).split('/').filter(Boolean),
			);
```

Change file path construction:
```typescript
			const diskPath = join(
				resolvedTarget,
				...file.path.split('/').filter(Boolean),
			);
```
To:
```typescript
			const diskPath = join(
				resolvedTarget,
				...toLocalPath(file.path).split('/').filter(Boolean),
			);
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/e2e-vfs-tool.test.ts`

Expected: PASS -- commit writes files to disk correctly

**Step 5: Commit**

```
feat(vfs): use toLocalPath() in commit() for disk path construction
```

---

### Task 8: vfs-disk.ts -- Update load() path construction

**Files:**
- Modify: `src/ai/vfs/vfs-disk.ts` (load function, lines 299-389)

The `load()` function constructs VFS paths from disk directory entries. Currently it uses `''` as the base and builds paths like `/${entry.name}`. It must now build `vfs:///${entry.name}`.

**Step 1: Write the failing test**

Add a load test to `tests/e2e-vfs-tool.test.ts`:

```typescript
	it('load() imports disk files with vfs:// paths', async () => {
		tempDir = mkdtempSync(join(tmpdir(), 'simse-vfs-e2e-'));

		// Create a file on disk
		const { writeFileSync, mkdirSync } = await import('node:fs');
		mkdirSync(join(tempDir, 'src'), { recursive: true });
		writeFileSync(join(tempDir, 'src', 'index.ts'), 'console.log("hi")');

		const vfs = createVirtualFS();
		const disk = createVFSDisk(vfs, { baseDir: tempDir });

		const result = await disk.load();
		expect(result.filesWritten).toBe(1);
		expect(vfs.exists('vfs:///src/index.ts')).toBe(true);
		expect(vfs.readFile('vfs:///src/index.ts').text).toBe('console.log("hi")');
	});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/e2e-vfs-tool.test.ts`

Expected: FAIL -- load constructs `/src/index.ts` but VFS now requires `vfs:///src/index.ts`

**Step 3: Write the implementation**

In `src/ai/vfs/vfs-disk.ts`, add `VFS_SCHEME` to the import:

```typescript
import { VFS_SCHEME, toLocalPath } from './path-utils.js';
```

Update the initial `scanDir` call (line 389):

Change:
```typescript
		await scanDir(resolvedSource, '');
```
To:
```typescript
		await scanDir(resolvedSource, 'vfs://');
```

This makes `entryVfsPath = `${vfsBase}/${entry.name}`` produce `vfs:///entry.name` for root-level entries, and `vfs:///dir/entry.name` for nested entries.

Also update the `filter` callback path. Currently (line 305):
```typescript
				const relativePath = `/${relative(resolvedSource, entryDiskPath).split(sep).join('/')}`;
```
Change to:
```typescript
				const relativePath = `${VFS_SCHEME}/${relative(resolvedSource, entryDiskPath).split(sep).join('/')}`;
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/e2e-vfs-tool.test.ts`

Expected: PASS

**Step 5: Commit**

```
feat(vfs): construct vfs:// paths in load() when importing disk files
```

---

### Task 9: builtin-tools.ts -- Update VFS tool descriptions and defaults

**Files:**
- Modify: `src/ai/tools/builtin-tools.ts` (lines 137-249)
- Modify: `tests/builtin-tools.test.ts` (update path arguments)

Tool descriptions must mention `vfs://` scheme. Default path arguments change from `/` to `vfs:///`. Tool result strings must show `vfs://` paths.

**Step 1: Write the failing test**

Update `tests/builtin-tools.test.ts` VFS tool tests. Change all path arguments from `/path` to `vfs:///path`:

```typescript
	it('vfs_write then vfs_read round-trips content', async () => {
		await registry.execute({
			id: 'w1',
			name: 'vfs_write',
			arguments: { path: 'vfs:///test.txt', content: 'hello world' },
		});

		const result = await registry.execute({
			id: 'r1',
			name: 'vfs_read',
			arguments: { path: 'vfs:///test.txt' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toBe('hello world');
	});

	it('vfs_list shows files', async () => {
		await registry.execute({
			id: 'w1',
			name: 'vfs_write',
			arguments: { path: 'vfs:///dir/a.txt', content: 'a' },
		});

		const result = await registry.execute({
			id: 'l1',
			name: 'vfs_list',
			arguments: { path: 'vfs:///dir' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('a.txt');
	});

	it('vfs_list returns empty message for empty dir', async () => {
		const result = await registry.execute({
			id: 'l2',
			name: 'vfs_list',
			arguments: { path: 'vfs:///' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('empty');
	});

	it('vfs_tree returns tree output', async () => {
		await registry.execute({
			id: 'w1',
			name: 'vfs_write',
			arguments: { path: 'vfs:///a/b.txt', content: 'data' },
		});

		const result = await registry.execute({
			id: 't1',
			name: 'vfs_tree',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('b.txt');
	});

	it('vfs_read throws on non-existent file', async () => {
		const result = await registry.execute({
			id: 'r2',
			name: 'vfs_read',
			arguments: { path: 'vfs:///nope.txt' },
		});
		expect(result.isError).toBe(true);
	});
```

**Step 2: Run test to verify it fails**

Run: `bun test tests/builtin-tools.test.ts`

Expected: FAIL -- the tool handlers still use `String(args.path ?? '/')` as the default, and `normalizePath()` now rejects bare `/`

**Step 3: Write the implementation**

In `src/ai/tools/builtin-tools.ts`, update the VFS tools:

**vfs_read** (lines 141-164):
```typescript
	registerTool(
		registry,
		{
			name: 'vfs_read',
			description: 'Read a file from the virtual filesystem sandbox.',
			parameters: {
				path: {
					type: 'string',
					description: 'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
					required: true,
				},
			},
			category: 'vfs',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const path = String(args.path ?? 'vfs:///');
				const result = vfs.readFile(path);
				if (result.contentType === 'binary') {
					return `[Binary file: ${result.size} bytes]`;
				}
				return result.text;
			} catch (err) {
				throw toError(err);
			}
		},
	);
```

**vfs_write** (lines 167-196):
```typescript
	registerTool(
		registry,
		{
			name: 'vfs_write',
			description: 'Write a file to the virtual filesystem sandbox.',
			parameters: {
				path: {
					type: 'string',
					description: 'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
					required: true,
				},
				content: {
					type: 'string',
					description: 'The file content to write',
					required: true,
				},
			},
			category: 'vfs',
		},
		async (args) => {
			try {
				const path = String(args.path ?? '');
				const content = String(args.content ?? '');
				vfs.writeFile(path, content, { createParents: true });
				return `Wrote ${Buffer.byteLength(content, 'utf-8')} bytes to ${path}`;
			} catch (err) {
				throw toError(err);
			}
		},
	);
```

**vfs_list** (lines 198-225):
```typescript
	registerTool(
		registry,
		{
			name: 'vfs_list',
			description:
				'List files and directories in the virtual filesystem sandbox.',
			parameters: {
				path: {
					type: 'string',
					description: 'VFS directory path using vfs:// scheme (default: vfs:///)',
				},
			},
			category: 'vfs',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const path = String(args.path ?? 'vfs:///');
				const entries = vfs.readdir(path);
				if (entries.length === 0) return 'Directory is empty.';
				return entries
					.map((e) => `${e.type === 'directory' ? 'd' : 'f'} ${e.name}`)
					.join('\n');
			} catch (err) {
				throw toError(err);
			}
		},
	);
```

**vfs_tree** (lines 227-249):
```typescript
	registerTool(
		registry,
		{
			name: 'vfs_tree',
			description: 'Show a tree view of the virtual filesystem sandbox.',
			parameters: {
				path: {
					type: 'string',
					description: 'VFS root path using vfs:// scheme (default: vfs:///)',
				},
			},
			category: 'vfs',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const path = String(args.path ?? 'vfs:///');
				return vfs.tree(path);
			} catch (err) {
				throw toError(err);
			}
		},
	);
```

**Step 4: Run test to verify it passes**

Run: `bun test tests/builtin-tools.test.ts`

Expected: PASS

**Step 5: Commit**

```
feat(vfs): update builtin-tools VFS tool descriptions and defaults for vfs:// scheme
```

---

### Task 10: mcp-server.ts -- Update MCP VFS tool descriptions and defaults

**Files:**
- Modify: `src/ai/mcp/mcp-server.ts` (lines 512-645)

Same changes as Task 9 but for MCP server tools.

**Step 1: No new test needed**

The MCP server tools are tested via integration. The changes are descriptions and defaults only.

**Step 2: Write the implementation**

In `src/ai/mcp/mcp-server.ts`, update the VFS tools:

**vfs-read** (lines 512-545):

Change description:
```typescript
				{
					title: 'VFS Read',
					description: 'Read a file from the virtual filesystem sandbox. Paths use vfs:// scheme (e.g. vfs:///hello.js)',
					inputSchema: {
						path: z.string().describe('VFS path using vfs:// scheme (e.g. vfs:///hello.js)'),
					},
				},
```

**vfs-write** (lines 547-578):

Change description:
```typescript
				{
					title: 'VFS Write',
					description: 'Write a file to the virtual filesystem sandbox. Paths use vfs:// scheme (e.g. vfs:///hello.js)',
					inputSchema: {
						path: z.string().describe('VFS path using vfs:// scheme (e.g. vfs:///hello.js)'),
						content: z.string().describe('The file content'),
					},
				},
```

**vfs-list** (lines 580-617):

Change description and default:
```typescript
				{
					title: 'VFS List',
					description:
						'List files and directories in the virtual filesystem sandbox. Paths use vfs:// scheme.',
					inputSchema: {
						path: z
							.string()
							.optional()
							.describe('VFS directory path (default: vfs:///)'),
					},
				},
				async ({ path }) => {
					try {
						const entries = vfs.readdir((path as string) ?? 'vfs:///');
```

**vfs-tree** (lines 619-645):

Change description and default:
```typescript
				{
					title: 'VFS Tree',
					description: 'Show a tree view of the virtual filesystem sandbox. Paths use vfs:// scheme.',
					inputSchema: {
						path: z
							.string()
							.optional()
							.describe('VFS root path (default: vfs:///)'),
					},
				},
				async ({ path }) => {
					try {
						const tree = vfs.tree((path as string) ?? 'vfs:///');
```

**Step 3: Run typecheck**

Run: `bun run typecheck`

Expected: PASS

**Step 4: Commit**

```
feat(vfs): update MCP server VFS tool descriptions and defaults for vfs:// scheme
```

---

### Task 11: simse-code/tool-registry.ts -- Update CLI VFS tool defaults

**Files:**
- Modify: `simse-code/tool-registry.ts` (lines 148-234)
- Modify: `simse-code/tests/tool-registry.test.ts` (update mock VFS and test paths)

**Step 1: Write the failing test**

In `simse-code/tests/tool-registry.test.ts`, update the mock VFS to accept `vfs://` paths and update test call arguments.

Update the mock VFS factory (uses a Map that must now key on `vfs:///` paths):

```typescript
function createMockVFS() {
	const files = new Map<string, string>();

	return {
		readFile: (path: string) => {
			const content = files.get(path);
			if (content === undefined) {
				throw new Error(`File not found: ${path}`);
			}
			return { text: content, contentType: 'text', size: content.length };
		},
		writeFile: (
			path: string,
			content: string,
			_opts?: { createParents?: boolean },
		) => {
			files.set(path, content);
		},
		readdir: (path: string) => {
			const entries: Array<{ name: string; type: string }> = [];
			for (const key of files.keys()) {
				if (key.startsWith(path) && key !== path) {
					const rest = key.slice(
						path.endsWith('/') ? path.length : path.length + 1,
					);
					const parts = rest.split('/');
					const name = parts[0];
					const type = parts.length > 1 ? 'directory' : 'file';
					if (!entries.find((e) => e.name === name)) {
						entries.push({ name, type });
					}
				}
			}
			return entries;
		},
		tree: (path: string) => `Tree of ${path}`,
		_files: files,
	};
}
```

Update test assertions:

```typescript
	it('should execute vfs_write and vfs_read', async () => {
		const vfs = createMockVFS();
		const registry = createToolRegistry({
			vfs: vfs as never,
		});

		const writeResult = await registry.execute({
			id: 'c1',
			name: 'vfs_write',
			arguments: { path: 'vfs:///test.txt', content: 'hello world' },
		});
		expect(writeResult.isError).toBe(false);
		expect(writeResult.output).toContain('bytes');

		const readResult = await registry.execute({
			id: 'c2',
			name: 'vfs_read',
			arguments: { path: 'vfs:///test.txt' },
		});
		expect(readResult.isError).toBe(false);
		expect(readResult.output).toBe('hello world');
	});

	it('should execute vfs_list', async () => {
		const vfs = createMockVFS();
		vfs._files.set('vfs:///dir/file1.txt', 'content1');
		vfs._files.set('vfs:///dir/file2.txt', 'content2');

		const registry = createToolRegistry({
			vfs: vfs as never,
		});

		const result = await registry.execute({
			id: 'c1',
			name: 'vfs_list',
			arguments: { path: 'vfs:///dir' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('file1.txt');
		expect(result.output).toContain('file2.txt');
	});

	it('should execute vfs_tree', async () => {
		const vfs = createMockVFS();
		const registry = createToolRegistry({
			vfs: vfs as never,
		});

		const result = await registry.execute({
			id: 'c1',
			name: 'vfs_tree',
			arguments: { path: 'vfs:///' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('Tree of vfs:///');
	});
```

**Step 2: Run test to verify it fails**

Run: `bun test simse-code/tests/tool-registry.test.ts`

Expected: FAIL -- tool handlers still default to `/` and descriptions don't mention vfs://

**Step 3: Write the implementation**

In `simse-code/tool-registry.ts`, update VFS tools (lines 148-234):

**vfs_read** handler default:
```typescript
			async (args) => {
				const path = String(args.path ?? 'vfs:///');
				const result = vfs.readFile(path);
```

Description update:
```typescript
				description: 'Read a file from the virtual filesystem sandbox.',
				parameters: {
					path: {
						type: 'string',
						description: 'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
						required: true,
					},
				},
```

**vfs_write** description:
```typescript
				description: 'Write a file to the virtual filesystem sandbox.',
				parameters: {
					path: {
						type: 'string',
						description: 'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
						required: true,
					},
					content: {
						type: 'string',
						description: 'The file content to write',
						required: true,
					},
				},
```

**vfs_list** handler default:
```typescript
			async (args) => {
				const path = String(args.path ?? 'vfs:///');
				const entries = vfs.readdir(path);
```

Description update:
```typescript
				description:
					'List files and directories in the virtual filesystem sandbox.',
				parameters: {
					path: {
						type: 'string',
						description: 'VFS directory path using vfs:// scheme (default: vfs:///)',
					},
				},
```

**vfs_tree** handler default:
```typescript
			async (args) => {
				const path = String(args.path ?? 'vfs:///');
				return vfs.tree(path);
			},
```

Description update:
```typescript
				description: 'Show a tree view of the virtual filesystem sandbox.',
				parameters: {
					path: {
						type: 'string',
						description: 'VFS root path using vfs:// scheme (default: vfs:///)',
					},
				},
```

**Step 4: Run test to verify it passes**

Run: `bun test simse-code/tests/tool-registry.test.ts`

Expected: PASS

**Step 5: Commit**

```
feat(vfs): update CLI tool-registry VFS tool defaults and descriptions for vfs:// scheme
```

---

### Task 12: simse-code/cli.ts -- Update CLI display paths

**Files:**
- Modify: `simse-code/cli.ts` (onFileWrite callback, tryRenderInlineDiff, file tracker calls)

The VFS `onFileWrite` callback receives `event.path` which is now `vfs:///hello.js`. The display already prints `event.path` directly, so the output naturally changes to `[vfs] created vfs:///hello.js (67 bytes)`. No code change needed for onFileWrite -- the path flows through automatically.

For `tryRenderInlineDiff`: The function extracts `path` from tool call args (already `vfs:///path`) and passes it to `vfs.history(path)` and `vfs.diffVersions(path, ...)`. Since the VFS APIs now expect `vfs:///` paths and the tool args contain `vfs:///` paths, this works without code changes.

For the file tracker: The tracker calls `ctx.fileTracker.track(filePath, ...)` where `filePath` is extracted from tool call args. Since tool args now contain `vfs:///` paths, the file tracker automatically stores `vfs:///` paths. No code change needed.

**Step 1: Verify no code changes needed**

The only change that may be needed is if any code in `cli.ts` constructs VFS paths with bare `/`. Search for hardcoded path defaults.

Check the filePath extraction in `cli.ts` (around lines 656-677 and 938-958):
```typescript
				const filePath =
					(parsed.path as string) ??
					(parsed.file_path as string) ??
					(parsed.filePath as string);
```

This extracts from tool call args, which already contain `vfs:///` paths. No change needed.

**Step 2: Run typecheck**

Run: `bun run typecheck`

Expected: PASS -- no changes needed in cli.ts since paths flow through from tool args

**Step 3: Commit**

No commit needed -- this task is a verification that cli.ts requires no changes.

---

### Task 13: Update remaining test files

**Files:**
- Modify: `tests/builtin-tools.test.ts` (already done in Task 9)
- Modify: `tests/e2e-vfs-tool.test.ts` (already done in Tasks 7-8)
- Modify: `simse-code/tests/tool-registry.test.ts` (already done in Task 11)

This task is a final sweep to catch any remaining test assertions that reference bare `/path` for VFS operations.

**Step 1: Run all tests**

Run: `bun test`

**Step 2: Fix any remaining failures**

If any test still passes bare `/path` to a VFS API, update it to use `vfs:///path`.

Common patterns to search for:
- `vfs.writeFile('/` -> `vfs.writeFile('vfs:///`
- `vfs.readFile('/` -> `vfs.readFile('vfs:///`
- `vfs.exists('/` -> `vfs.exists('vfs:///`
- `vfs.stat('/` -> `vfs.stat('vfs:///`
- `vfs.mkdir('/` -> `vfs.mkdir('vfs:///`
- `path: '/` -> `path: 'vfs:///` (in tool call arguments objects)
- `.path).toBe('/` -> `.path).toBe('vfs:///`

**Step 3: Run all tests again**

Run: `bun test`

Expected: ALL PASS

**Step 4: Run lint and typecheck**

Run: `bun run typecheck && bun run lint`

Expected: PASS

**Step 5: Commit**

```
test(vfs): update all test path assertions from /path to vfs:///path
```

---

## Summary of all files modified

| File | Change |
|------|--------|
| `src/ai/vfs/path-utils.ts` | Add `VFS_SCHEME`, `toLocalPath()`. Rewrite `normalizePath()`, `parentPath()`, `baseName()`, `ancestorPaths()`, `pathDepth()`, `validatePath()` |
| `src/ai/vfs/index.ts` | Re-export `VFS_SCHEME`, `toLocalPath` |
| `src/ai/vfs/vfs.ts` | Import `VFS_SCHEME`/`toLocalPath`. Change root key from `'/'` to `VFS_ROOT`. Update `getDirectChildren`, `getDescendants`, `writeFile`, `readdir`, `rmdir`, `rename`, `globFn`, `tree`, `snapshot` root comparisons. Update `matchGlob` to strip scheme. |
| `src/ai/vfs/vfs-disk.ts` | Import `VFS_SCHEME`/`toLocalPath`. Use `toLocalPath()` in `commit()`. Use `vfs://` base in `load()`. |
| `src/ai/tools/builtin-tools.ts` | Update VFS tool descriptions and default paths to `vfs:///` |
| `src/ai/mcp/mcp-server.ts` | Update MCP VFS tool descriptions and default paths to `vfs:///` |
| `simse-code/tool-registry.ts` | Update CLI VFS tool descriptions and default paths to `vfs:///` |
| `tests/vfs.test.ts` | All VFS path arguments and assertions switch from `/path` to `vfs:///path` |
| `tests/e2e-vfs-tool.test.ts` | All VFS path arguments, mock responses, and assertions switch to `vfs:///path` |
| `tests/builtin-tools.test.ts` | All VFS tool call path arguments switch to `vfs:///path` |
| `simse-code/tests/tool-registry.test.ts` | Mock VFS keys and tool call path arguments switch to `vfs:///path` |
