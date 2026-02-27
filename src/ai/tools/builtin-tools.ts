// ---------------------------------------------------------------------------
// Built-in Tool Registration
//
// Registers library, VFS, and task tools with a ToolRegistry.
// Each function is idempotent and safe to call multiple times.
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import type { Library } from '../library/library.js';
import type { TaskList } from '../tasks/types.js';
import type { VirtualFS } from '../vfs/index.js';
import type { ToolDefinition, ToolHandler, ToolRegistry } from './types.js';

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

const registerTool = (
	registry: ToolRegistry,
	definition: ToolDefinition,
	handler: ToolHandler,
): void => {
	registry.register(definition, handler);
};

// ---------------------------------------------------------------------------
// Library tools
// ---------------------------------------------------------------------------

export function registerLibraryTools(
	registry: ToolRegistry,
	library: Library,
): void {
	registerTool(
		registry,
		{
			name: 'library_search',
			description:
				'Search the library for relevant volumes and context. Returns matching volumes ranked by relevance.',
			parameters: {
				query: {
					type: 'string',
					description: 'The search query',
					required: true,
				},
				maxResults: {
					type: 'number',
					description: 'Maximum number of results to return (default: 5)',
				},
			},
			category: 'library',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const query = String(args.query ?? '');
				const maxResults =
					typeof args.maxResults === 'number' ? args.maxResults : 5;
				const results = await library.search(query, maxResults);
				if (results.length === 0) return 'No matching volumes found.';
				return results
					.map(
						(r, i) =>
							`${i + 1}. [${r.volume.metadata.topic ?? 'uncategorized'}] (score: ${r.score.toFixed(2)})\n   ${r.volume.text}`,
					)
					.join('\n\n');
			} catch (err) {
				throw toError(err);
			}
		},
	);

	registerTool(
		registry,
		{
			name: 'library_shelve',
			description: 'Shelve a volume in the library for long-term storage.',
			parameters: {
				text: {
					type: 'string',
					description: 'The text content to shelve',
					required: true,
				},
				topic: {
					type: 'string',
					description: 'Topic category for the volume',
					required: true,
				},
			},
			category: 'library',
		},
		async (args) => {
			try {
				const text = String(args.text ?? '');
				const topic = String(args.topic ?? 'general');
				const id = await library.add(text, { topic });
				return `Shelved volume with ID: ${id}`;
			} catch (err) {
				throw toError(err);
			}
		},
	);

	registerTool(
		registry,
		{
			name: 'library_withdraw',
			description: 'Withdraw a volume from the library by ID.',
			parameters: {
				id: {
					type: 'string',
					description: 'The volume ID to withdraw',
					required: true,
				},
			},
			category: 'library',
			annotations: { destructive: true },
		},
		async (args) => {
			try {
				const id = String(args.id ?? '');
				const deleted = await library.delete(id);
				return deleted
					? `Withdrew volume: ${id}`
					: `Volume not found: ${id}`;
			} catch (err) {
				throw toError(err);
			}
		},
	);
}

/** @deprecated Use registerLibraryTools */
export const registerMemoryTools = registerLibraryTools;

// ---------------------------------------------------------------------------
// VFS tools
// ---------------------------------------------------------------------------

