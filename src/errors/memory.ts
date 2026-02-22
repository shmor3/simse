// ---------------------------------------------------------------------------
// Memory / Vector Store / Embedding Errors
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

export const createMemoryError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): SimseError =>
	createSimseError(message, {
		name: options.name ?? 'MemoryError',
		code: options.code ?? 'MEMORY_ERROR',
		statusCode: 500,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createEmbeddingError = (
	message: string,
	options: { cause?: unknown; model?: string } = {},
): SimseError =>
	createMemoryError(message, {
		name: 'EmbeddingError',
		code: 'EMBEDDING_ERROR',
		cause: options.cause,
		metadata: options.model ? { model: options.model } : {},
	});

export const createVectorStoreCorruptionError = (
	storePath: string,
	options: { cause?: unknown } = {},
): SimseError & { readonly storePath: string } => {
	const err = createMemoryError(
		`Vector store file is corrupted: ${storePath}`,
		{
			name: 'VectorStoreCorruptionError',
			code: 'VECTOR_STORE_CORRUPT',
			cause: options.cause,
			metadata: { storePath },
		},
	) as SimseError & { readonly storePath: string };

	Object.defineProperty(err, 'storePath', {
		value: storePath,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createVectorStoreIOError = (
	storePath: string,
	operation: 'read' | 'write',
	options: { cause?: unknown } = {},
): SimseError & { readonly storePath: string } => {
	const err = createMemoryError(
		`Failed to ${operation} vector store: ${storePath}`,
		{
			name: 'VectorStoreIOError',
			code: 'VECTOR_STORE_IO',
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
// Type Guards
// ---------------------------------------------------------------------------

export const isMemoryError = (value: unknown): value is SimseError =>
	isSimseError(value) &&
	(value.code.startsWith('MEMORY_') ||
		value.code.startsWith('EMBEDDING_') ||
		value.code.startsWith('VECTOR_STORE_'));

export const isEmbeddingError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'EMBEDDING_ERROR';

export const isVectorStoreCorruptionError = (
	value: unknown,
): value is SimseError & { readonly storePath: string } =>
	isSimseError(value) && value.code === 'VECTOR_STORE_CORRUPT';

export const isVectorStoreIOError = (
	value: unknown,
): value is SimseError & { readonly storePath: string } =>
	isSimseError(value) && value.code === 'VECTOR_STORE_IO';
