// ---------------------------------------------------------------------------
// VFSError — self-contained error hierarchy for simse-vfs
// ---------------------------------------------------------------------------

/**
 * Structured error interface for simse-vfs.
 * Compatible with simse's SimseError shape for seamless integration.
 */
export interface VFSError extends Error {
	readonly code: string;
	readonly statusCode: number;
	readonly metadata: Record<string, unknown>;
	readonly toJSON: () => Record<string, unknown>;
}

export interface VFSErrorOptions {
	readonly name?: string;
	readonly code?: string;
	readonly statusCode?: number;
	readonly cause?: unknown;
	readonly metadata?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------
// Base factory
// ---------------------------------------------------------------------------

export const createVFSError = (
	message: string,
	options: VFSErrorOptions = {},
): VFSError => {
	const err = new Error(message, { cause: options.cause }) as Error & {
		code: string;
		statusCode: number;
		metadata: Record<string, unknown>;
		toJSON: () => Record<string, unknown>;
	};

	err.name = options.name ?? 'VFSError';
	const code = options.code ?? 'VFS_ERROR';
	const statusCode = options.statusCode ?? 400;
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

	return err as VFSError;
};

// ---------------------------------------------------------------------------
// Type guard
// ---------------------------------------------------------------------------

export const isVFSError = (value: unknown): value is VFSError =>
	value instanceof Error &&
	typeof (value as unknown as Record<string, unknown>).code === 'string' &&
	((value as unknown as Record<string, unknown>).code as string).startsWith(
		'VFS_',
	) &&
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
