// ---------------------------------------------------------------------------
// SimseError — base interface, factory, type guard, and utilities
// ---------------------------------------------------------------------------

/**
 * The SimseError interface describes the shape of every error produced by
 * SimSE.  Consumers discriminate errors via the `code` field and the
 * type-guard functions exported from sibling modules.
 */
export interface SimseError extends Error {
	/** Machine-readable error code (e.g. "CONFIG_INVALID", "PROVIDER_UNAVAILABLE"). */
	readonly code: string;
	/** HTTP-style status hint for MCP server responses. */
	readonly statusCode: number;
	/** Arbitrary structured context attached to the error. */
	readonly metadata: Record<string, unknown>;
	/** Return a plain-object representation suitable for logging / serialisation. */
	readonly toJSON: () => Record<string, unknown>;
}

// ---------------------------------------------------------------------------
// Options type shared by all factory helpers
// ---------------------------------------------------------------------------

export interface SimseErrorOptions {
	readonly name?: string;
	readonly code?: string;
	readonly statusCode?: number;
	readonly cause?: unknown;
	readonly metadata?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------
// Base factory
// ---------------------------------------------------------------------------

/**
 * Create a `SimseError` — a plain `Error` object augmented with structured
 * fields.  This is the only place in the codebase where `new Error` is used.
 */
export const createSimseError = (
	message: string,
	options: SimseErrorOptions = {},
): SimseError => {
	const err = new Error(message, { cause: options.cause }) as Error & {
		code: string;
		statusCode: number;
		metadata: Record<string, unknown>;
		toJSON: () => Record<string, unknown>;
	};

	err.name = options.name ?? 'SimseError';
	const code = options.code ?? 'SIMSE_ERROR';
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

	return err as SimseError;
};

// ---------------------------------------------------------------------------
// Base type guard
// ---------------------------------------------------------------------------

/**
 * Type-guard that checks whether a value is a `SimseError`.
 * Uses duck-typing on the `code` field rather than `instanceof`.
 */
export const isSimseError = (value: unknown): value is SimseError =>
	value instanceof Error &&
	typeof (value as unknown as Record<string, unknown>).code === 'string' &&
	typeof (value as unknown as Record<string, unknown>).statusCode ===
		'number' &&
	typeof (value as unknown as Record<string, unknown>).toJSON === 'function';

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/**
 * Normalise an unknown thrown value into a proper `Error` instance.
 * If it is already an `Error`, returns it directly.
 */
export const toError = (value: unknown): Error => {
	if (value instanceof Error) return value;
	if (typeof value === 'string') return new Error(value);
	return new Error(String(value));
};

/**
 * Wrap an unknown cause in a `SimseError` with an optional error code.
 * The original value is attached as `cause` for chaining.
 */
export const wrapError = (
	message: string,
	cause: unknown,
	code?: string,
): SimseError => createSimseError(message, { cause, code });
