// ---------------------------------------------------------------------------
// Task List Types
// ---------------------------------------------------------------------------

export type TaskStatus = 'pending' | 'in_progress' | 'completed';

export interface TaskItem {
	readonly id: string;
	readonly subject: string;
	readonly description: string;
	readonly status: TaskStatus;
	readonly activeForm?: string;
	readonly owner?: string;
	readonly metadata?: Readonly<Record<string, unknown>>;
	readonly blocks: readonly string[];
	readonly blockedBy: readonly string[];
	readonly createdAt: number;
	readonly updatedAt: number;
}

export interface TaskCreateInput {
	readonly subject: string;
	readonly description: string;
	readonly activeForm?: string;
	readonly owner?: string;
	readonly metadata?: Record<string, unknown>;
}

export interface TaskUpdateInput {
	readonly status?: TaskStatus;
	readonly subject?: string;
	readonly description?: string;
	readonly activeForm?: string;
	readonly owner?: string;
	readonly metadata?: Record<string, unknown>;
	readonly addBlocks?: readonly string[];
	readonly addBlockedBy?: readonly string[];
}

export interface TaskListOptions {
	/** Maximum number of tasks allowed. Default: 100. */
	readonly maxTasks?: number;
	/** Callback fired whenever a task changes. */
	readonly onTaskChange?: (
		task: TaskItem,
		action: 'create' | 'update' | 'delete',
	) => void;
}

export interface TaskList {
	readonly create: (input: TaskCreateInput) => TaskItem;
	readonly get: (id: string) => TaskItem | undefined;
	readonly update: (id: string, input: TaskUpdateInput) => TaskItem | undefined;
	readonly delete: (id: string) => boolean;
	readonly list: () => readonly TaskItem[];
	/** List tasks that are pending, have no owner, and are not blocked. */
	readonly listAvailable: () => readonly TaskItem[];
	/** List tasks that are blocked by unresolved dependencies. */
	readonly getBlocked: () => readonly TaskItem[];
	readonly clear: () => void;
	readonly taskCount: number;
}
