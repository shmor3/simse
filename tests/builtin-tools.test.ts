import { afterEach, beforeEach, describe, expect, it, mock } from 'bun:test';
import { fileURLToPath } from 'node:url';
import type { Library } from 'simse-vector';
import { createVirtualFS, type VirtualFS } from 'simse-vfs';
import type { TaskList } from '../src/ai/tasks/types.js';
import {
	registerLibraryTools,
	registerTaskTools,
	registerVFSTools,
} from '../src/ai/tools/builtin-tools.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import type { ToolRegistry } from '../src/ai/tools/types.js';
import { createSilentLogger } from './utils/mocks.js';

const ENGINE_PATH = fileURLToPath(
	new URL(
		'../simse-vfs/engine/target/debug/simse-vfs-engine.exe',
		import.meta.url,
	),
);

// ---------------------------------------------------------------------------
// Mock factories
// ---------------------------------------------------------------------------

function createMockLibrary(): Library {
	return {
		initialize: mock(async () => {}),
		dispose: mock(async () => {}),
		add: mock(async () => 'mock-id'),
		addBatch: mock(async () => []),
		search: mock(async () => [
			{
				volume: {
					id: '1',
					text: 'result text',
					embedding: [0.1],
					metadata: { topic: 'test' },
					timestamp: Date.now(),
				},
				score: 0.9,
			},
		]),
		textSearch: mock(async () => []),
		filterByMetadata: mock(async () => []),
		filterByDateRange: mock(async () => []),
		advancedSearch: mock(async () => []),
		getById: mock(async () => undefined),
		getAll: mock(async () => []),
		getTopics: mock(async () => []),
		filterByTopic: mock(async () => []),
		recommend: mock(async () => []),
		findDuplicates: mock(async () => []),
		checkDuplicate: mock(async () => ({ isDuplicate: false })),
		compendium: mock(async () => ({
			compendiumId: 'comp-1',
			text: '',
			sourceIds: [],
			deletedOriginals: false,
		})),
		setTextGenerator: mock(() => {}),
		recordFeedback: mock(async () => {}),
		delete: mock(async () => true),
		deleteBatch: mock(async () => 0),
		clear: mock(async () => {}),
		patronProfile: Promise.resolve(undefined),
		size: Promise.resolve(1),
		shelves: mock(async () => []),
		query: mock(async () => []),
		isInitialized: true,
		isDirty: false,
		embeddingAgent: undefined,
	} as unknown as Library;
}

function createMockTaskList(): TaskList {
	const tasks = new Map<
		string,
		{
			id: string;
			subject: string;
			description: string;
			status: string;
			blockedBy: string[];
			blocks: string[];
		}
	>();
	let nextId = 1;

	return {
		create: mock((input: { subject: string; description: string }) => {
			const id = String(nextId++);
			const task = {
				id,
				subject: input.subject,
				description: input.description,
				status: 'pending',
				blockedBy: [],
				blocks: [],
			};
			tasks.set(id, task);
			return task;
		}),
		get: mock((id: string) => tasks.get(id) ?? null),
		update: mock((id: string, updates: Record<string, unknown>) => {
			const task = tasks.get(id);
			if (!task) return null;
			Object.assign(task, updates);
			return task;
		}),
		delete: mock((id: string) => {
			return tasks.delete(id);
		}),
		list: mock(() => [...tasks.values()]),
	} as unknown as TaskList;
}

// ---------------------------------------------------------------------------
// Library Tools
// ---------------------------------------------------------------------------

