// ---------------------------------------------------------------------------
// Virtual Filesystem Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createVFSError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		statusCode?: number;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createSimseError(message, {
		name: options.name ?? 'VFSError',
		code: options.code ?? 'VFS_ERROR',
		statusCode: options.statusCode ?? 400,
		cause: options.cause,
		metadata: options.metadata,
	});

// ---------------------------------------------------------------------------
// Type Guard
// ---------------------------------------------------------------------------

export const isVFSError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code.startsWith('VFS_');
