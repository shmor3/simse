// ---------------------------------------------------------------------------
// Library / Stacks / Embedding Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createLibraryError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createSimseError(message, {
		name: options.name ?? 'LibraryError',
		code: options.code ?? 'LIBRARY_ERROR',
		statusCode: 500,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createEmbeddingError = (
	message: string,
	options: { cause?: unknown; model?: string } = {},
): SimseError =>
	createLibraryError(message, {
		name: 'EmbeddingError',
		code: 'EMBEDDING_ERROR',
		cause: options.cause,
		metadata: options.model ? { model: options.model } : {},
	});

export const createStacksCorruptionError = (
	storePath: string,
	options: { cause?: unknown } = {},
): SimseError & { readonly storePath: string } => {
	const err = createLibraryError(`Stacks file is corrupted: ${storePath}`, {
		name: 'StacksCorruptionError',
		code: 'STACKS_CORRUPT',
		cause: options.cause,
		metadata: { storePath },
	}) as SimseError & { readonly storePath: string };

	Object.defineProperty(err, 'storePath', {
		value: storePath,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createStacksIOError = (
	storePath: string,
	operation: 'read' | 'write',
	options: { cause?: unknown } = {},
): SimseError & { readonly storePath: string } => {
	const err = createLibraryError(
		`Failed to ${operation} stacks: ${storePath}`,
		{
			name: 'StacksIOError',
			code: 'STACKS_IO',
			cause: options.cause,
			metadata: { storePath, operation },
		},
	) as SimseError & { readonly storePath: string };

	Object.defineProperty(err, 'storePath', {
		value: storePath,
		writable: false,
		enumerable: true,
	});

	return err;
};

// ---------------------------------------------------------------------------
// Convenience alias (some internal code uses createStacksError)
// ---------------------------------------------------------------------------

export const createStacksError = (
	message: string,
	options: {
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createLibraryError(message, {
		name: 'StacksError',
		code: options.code ?? 'STACKS_ERROR',
		cause: options.cause,
		metadata: options.metadata,
	});

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isLibraryError = (value: unknown): value is SimseError =>
	isSimseError(value) &&
	(value.code.startsWith('LIBRARY_') ||
		value.code.startsWith('EMBEDDING_') ||
		value.code.startsWith('STACKS_'));

export const isStacksError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code.startsWith('STACKS_');

export const isEmbeddingError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'EMBEDDING_ERROR';

export const isStacksCorruptionError = (
	value: unknown,
): value is SimseError & { readonly storePath: string } =>
	isSimseError(value) && value.code === 'STACKS_CORRUPT';

export const isStacksIOError = (
	value: unknown,
): value is SimseError & { readonly storePath: string } =>
	isSimseError(value) && value.code === 'STACKS_IO';
