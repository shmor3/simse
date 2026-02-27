import { beforeEach, describe, expect, it, mock } from 'bun:test';
import type { MemoryManager } from '../src/ai/memory/memory.js';
import type { TaskList } from '../src/ai/tasks/types.js';
import {
	registerMemoryTools,
	registerTaskTools,
	registerVFSTools,
} from '../src/ai/tools/builtin-tools.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import type { ToolRegistry } from '../src/ai/tools/types.js';
import { createVirtualFS } from '../src/ai/vfs/index.js';
import { createSilentLogger } from './utils/mocks.js';

// ---------------------------------------------------------------------------
// Mock factories
// ---------------------------------------------------------------------------

function createMockMemoryManager(): MemoryManager {
	return {
		initialize: mock(async () => {}),
		dispose: mock(async () => {}),
		add: mock(async () => 'mock-id'),
		addBatch: mock(async () => []),
		search: mock(async () => [
			{
				entry: {
					id: '1',
					text: 'result text',
					embedding: [0.1],
					metadata: { topic: 'test' },
					timestamp: Date.now(),
				},
				score: 0.9,
			},
		]),
		textSearch: mock(() => []),
		filterByMetadata: mock(() => []),
		filterByDateRange: mock(() => []),
		advancedSearch: mock(async () => []),
		getById: mock(() => undefined),
		getAll: mock(() => []),
		getTopics: mock(() => []),
		filterByTopic: mock(() => []),
		recommend: mock(async () => []),
		findDuplicates: mock(() => []),
		checkDuplicate: mock(async () => ({ isDuplicate: false })),
		summarize: mock(async () => ({
			summaryId: 's',
			summaryText: '',
			sourceIds: [],
			deletedOriginals: false,
		})),
		setTextGenerator: mock(() => {}),
		delete: mock(async () => true),
		deleteBatch: mock(async () => 0),
		clear: mock(async () => {}),
		learningProfile: undefined,
		size: 1,
		isInitialized: true,
		isDirty: false,
		embeddingAgent: undefined,
	} as unknown as MemoryManager;
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
// Memory Tools
// ---------------------------------------------------------------------------

describe('registerMemoryTools', () => {
	let registry: ToolRegistry;
	let memoryManager: MemoryManager;

	beforeEach(() => {
		registry = createToolRegistry({ logger: createSilentLogger() });
		memoryManager = createMockMemoryManager();
		registerMemoryTools(registry, memoryManager);
	});

	it('registers memory_search and memory_add tools', () => {
		expect(registry.toolNames).toContain('memory_search');
		expect(registry.toolNames).toContain('memory_add');
	});

	it('memory_search returns formatted results', async () => {
		const result = await registry.execute({
			id: 'call-1',
			name: 'memory_search',
			arguments: { query: 'test' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('result text');
		expect(result.output).toContain('0.90');
	});

	it('memory_search returns no-match message for empty results', async () => {
		(memoryManager.search as ReturnType<typeof mock>).mockResolvedValue([]);
		const result = await registry.execute({
			id: 'call-1',
			name: 'memory_search',
			arguments: { query: 'nothing' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('No matching');
	});

	it('memory_add stores text and returns ID', async () => {
		const result = await registry.execute({
			id: 'call-2',
			name: 'memory_add',
			arguments: { text: 'remember this', topic: 'notes' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('mock-id');
	});

	it('memory_add wraps errors via toError', async () => {
		(memoryManager.add as ReturnType<typeof mock>).mockRejectedValue(
			'raw string error',
		);
		const result = await registry.execute({
			id: 'call-3',
			name: 'memory_add',
			arguments: { text: 'fail', topic: 'x' },
		});
		expect(result.isError).toBe(true);
	});
});

// ---------------------------------------------------------------------------
// VFS Tools
// ---------------------------------------------------------------------------

describe('registerVFSTools', () => {
	let registry: ToolRegistry;

	beforeEach(() => {
		registry = createToolRegistry({ logger: createSilentLogger() });
		const vfs = createVirtualFS();
		registerVFSTools(registry, vfs);
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