describe('registerLibraryTools', () => {
	let registry: ToolRegistry;
	let library: Library;

	beforeEach(() => {
		registry = createToolRegistry({ logger: createSilentLogger() });
		library = createMockLibrary();
		registerLibraryTools(registry, library);
	});

	it('registers library_search and library_shelve tools', () => {
		expect(registry.toolNames).toContain('library_search');
		expect(registry.toolNames).toContain('library_shelve');
	});

	it('library_search returns formatted results', async () => {
		const result = await registry.execute({
			id: 'call-1',
			name: 'library_search',
			arguments: { query: 'test' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('result text');
		expect(result.output).toContain('0.90');
	});

	it('library_search returns no-match message for empty results', async () => {
		(library.search as ReturnType<typeof mock>).mockResolvedValue([]);
		const result = await registry.execute({
			id: 'call-1',
			name: 'library_search',
			arguments: { query: 'nothing' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('No matching');
	});

	it('library_shelve stores text and returns ID', async () => {
		const result = await registry.execute({
			id: 'call-2',
			name: 'library_shelve',
			arguments: { text: 'remember this', topic: 'notes' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('mock-id');
	});

	it('library_shelve wraps errors via toError', async () => {
		(library.add as ReturnType<typeof mock>).mockRejectedValue(
			'raw string error',
		);
		const result = await registry.execute({
			id: 'call-3',
			name: 'library_shelve',
			arguments: { text: 'fail', topic: 'x' },
		});
		expect(result.isError).toBe(true);
	});

	it('registers library_catalog tool', () => {
		const defs = registry.getToolDefinitions();
		expect(defs.find((d) => d.name === 'library_catalog')).toBeDefined();
	});

	it('library_catalog returns topic tree', async () => {
		const result = await registry.execute({
			id: 'call_1',
			name: 'library_catalog',
			arguments: {},
		});
		expect(result.isError).toBe(false);
	});

	it('library_catalog returns no-topics message for empty catalog', async () => {
		const result = await registry.execute({
			id: 'call_2',
			name: 'library_catalog',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('No topics found');
	});

	it('library_catalog formats topics with indent and counts', async () => {
		(library.getTopics as ReturnType<typeof mock>).mockResolvedValue([
			{ topic: 'code', entryCount: 5 },
			{ topic: 'code/js', entryCount: 3 },
			{ topic: 'notes', entryCount: 2 },
		]);
		const result = await registry.execute({
			id: 'call_3',
			name: 'library_catalog',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('code (5 volumes)');
		expect(result.output).toContain('  code/js (3 volumes)');
		expect(result.output).toContain('notes (2 volumes)');
	});

	it('library_catalog filters by topic', async () => {
		(library.getTopics as ReturnType<typeof mock>).mockResolvedValue([
			{ topic: 'code', entryCount: 5 },
			{ topic: 'code/js', entryCount: 3 },
			{ topic: 'notes', entryCount: 2 },
		]);
		const result = await registry.execute({
			id: 'call_4',
			name: 'library_catalog',
			arguments: { topic: 'code' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('code');
		expect(result.output).toContain('code/js');
		expect(result.output).not.toContain('notes');
	});

	it('registers library_compact tool', () => {
		const defs = registry.getToolDefinitions();
		expect(defs.find((d) => d.name === 'library_compact')).toBeDefined();
	});

	it('library_compact returns nothing-to-compact for < 2 volumes', async () => {
		const result = await registry.execute({
			id: 'call_5',
			name: 'library_compact',
			arguments: { topic: 'empty' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('fewer than 2 volumes');
	});

	it('library_compact creates compendium when volumes exist', async () => {
		(library.filterByTopic as ReturnType<typeof mock>).mockResolvedValue([
			{ id: 'v1', text: 'a' },
			{ id: 'v2', text: 'b' },
		]);
		(library.compendium as ReturnType<typeof mock>).mockResolvedValue({
			compendiumId: 'comp-1',
			text: 'summary',
			sourceIds: ['v1', 'v2'],
			deletedOriginals: false,
		});
		const result = await registry.execute({
			id: 'call_6',
			name: 'library_compact',
			arguments: { topic: 'code' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('comp-1');
		expect(result.output).toContain('2 volumes');
	});
});

// ---------------------------------------------------------------------------
// VFS Tools
// ---------------------------------------------------------------------------

describe('registerVFSTools', () => {
	let registry: ToolRegistry;
	let vfs: VirtualFS;

	beforeEach(async () => {
		registry = createToolRegistry({ logger: createSilentLogger() });
		vfs = await createVirtualFS({ enginePath: ENGINE_PATH });
		registerVFSTools(registry, vfs);
	});

	afterEach(async () => {
		await vfs?.dispose();
	});

	it('registers vfs_read, vfs_write, vfs_list, vfs_tree tools', () => {
		expect(registry.toolNames).toContain('vfs_read');
		expect(registry.toolNames).toContain('vfs_write');
		expect(registry.toolNames).toContain('vfs_list');
		expect(registry.toolNames).toContain('vfs_tree');
	});

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
});

// ---------------------------------------------------------------------------
// Task Tools
// ---------------------------------------------------------------------------

describe('registerTaskTools', () => {
	let registry: ToolRegistry;
	let taskList: TaskList;

	beforeEach(() => {
		registry = createToolRegistry({ logger: createSilentLogger() });
		taskList = createMockTaskList();
		registerTaskTools(registry, taskList);
	});

	it('registers task_create, task_get, task_update, task_list, task_delete tools', () => {
		expect(registry.toolNames).toContain('task_create');
		expect(registry.toolNames).toContain('task_get');
		expect(registry.toolNames).toContain('task_update');
		expect(registry.toolNames).toContain('task_list');
		expect(registry.toolNames).toContain('task_delete');
	});

	it('task_create creates and returns task info', async () => {
		const result = await registry.execute({
			id: 'c1',
			name: 'task_create',
			arguments: { subject: 'Fix bug', description: 'Fix the login bug' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('Fix bug');
		expect(result.output).toContain('#1');
	});

	it('task_get returns task details as JSON', async () => {
		await registry.execute({
			id: 'c1',
			name: 'task_create',
			arguments: { subject: 'My task', description: 'Details' },
		});

		const result = await registry.execute({
			id: 'g1',
			name: 'task_get',
			arguments: { id: '1' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('My task');
	});

	it('task_get returns not-found for missing task', async () => {
		const result = await registry.execute({
			id: 'g2',
			name: 'task_get',
			arguments: { id: '999' },
		});
		expect(result.output).toContain('not found');
	});

	it('task_update changes task status', async () => {
		await registry.execute({
			id: 'c1',
			name: 'task_create',
			arguments: { subject: 'Task', description: 'Desc' },
		});

		const result = await registry.execute({
			id: 'u1',
			name: 'task_update',
			arguments: { id: '1', status: 'in_progress' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('in_progress');
	});

	it('task_list returns formatted task list', async () => {
		await registry.execute({
			id: 'c1',
			name: 'task_create',
			arguments: { subject: 'First', description: 'D1' },
		});
		await registry.execute({
			id: 'c2',
			name: 'task_create',
			arguments: { subject: 'Second', description: 'D2' },
		});

		const result = await registry.execute({
			id: 'l1',
			name: 'task_list',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('First');
		expect(result.output).toContain('Second');
	});

	it('task_list returns empty message when no tasks', async () => {
		const result = await registry.execute({
			id: 'l2',
			name: 'task_list',
			arguments: {},
		});
		expect(result.output).toContain('No tasks');
	});

	it('task_delete removes a task', async () => {
		await registry.execute({
			id: 'c1',
			name: 'task_create',
			arguments: { subject: 'To delete', description: 'D' },
		});

		const result = await registry.execute({
			id: 'd1',
			name: 'task_delete',
			arguments: { id: '1' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('Deleted');
	});
});
