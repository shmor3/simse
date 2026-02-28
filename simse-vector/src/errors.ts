// ---------------------------------------------------------------------------
// VectorError — self-contained error hierarchy for simse-vector
// ---------------------------------------------------------------------------

/**
 * Structured error interface for simse-vector.
 * Compatible with simse's SimseError shape for seamless integration.
 */
export interface VectorError extends Error {
	readonly code: string;
	readonly statusCode: number;
	readonly metadata: Record<string, unknown>;
	readonly toJSON: () => Record<string, unknown>;
}

export interface VectorErrorOptions {
	readonly name?: string;
	readonly code?: string;
	readonly statusCode?: number;
	readonly cause?: unknown;
	readonly metadata?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------
// Base factory
// ---------------------------------------------------------------------------

export const createVectorError = (
	message: string,
	options: VectorErrorOptions = {},
): VectorError => {
	const err = new Error(message, { cause: options.cause }) as Error & {
		code: string;
		statusCode: number;
		metadata: Record<string, unknown>;
		toJSON: () => Record<string, unknown>;
	};

	err.name = options.name ?? 'VectorError';
	const code = options.code ?? 'VECTOR_ERROR';
	const statusCode = options.statusCode ?? 500;
	const metadata = options.metadata ?? {};

	Object.defineProperties(err, {
		code: { value: code, writable: false, enumerable: true },
		statusCode: { value: statusCode, writable: false, enumerable: true },
		metadata: { value: metadata, writable: false, enumerable: true },
		toJSON: {
			value: (): Record<string, unknown> => ({
				name: err.name,
				code,
				message: err.message,
				statusCode,
				metadata,
				cause:
					err.cause &&
					typeof err.cause === 'object' &&
					typeof (err.cause as Record<string, unknown>).message === 'string' &&
					typeof (err.cause as Record<string, unknown>).name === 'string'
						? {
								name: (err.cause as Record<string, unknown>).name,
								message: (err.cause as Record<string, unknown>).message,
							}
						: err.cause,
				stack: err.stack,
			}),
			writable: false,
			enumerable: false,
		},
	});

	return err as VectorError;
};

// ---------------------------------------------------------------------------
// Base type guard
// ---------------------------------------------------------------------------

export const isVectorError = (value: unknown): value is VectorError =>
	value instanceof Error &&
	typeof (value as unknown as Record<string, unknown>).code === 'string' &&
	typeof (value as unknown as Record<string, unknown>).statusCode ===
		'number' &&
	typeof (value as unknown as Record<string, unknown>).toJSON === 'function';

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

export const toError = (value: unknown): Error => {
	if (value instanceof Error) return value;
	if (typeof value === 'string') return new Error(value);
	return new Error(String(value));
};

// ---------------------------------------------------------------------------
// Library errors
// ---------------------------------------------------------------------------

export const createLibraryError = (
	message: string,
	options: {
		name?: string;
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): VectorError =>
	createVectorError(message, {
		name: options.name ?? 'LibraryError',
		code: options.code ?? 'LIBRARY_ERROR',
		statusCode: 500,
		cause: options.cause,
		metadata: options.metadata,
	});

export const createEmbeddingError = (
	message: string,
	options: { cause?: unknown; model?: string } = {},
): VectorError =>
	createLibraryError(message, {
		name: 'EmbeddingError',
		code: 'EMBEDDING_ERROR',
		cause: options.cause,
		metadata: options.model ? { model: options.model } : {},
	});

export const createStacksCorruptionError = (
	storePath: string,
	options: { cause?: unknown } = {},
): VectorError & { readonly storePath: string } => {
	const err = createLibraryError(`Stacks file is corrupted: ${storePath}`, {
		name: 'StacksCorruptionError',
		code: 'STACKS_CORRUPT',
		cause: options.cause,
		metadata: { storePath },
	}) as VectorError & { readonly storePath: string };

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
): VectorError & { readonly storePath: string } => {
	const err = createLibraryError(
		`Failed to ${operation} stacks: ${storePath}`,
		{
			name: 'StacksIOError',
			code: 'STACKS_IO',
			cause: options.cause,
			metadata: { storePath, operation },
		},
	) as VectorError & { readonly storePath: string };

	Object.defineProperty(err, 'storePath', {
		value: storePath,
		writable: false,
		enumerable: true,
	});

	return err;
};

export const createStacksError = (
	message: string,
	options: {
		code?: string;
		cause?: unknown;
		metadata?: Record<string, unknown>;
	} = {},
): VectorError =>
	createLibraryError(message, {
		name: 'StacksError',
		code: options.code ?? 'STACKS_ERROR',
		cause: options.cause,
		metadata: options.metadata,
	});

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isLibraryError = (value: unknown): value is VectorError =>
	isVectorError(value) &&
	(value.code.startsWith('LIBRARY_') ||
		value.code.startsWith('EMBEDDING_') ||
		value.code.startsWith('STACKS_'));

export const isStacksError = (value: unknown): value is VectorError =>
	isVectorError(value) && value.code.startsWith('STACKS_');

export const isEmbeddingError = (value: unknown): value is VectorError =>
	isVectorError(value) && value.code === 'EMBEDDING_ERROR';

export const isStacksCorruptionError = (
	value: unknown,
): value is VectorError & { readonly storePath: string } =>
	isVectorError(value) && value.code === 'STACKS_CORRUPT';

export const isStacksIOError = (
	value: unknown,
): value is VectorError & { readonly storePath: string } =>
	isVectorError(value) && value.code === 'STACKS_IO';