export function registerVFSTools(registry: ToolRegistry, vfs: VirtualFS): void {
	registerTool(
		registry,
		{
			name: 'vfs_read',
			description: 'Read a file from the virtual filesystem sandbox.',
			parameters: {
				path: {
					type: 'string',
					description:
						'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
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

	registerTool(
		registry,
		{
			name: 'vfs_write',
			description: 'Write a file to the virtual filesystem sandbox.',
			parameters: {
				path: {
					type: 'string',
					description:
						'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
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
				const path = String(args.path ?? 'vfs:///');
				const content = String(args.content ?? '');
				vfs.writeFile(path, content, { createParents: true });
				return `Wrote ${Buffer.byteLength(content, 'utf-8')} bytes to ${path}`;
			} catch (err) {
				throw toError(err);
			}
		},
	);

	registerTool(
		registry,
		{
			name: 'vfs_list',
			description:
				'List files and directories in the virtual filesystem sandbox.',
			parameters: {
				path: {
					type: 'string',
					description:
						'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
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

	registerTool(
		registry,
		{
			name: 'vfs_tree',
			description: 'Show a tree view of the virtual filesystem sandbox.',
			parameters: {
				path: {
					type: 'string',
					description:
						'VFS path using vfs:// scheme (e.g. vfs:///hello.js)',
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
}

// ---------------------------------------------------------------------------
// Task tools
// ---------------------------------------------------------------------------

export function registerTaskTools(
	registry: ToolRegistry,
	taskList: TaskList,
): void {
	registerTool(
		registry,
		{
			name: 'task_create',
			description: 'Create a new task to track work. Returns the task ID.',
			parameters: {
				subject: {
					type: 'string',
					description: 'Brief imperative title (e.g. "Fix authentication bug")',
					required: true,
				},
				description: {
					type: 'string',
					description: 'Detailed description of what needs to be done',
					required: true,
				},
				activeForm: {
					type: 'string',
					description:
						'Present continuous form shown while in progress (e.g. "Fixing authentication bug")',
				},
			},
			category: 'task',
		},
		async (args) => {
			try {
				const task = taskList.create({
					subject: String(args.subject ?? ''),
					description: String(args.description ?? ''),
					activeForm:
						typeof args.activeForm === 'string' ? args.activeForm : undefined,
				});
				return `Created task #${task.id}: ${task.subject}`;
			} catch (err) {
				throw toError(err);
			}
		},
	);

	registerTool(
		registry,
		{
			name: 'task_get',
			description: 'Get full details of a task by ID.',
			parameters: {
				id: {
					type: 'string',
					description: 'The task ID',
					required: true,
				},
			},
			category: 'task',
			annotations: { readOnly: true },
		},
		async (args) => {
			const task = taskList.get(String(args.id ?? ''));
			if (!task) return `Task not found: ${args.id}`;
			return JSON.stringify(task, null, 2);
		},
	);

	registerTool(
		registry,
		{
			name: 'task_update',
			description:
				'Update a task (status, subject, description, dependencies).',
			parameters: {
				id: {
					type: 'string',
					description: 'The task ID',
					required: true,
				},
				status: {
					type: 'string',
					description: 'New status: "pending", "in_progress", or "completed"',
				},
				subject: {
					type: 'string',
					description: 'New subject',
				},
				description: {
					type: 'string',
					description: 'New description',
				},
				activeForm: {
					type: 'string',
					description: 'New active form text',
				},
				addBlocks: {
					type: 'string',
					description: 'Comma-separated task IDs that this task blocks',
				},
				addBlockedBy: {
					type: 'string',
					description: 'Comma-separated task IDs that block this task',
				},
			},
			category: 'task',
		},
		async (args) => {
			try {
				const id = String(args.id ?? '');
				const updates: Record<string, unknown> = {};
				if (typeof args.status === 'string') updates.status = args.status;
				if (typeof args.subject === 'string') updates.subject = args.subject;
				if (typeof args.description === 'string')
					updates.description = args.description;
				if (typeof args.activeForm === 'string')
					updates.activeForm = args.activeForm;
				if (typeof args.addBlocks === 'string')
					updates.addBlocks = args.addBlocks
						.split(',')
						.map((s: string) => s.trim());
				if (typeof args.addBlockedBy === 'string')
					updates.addBlockedBy = args.addBlockedBy
						.split(',')
						.map((s: string) => s.trim());

				const task = taskList.update(id, updates);
				if (!task) return `Task not found: ${id}`;
				return `Updated task #${task.id}: ${task.subject} [${task.status}]`;
			} catch (err) {
				throw toError(err);
			}
		},
	);

	registerTool(
		registry,
		{
			name: 'task_delete',
			description: 'Delete a task by ID.',
			parameters: {
				id: {
					type: 'string',
					description: 'The task ID',
					required: true,
				},
			},
			category: 'task',
			annotations: { destructive: true },
		},
		async (args) => {
			const deleted = taskList.delete(String(args.id ?? ''));
			return deleted
				? `Deleted task #${args.id}`
				: `Task not found: ${args.id}`;
		},
	);

	registerTool(
		registry,
		{
			name: 'task_list',
			description:
				'List all tasks with their status, subject, and dependencies.',
			parameters: {},
			category: 'task',
			annotations: { readOnly: true },
		},
		async () => {
			const tasks = taskList.list();
			if (tasks.length === 0) return 'No tasks.';
			return tasks
				.map((t) => {
					let line = `#${t.id} [${t.status}] ${t.subject}`;
					if (t.blockedBy.length > 0)
						line += ` (blocked by: ${t.blockedBy.join(', ')})`;
					if (t.blocks.length > 0) line += ` (blocks: ${t.blocks.join(', ')})`;
					return line;
				})
				.join('\n');
		},
	);
}
