// ---------------------------------------------------------------------------
// Logger — interface for simse-vfs
// ---------------------------------------------------------------------------

/**
 * Minimal logger interface.
 * Compatible with simse's Logger for seamless integration.
 */
export interface Logger {
	readonly debug: (
		message: string,
		metadata?: Readonly<Record<string, unknown>>,
	) => void;
	readonly info: (
		message: string,
		metadata?: Readonly<Record<string, unknown>>,
	) => void;
	readonly warn: (
		message: string,
		metadata?: Readonly<Record<string, unknown>>,
	) => void;
	readonly error: (
		message: string,
		errorOrMetadata?: Error | Readonly<Record<string, unknown>>,
	) => void;
	readonly child: (childContext: string) => Logger;
}

// ---------------------------------------------------------------------------
// No-op logger (default when none is provided)
// ---------------------------------------------------------------------------

const noop = (): void => {};

export const createNoopLogger = (): Logger =>
	Object.freeze({
		debug: noop,
		info: noop,
		warn: noop,
		error: noop,
		child: () => createNoopLogger(),
	});

// ---------------------------------------------------------------------------
// EventBus — minimal interface for event publishing
// ---------------------------------------------------------------------------

/**
 * Minimal event bus interface.
 * simse passes its full EventBus which is a superset of this.
 */
export interface EventBus {
	readonly publish: <T extends string>(type: T, payload: unknown) => void;
}
