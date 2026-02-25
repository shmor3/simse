// ---------------------------------------------------------------------------
// Task List
//
// TodoRead/TodoWrite-style task tracking for agentic sessions.
// Auto-incrementing IDs, circular dependency detection, and
// configurable limits.
// ---------------------------------------------------------------------------

import {
	createTaskCircularDependencyError,
	createTaskError,
} from '../../errors/tasks.js';
import type {
	TaskCreateInput,
	TaskItem,
	TaskList,
	TaskListOptions,
	TaskUpdateInput,
} from './types.js';

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createTaskList(options?: TaskListOptions): TaskList {
	const maxTasks = options?.maxTasks ?? 100;
	const onTaskChange = options?.onTaskChange;
	const tasks = new Map<string, TaskItem>();
	let nextId = 1;

	// -----------------------------------------------------------------------
	// Helpers
	// -----------------------------------------------------------------------

	const wouldCreateCycle = (blockerId: string, blockedId: string): boolean => {
		// Check if adding "blockerId blocks blockedId" would create a cycle.
		// A cycle exists if blockedId can already reach blockerId via blocks edges
		// (i.e., blockedId already transitively blocks blockerId).
		const visited = new Set<string>();
		const queue = [blockedId];

		while (queue.length > 0) {
			const current = queue.pop() as string;
			if (current === blockerId) return true;
			if (visited.has(current)) continue;
			visited.add(current);

			const task = tasks.get(current);
			if (task) {
				for (const dep of task.blocks) {
					queue.push(dep);
				}
			}
		}

		return false;
	};

	const buildTask = (id: string, input: TaskCreateInput): TaskItem => {
		const now = Date.now();
		return Object.freeze({
			id,
			subject: input.subject,
			description: input.description,
			status: 'pending' as const,
			activeForm: input.activeForm,
			owner: input.owner,
			metadata: input.metadata
				? Object.freeze({ ...input.metadata })
				: undefined,
			blocks: Object.freeze([]),
			blockedBy: Object.freeze([]),
			createdAt: now,
			updatedAt: now,
		});
	};

	// -----------------------------------------------------------------------
	// Public interface
	// -----------------------------------------------------------------------

	const create = (input: TaskCreateInput): TaskItem => {
		if (tasks.size >= maxTasks) {
			throw createTaskError(
				`Task limit reached: maximum ${maxTasks} tasks allowed`,
				{ metadata: { maxTasks } },
			);
		}

		const id = String(nextId++);
		const task = buildTask(id, input);
		tasks.set(id, task);
		onTaskChange?.(task, 'create');
		return task;
	};

	const get = (id: string): TaskItem | undefined => {
		return tasks.get(id);
	};

	const update = (id: string, input: TaskUpdateInput): TaskItem | undefined => {
		const existing = tasks.get(id);
		if (!existing) return undefined;

		// Process dependency additions
		const blocks = [...existing.blocks];
		const blockedBy = [...existing.blockedBy];

		if (input.addBlocks) {
			for (const targetId of input.addBlocks) {
				if (targetId === id) continue; // Can't block self
				if (blocks.includes(targetId)) continue; // Already blocking

				// Check for circular dependency: id blocks targetId
				if (wouldCreateCycle(id, targetId)) {
					throw createTaskCircularDependencyError(id, targetId);
				}

				blocks.push(targetId);

				// Add reciprocal blockedBy on the target task
				const target = tasks.get(targetId);
				if (target && !target.blockedBy.includes(id)) {
					const updatedTarget = Object.freeze({
						...target,
						blockedBy: Object.freeze([...target.blockedBy, id]),
						updatedAt: Date.now(),
					});
					tasks.set(targetId, updatedTarget);
				}
			}
		}

		if (input.addBlockedBy) {
			for (const depId of input.addBlockedBy) {
				if (depId === id) continue; // Can't be blocked by self
				if (blockedBy.includes(depId)) continue; // Already blocked by

				// Check for circular dependency: depId blocks id
				if (wouldCreateCycle(depId, id)) {
					throw createTaskCircularDependencyError(id, depId);
				}

				blockedBy.push(depId);

				// Add reciprocal blocks on the dependency task
				const dep = tasks.get(depId);
				if (dep && !dep.blocks.includes(id)) {
					const updatedDep = Object.freeze({
						...dep,
						blocks: Object.freeze([...dep.blocks, id]),
						updatedAt: Date.now(),
					});
					tasks.set(depId, updatedDep);
				}
			}
		}

		// When a task is completed, remove it from other tasks' blockedBy
		if (input.status === 'completed' && existing.status !== 'completed') {
			for (const blockedId of blocks) {
				const blocked = tasks.get(blockedId);
				if (blocked) {
					const updatedBlocked = Object.freeze({
						...blocked,
						blockedBy: Object.freeze(
							blocked.blockedBy.filter((bId) => bId !== id),
						),
						updatedAt: Date.now(),
					});
					tasks.set(blockedId, updatedBlocked);
				}
			}
		}

		const merged = input.metadata
			? { ...(existing.metadata ?? {}), ...input.metadata }
			: existing.metadata;

		// Filter out null metadata keys
		const cleanedMetadata = merged
			? Object.fromEntries(Object.entries(merged).filter(([, v]) => v !== null))
			: undefined;

		const updated = Object.freeze({
			...existing,
			...(input.status !== undefined && { status: input.status }),
			...(input.subject !== undefined && { subject: input.subject }),
			...(input.description !== undefined && {
				description: input.description,
			}),
			...(input.activeForm !== undefined && {
				activeForm: input.activeForm,
			}),
			...(input.owner !== undefined && { owner: input.owner }),
			...(cleanedMetadata && {
				metadata: Object.freeze(cleanedMetadata),
			}),
			blocks: Object.freeze(blocks),
			blockedBy: Object.freeze(blockedBy),
			updatedAt: Date.now(),
		});

		tasks.set(id, updated);
		onTaskChange?.(updated, 'update');
		return updated;
	};

	const deleteTask = (id: string): boolean => {
		const existing = tasks.get(id);
		if (!existing) return false;

		// Remove from other tasks' blocks/blockedBy
		for (const [, task] of tasks) {
			if (task.id === id) continue;

			let needsUpdate = false;
			let newBlocks = task.blocks;
			let newBlockedBy = task.blockedBy;

			if (task.blocks.includes(id)) {
				newBlocks = Object.freeze(task.blocks.filter((bId) => bId !== id));
				needsUpdate = true;
			}
			if (task.blockedBy.includes(id)) {
				newBlockedBy = Object.freeze(
					task.blockedBy.filter((bId) => bId !== id),
				);
				needsUpdate = true;
			}

			if (needsUpdate) {
				tasks.set(
					task.id,
					Object.freeze({
						...task,
						blocks: newBlocks,
						blockedBy: newBlockedBy,
						updatedAt: Date.now(),
					}),
				);
			}
		}

		tasks.delete(id);
		onTaskChange?.(existing, 'delete');
		return true;
	};

	const list = (): readonly TaskItem[] => {
		return Object.freeze([...tasks.values()]);
	};

	const listAvailable = (): readonly TaskItem[] => {
		return Object.freeze(
			[...tasks.values()].filter(
				(t) =>
					t.status === 'pending' &&
					!t.owner &&
					t.blockedBy.every(
						(depId) => tasks.get(depId)?.status === 'completed',
					),
			),
		);
	};

	const getBlocked = (): readonly TaskItem[] => {
		return Object.freeze(
			[...tasks.values()].filter(
				(t) =>
					t.status !== 'completed' &&
					t.blockedBy.some((depId) => tasks.get(depId)?.status !== 'completed'),
			),
		);
	};

	const clear = (): void => {
		tasks.clear();
		nextId = 1;
	};

	return Object.freeze({
		create,
		get,
		update,
		delete: deleteTask,
		list,
		listAvailable,
		getBlocked,
		clear,
		get taskCount() {
			return tasks.size;
		},
	});
}
