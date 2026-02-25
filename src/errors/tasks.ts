// ---------------------------------------------------------------------------
// Task List Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createTaskError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createSimseError(message, {
		name: options.name ?? 'TaskError',
		code: options.code ?? 'TASK_ERROR',
		statusCode: 400,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createTaskNotFoundError = (
	taskId: string,
	options: { cause?: unknown } = {},
): SimseError =>
	createTaskError(`Task not found: "${taskId}"`, {
		name: 'TaskNotFoundError',
		code: 'TASK_NOT_FOUND',
		cause: options.cause,
		metadata: { taskId },
	});

export const createTaskCircularDependencyError = (
	taskId: string,
	dependencyId: string,
	options: { cause?: unknown } = {},
): SimseError =>
	createTaskError(
		`Circular dependency detected: task "${taskId}" and "${dependencyId}"`,
		{
			name: 'TaskCircularDependencyError',
			code: 'TASK_CIRCULAR_DEPENDENCY',
			cause: options.cause,
			metadata: { taskId, dependencyId },
		},
	);

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isTaskError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code.startsWith('TASK_');

export const isTaskNotFoundError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'TASK_NOT_FOUND';

export const isTaskCircularDependencyError = (
	value: unknown,
): value is SimseError =>
	isSimseError(value) && value.code === 'TASK_CIRCULAR_DEPENDENCY';
