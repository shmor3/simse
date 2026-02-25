import { describe, expect, it } from 'bun:test';
import { createToolRegistry } from '../tool-registry.js';

// ---------------------------------------------------------------------------
// Helpers — minimal mocks for simse interfaces
// ---------------------------------------------------------------------------

function createMockMemoryManager() {
	const entries: Array<{ id: string; text: string; topic: string }> = [];

	return {
		add: async (text: string, meta?: { topic?: string }) => {
			const id = `mem_${entries.length}`;
			entries.push({ id, text, topic: meta?.topic ?? 'general' });
			return id;
		},
		search: async (query: string, maxResults?: number) => {
			const max = maxResults ?? 5;
			return entries
				.filter((e) => e.text.toLowerCase().includes(query.toLowerCase()))
				.slice(0, max)
				.map((e) => ({
					score: 0.9,
					entry: {
						text: e.text,
						metadata: { topic: e.topic },
					},
				}));
		},
	};
}

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
		_files: files, // For test setup
	};
}

// ---------------------------------------------------------------------------
// createToolRegistry
// ---------------------------------------------------------------------------

describe('createToolRegistry', () => {
	it('should return a frozen object', () => {
		const registry = createToolRegistry({});
		expect(Object.isFrozen(registry)).toBe(true);
	});

	it('should start with zero tools when no providers given', () => {
		const registry = createToolRegistry({});
		expect(registry.toolCount).toBe(0);
	});

	// -- Built-in memory tools -------------------------------------------------

	it('should register memory tools when memoryManager provided', () => {
		const mm = createMockMemoryManager();
		const registry = createToolRegistry({
			memoryManager: mm as never,
		});

		const defs = registry.getToolDefinitions();
		const names = defs.map((d) => d.name);
		expect(names).toContain('memory_search');
		expect(names).toContain('memory_add');
	});

	it('should execute memory_search', async () => {
		const mm = createMockMemoryManager();
		await mm.add('auth flow design', { topic: 'architecture' });

		const registry = createToolRegistry({
			memoryManager: mm as never,
		});

		const result = await registry.execute({
			id: 'c1',
			name: 'memory_search',
			arguments: { query: 'auth' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('auth flow design');
	});

	it('should execute memory_search with no results', async () => {
		const mm = createMockMemoryManager();
		const registry = createToolRegistry({
			memoryManager: mm as never,
		});

		const result = await registry.execute({
			id: 'c1',
			name: 'memory_search',
			arguments: { query: 'nonexistent' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('No matching entries found');
	});

	it('should execute memory_add', async () => {
		const mm = createMockMemoryManager();
		const registry = createToolRegistry({
			memoryManager: mm as never,
		});

		const result = await registry.execute({
			id: 'c1',
			name: 'memory_add',
			arguments: { text: 'Remember this', topic: 'notes' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('Stored note with ID');
	});

	// -- Built-in VFS tools ----------------------------------------------------

	it('should register VFS tools when vfs provided', () => {
		const vfs = createMockVFS();
		const registry = createToolRegistry({
			vfs: vfs as never,
		});

		const defs = registry.getToolDefinitions();
		const names = defs.map((d) => d.name);
		expect(names).toContain('vfs_read');
		expect(names).toContain('vfs_write');
		expect(names).toContain('vfs_list');
		expect(names).toContain('vfs_tree');
	});

	it('should execute vfs_write and vfs_read', async () => {
		const vfs = createMockVFS();
		const registry = createToolRegistry({
			vfs: vfs as never,
		});

		// Write
		const writeResult = await registry.execute({
			id: 'c1',
			name: 'vfs_write',
			arguments: { path: '/test.txt', content: 'hello world' },
		});
		expect(writeResult.isError).toBe(false);
		expect(writeResult.output).toContain('bytes');

		// Read
		const readResult = await registry.execute({
			id: 'c2',
			name: 'vfs_read',
			arguments: { path: '/test.txt' },
		});
		expect(readResult.isError).toBe(false);
		expect(readResult.output).toBe('hello world');
	});

	it('should execute vfs_list', async () => {
		const vfs = createMockVFS();
		vfs._files.set('/dir/file1.txt', 'content1');
		vfs._files.set('/dir/file2.txt', 'content2');

		const registry = createToolRegistry({
			vfs: vfs as never,
		});

		const result = await registry.execute({
			id: 'c1',
			name: 'vfs_list',
			arguments: { path: '/dir' },
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
			arguments: { path: '/' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('Tree of /');
	});

	// -- Unknown tool ----------------------------------------------------------

	it('should return error for unknown tool', async () => {
		const registry = createToolRegistry({});
		const result = await registry.execute({
			id: 'c1',
			name: 'nonexistent_tool',
			arguments: {},
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('Unknown tool');
		expect(result.output).toContain('nonexistent_tool');
	});

	it('should freeze error results', async () => {
		const registry = createToolRegistry({});
		const result = await registry.execute({
			id: 'c1',
			name: 'nope',
			arguments: {},
		});
		expect(Object.isFrozen(result)).toBe(true);
	});

	// -- Tool handler errors ---------------------------------------------------

	it('should catch handler errors and return isError result', async () => {
		const mm = createMockMemoryManager();
		// Make search throw
		mm.search = async () => {
			throw new Error('Database connection lost');
		};

		const registry = createToolRegistry({
			memoryManager: mm as never,
		});

		const result = await registry.execute({
			id: 'c1',
			name: 'memory_search',
			arguments: { query: 'test' },
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('Tool error');
		expect(result.output).toContain('Database connection lost');
	});

	// -- formatForSystemPrompt -------------------------------------------------

	it('should return empty string when no tools registered', () => {
		const registry = createToolRegistry({});
		expect(registry.formatForSystemPrompt()).toBe('');
	});

	it('should format tool definitions for system prompt', () => {
		const mm = createMockMemoryManager();
		const registry = createToolRegistry({
			memoryManager: mm as never,
		});

		const prompt = registry.formatForSystemPrompt();
		expect(prompt).toContain('tool_use');
		expect(prompt).toContain('memory_search');
		expect(prompt).toContain('memory_add');
		expect(prompt).toContain('query');
		expect(prompt).toContain('Parameters:');
	});

	// -- Discover resets tools -------------------------------------------------

	it('should clear and re-register tools on discover', async () => {
		const mm = createMockMemoryManager();
		const registry = createToolRegistry({
			memoryManager: mm as never,
		});

		const countBefore = registry.toolCount;
		await registry.discover();
		const countAfter = registry.toolCount;

		// Should be the same since no MCP client
		expect(countAfter).toBe(countBefore);
	});

	// -- getToolDefinitions returns frozen array -------------------------------

	it('should return frozen array from getToolDefinitions', () => {
		const mm = createMockMemoryManager();
		const registry = createToolRegistry({
			memoryManager: mm as never,
		});
		expect(Object.isFrozen(registry.getToolDefinitions())).toBe(true);
	});

	// -- Both memory and VFS ---------------------------------------------------

	it('should register both memory and VFS tools', () => {
		const mm = createMockMemoryManager();
		const vfs = createMockVFS();
		const registry = createToolRegistry({
			memoryManager: mm as never,
			vfs: vfs as never,
		});

		expect(registry.toolCount).toBe(6); // 2 memory + 4 vfs
		const names = registry.getToolDefinitions().map((d) => d.name);
		expect(names).toContain('memory_search');
		expect(names).toContain('memory_add');
		expect(names).toContain('vfs_read');
		expect(names).toContain('vfs_write');
		expect(names).toContain('vfs_list');
		expect(names).toContain('vfs_tree');
	});
});
