// ---------------------------------------------------------------------------
// Tool Registry Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createToolError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createSimseError(message, {
		name: options.name ?? 'ToolError',
		code: options.code ?? 'TOOL_ERROR',
		statusCode: 500,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createToolNotFoundError = (
	toolName: string,
	options: { cause?: unknown } = {},
): SimseError =>
	createToolError(`Tool not found: "${toolName}"`, {
		name: 'ToolNotFoundError',
		code: 'TOOL_NOT_FOUND',
		cause: options.cause,
		metadata: { toolName },
	});

export const createToolExecutionError = (
	toolName: string,
	message: string,
	options: { cause?: unknown } = {},
): SimseError =>
	createToolError(`Tool "${toolName}" execution failed: ${message}`, {
		name: 'ToolExecutionError',
		code: 'TOOL_EXECUTION_ERROR',
		cause: options.cause,
		metadata: { toolName },
	});

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isToolError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code.startsWith('TOOL_');

export const isToolNotFoundError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'TOOL_NOT_FOUND';

export const isToolExecutionError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'TOOL_EXECUTION_ERROR';
