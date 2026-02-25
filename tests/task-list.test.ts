import { describe, expect, it } from 'bun:test';
import { createTaskList } from '../src/ai/tasks/task-list.js';

describe('createTaskList', () => {
	it('returns a frozen object', () => {
		const tl = createTaskList();
		expect(Object.isFrozen(tl)).toBe(true);
	});

	it('starts with zero tasks', () => {
		const tl = createTaskList();
		expect(tl.taskCount).toBe(0);
		expect(tl.list()).toEqual([]);
	});

	it('creates a task with auto-incrementing ID', () => {
		const tl = createTaskList();
		const t1 = tl.create({ subject: 'First', description: 'desc 1' });
		const t2 = tl.create({ subject: 'Second', description: 'desc 2' });
		expect(t1.id).toBe('1');
		expect(t2.id).toBe('2');
		expect(tl.taskCount).toBe(2);
	});

	it('creates a task with pending status', () => {
		const tl = createTaskList();
		const task = tl.create({ subject: 'Task', description: 'desc' });
		expect(task.status).toBe('pending');
	});

	it('creates a frozen task', () => {
		const tl = createTaskList();
		const task = tl.create({ subject: 'Task', description: 'desc' });
		expect(Object.isFrozen(task)).toBe(true);
	});

	it('creates a task with all optional fields', () => {
		const tl = createTaskList();
		const task = tl.create({
			subject: 'Full task',
			description: 'full description',
			activeForm: 'Working on task',
			owner: 'agent-1',
			metadata: { priority: 'high' },
		});
		expect(task.activeForm).toBe('Working on task');
		expect(task.owner).toBe('agent-1');
		expect(task.metadata?.priority).toBe('high');
	});

	it('get returns the task by ID', () => {
		const tl = createTaskList();
		const created = tl.create({ subject: 'Task', description: 'desc' });
		const fetched = tl.get(created.id);
		expect(fetched).toEqual(created);
	});

	it('get returns undefined for unknown ID', () => {
		const tl = createTaskList();
		expect(tl.get('999')).toBeUndefined();
	});

	it('update changes task fields', () => {
		const tl = createTaskList();
		tl.create({ subject: 'Original', description: 'original desc' });
		const updated = tl.update('1', {
			subject: 'Updated',
			status: 'in_progress',
		});
		expect(updated?.subject).toBe('Updated');
		expect(updated?.status).toBe('in_progress');
	});

	it('update returns undefined for unknown ID', () => {
		const tl = createTaskList();
		expect(tl.update('999', { status: 'completed' })).toBeUndefined();
	});

	it('update merges metadata', () => {
		const tl = createTaskList();
		tl.create({
			subject: 'Task',
			description: 'desc',
			metadata: { a: 1, b: 2 },
		});
		const updated = tl.update('1', { metadata: { b: 3, c: 4 } });
		expect(updated?.metadata).toEqual({ a: 1, b: 3, c: 4 });
	});

	it('update removes null metadata keys', () => {
		const tl = createTaskList();
		tl.create({
			subject: 'Task',
			description: 'desc',
			metadata: { a: 1, b: 2 },
		});
		const updated = tl.update('1', {
			metadata: { b: null } as any,
		});
		expect(updated?.metadata).toEqual({ a: 1 });
	});

	it('delete removes a task', () => {
		const tl = createTaskList();
		tl.create({ subject: 'Task', description: 'desc' });
		expect(tl.delete('1')).toBe(true);
		expect(tl.taskCount).toBe(0);
		expect(tl.get('1')).toBeUndefined();
	});

	it('delete returns false for unknown ID', () => {
		const tl = createTaskList();
		expect(tl.delete('999')).toBe(false);
	});

	it('list returns all tasks', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		const tasks = tl.list();
		expect(tasks.length).toBe(2);
		expect(tasks.map((t) => t.subject)).toEqual(['A', 'B']);
	});

	it('clear removes all tasks and resets IDs', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.clear();
		expect(tl.taskCount).toBe(0);
		const t = tl.create({ subject: 'C', description: 'c' });
		expect(t.id).toBe('1');
	});

	// --- Dependencies ---

	it('addBlocks creates a dependency', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.update('1', { addBlocks: ['2'] });

		const a = tl.get('1')!;
		const b = tl.get('2')!;
		expect(a.blocks).toContain('2');
		expect(b.blockedBy).toContain('1');
	});

	it('addBlockedBy creates a dependency', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.update('2', { addBlockedBy: ['1'] });

		const a = tl.get('1')!;
		const b = tl.get('2')!;
		expect(a.blocks).toContain('2');
		expect(b.blockedBy).toContain('1');
	});

	it('completing a task unblocks dependents', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.update('1', { addBlocks: ['2'] });

		expect(tl.get('2')!.blockedBy).toContain('1');
		tl.update('1', { status: 'completed' });
		expect(tl.get('2')!.blockedBy).not.toContain('1');
	});

	it('deleting a task removes it from dependencies', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.update('1', { addBlocks: ['2'] });

		tl.delete('1');
		expect(tl.get('2')!.blockedBy).not.toContain('1');
	});

	it('detects circular dependencies via addBlocks', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.update('1', { addBlocks: ['2'] });

		expect(() => tl.update('2', { addBlocks: ['1'] })).toThrow(/[Cc]ircular/);
	});

	it('detects circular dependencies via addBlockedBy', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.update('2', { addBlockedBy: ['1'] });

		expect(() => tl.update('1', { addBlockedBy: ['2'] })).toThrow(
			/[Cc]ircular/,
		);
	});

	it('ignores self-referencing dependencies', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.update('1', { addBlocks: ['1'] });
		expect(tl.get('1')!.blocks).not.toContain('1');
	});

	it('ignores duplicate dependencies', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.update('1', { addBlocks: ['2'] });
		tl.update('1', { addBlocks: ['2'] });
		expect(tl.get('1')!.blocks.filter((b) => b === '2').length).toBe(1);
	});

	// --- listAvailable / getBlocked ---

	it('listAvailable returns unblocked pending tasks without owner', () => {
		const tl = createTaskList();
		tl.create({ subject: 'Available', description: 'a' });
		tl.create({ subject: 'Owned', description: 'b', owner: 'agent-1' });
		tl.create({ subject: 'Blocked', description: 'c' });
		tl.update('3', { addBlockedBy: ['1'] });

		const available = tl.listAvailable();
		expect(available.length).toBe(1);
		expect(available[0].subject).toBe('Available');
	});

	it('getBlocked returns tasks with unresolved dependencies', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.update('2', { addBlockedBy: ['1'] });

		const blocked = tl.getBlocked();
		expect(blocked.length).toBe(1);
		expect(blocked[0].id).toBe('2');
	});

	it('getBlocked excludes completed tasks', () => {
		const tl = createTaskList();
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		tl.update('2', { addBlockedBy: ['1'] });
		tl.update('2', { status: 'completed' });

		expect(tl.getBlocked().length).toBe(0);
	});

	// --- Limits ---

	it('throws when maxTasks is exceeded', () => {
		const tl = createTaskList({ maxTasks: 2 });
		tl.create({ subject: 'A', description: 'a' });
		tl.create({ subject: 'B', description: 'b' });
		expect(() => tl.create({ subject: 'C', description: 'c' })).toThrow(
			/limit/i,
		);
	});

	// --- Callbacks ---

	it('calls onTaskChange on create', () => {
		const changes: Array<{ action: string; id: string }> = [];
		const tl = createTaskList({
			onTaskChange: (task, action) => changes.push({ action, id: task.id }),
		});
		tl.create({ subject: 'A', description: 'a' });
		expect(changes).toEqual([{ action: 'create', id: '1' }]);
	});

	it('calls onTaskChange on update', () => {
		const changes: Array<{ action: string; id: string }> = [];
		const tl = createTaskList({
			onTaskChange: (task, action) => changes.push({ action, id: task.id }),
		});
		tl.create({ subject: 'A', description: 'a' });
		tl.update('1', { status: 'in_progress' });
		expect(changes).toEqual([
			{ action: 'create', id: '1' },
			{ action: 'update', id: '1' },
		]);
	});

	it('calls onTaskChange on delete', () => {
		const changes: Array<{ action: string; id: string }> = [];
		const tl = createTaskList({
			onTaskChange: (task, action) => changes.push({ action, id: task.id }),
		});
		tl.create({ subject: 'A', description: 'a' });
		tl.delete('1');
		expect(changes).toEqual([
			{ action: 'create', id: '1' },
			{ action: 'delete', id: '1' },
		]);
	});

	// --- Timestamps ---

	it('sets createdAt and updatedAt on create', () => {
		const tl = createTaskList();
		const before = Date.now();
		const task = tl.create({ subject: 'A', description: 'a' });
		const after = Date.now();
		expect(task.createdAt).toBeGreaterThanOrEqual(before);
		expect(task.createdAt).toBeLessThanOrEqual(after);
		expect(task.updatedAt).toBe(task.createdAt);
	});

	it('updates updatedAt on update', async () => {
		const tl = createTaskList();
		const task = tl.create({ subject: 'A', description: 'a' });
		// Small delay to ensure timestamp difference
		await new Promise((r) => setTimeout(r, 5));
		const updated = tl.update('1', { subject: 'B' });
		expect(updated!.updatedAt).toBeGreaterThan(task.createdAt);
	});
});
