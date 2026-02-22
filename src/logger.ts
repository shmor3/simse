// ---------------------------------------------------------------------------
// Structured Logger — Functional API
// ---------------------------------------------------------------------------
//
// All logger state is captured in plain readonly data.  Logger "instances"
// are frozen records of functions that close over an immutable config.
// No classes — only interfaces, pure functions, and closures.
// ---------------------------------------------------------------------------

export type LogLevel = 'debug' | 'info' | 'warn' | 'error' | 'none';

const LOG_LEVEL_PRIORITY: Readonly<Record<LogLevel, number>> = Object.freeze({
	debug: 0,
	info: 1,
	warn: 2,
	error: 3,
	none: 4,
});

// ---------------------------------------------------------------------------
// Log Entry
// ---------------------------------------------------------------------------

export interface LogEntry {
	readonly level: LogLevel;
	readonly message: string;
	readonly timestamp: string;
	readonly context?: string;
	readonly metadata?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------
// Transport interface
// ---------------------------------------------------------------------------

export interface LogTransport {
	readonly write: (entry: LogEntry) => void;
}

// ---------------------------------------------------------------------------
// Built-in Transports (constructed via factory functions)
// ---------------------------------------------------------------------------

const COLOURS: Readonly<Record<string, string>> = Object.freeze({
	debug: '\x1b[90m',
	info: '\x1b[36m',
	warn: '\x1b[33m',
	error: '\x1b[31m',
	reset: '\x1b[0m',
});

/**
 * Create a transport that writes formatted log entries to the console.
 */
export const createConsoleTransport = (): LogTransport =>
	Object.freeze({
		write(entry: LogEntry): void {
			const colour = COLOURS[entry.level] ?? COLOURS.reset;
			const reset = COLOURS.reset;
			const prefix = entry.context ? `[${entry.context}]` : '';
			const tag = entry.level.toUpperCase().padEnd(5);
			const time = entry.timestamp;

			const base = `${colour}${tag}${reset} ${time} ${prefix} ${entry.message}`;
			const hasMetadata =
				entry.metadata !== undefined && Object.keys(entry.metadata).length > 0;

			const logFn =
				entry.level === 'error'
					? console.error
					: entry.level === 'warn'
						? console.warn
						: entry.level === 'debug'
							? console.debug
							: console.log;

			hasMetadata ? logFn(base, entry.metadata) : logFn(base);
		},
	});

/**
 * A transport backed by a mutable array — useful for testing.
 *
 * We deliberately keep `entries` as a plain mutable array so tests can
 * inspect / clear it.  The transport itself is otherwise side-effect–free.
 */
export interface MemoryTransportHandle extends LogTransport {
	readonly entries: LogEntry[];
	readonly clear: () => void;
	readonly filter: (level: LogLevel) => readonly LogEntry[];
}

export const createMemoryTransport = (): MemoryTransportHandle => {
	const entries: LogEntry[] = [];

	return {
		entries,
		write(entry: LogEntry): void {
			entries.push(entry);
		},
		clear(): void {
			entries.length = 0;
		},
		filter(level: LogLevel): readonly LogEntry[] {
			return entries.filter((e) => e.level === level);
		},
	};
};

// ---------------------------------------------------------------------------
// Logger interface — a record of functions
// ---------------------------------------------------------------------------

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
	readonly setLevel: (level: LogLevel) => void;
	readonly getLevel: () => LogLevel;
	readonly addTransport: (transport: LogTransport) => void;
	readonly clearTransports: () => void;
}

// ---------------------------------------------------------------------------
// Logger options
// ---------------------------------------------------------------------------

export interface LoggerOptions {
	readonly context?: string;
	readonly level?: LogLevel;
	readonly transports?: readonly LogTransport[];
}

// ---------------------------------------------------------------------------
// createLogger — the primary factory
// ---------------------------------------------------------------------------

const resolveErrorMetadata = (
	errorOrMetadata: Error | Readonly<Record<string, unknown>> | undefined,
): Readonly<Record<string, unknown>> | undefined => {
	if (errorOrMetadata === undefined) return undefined;

	// Check instanceof first; fall back to duck-typing for cross-realm errors
	const obj = errorOrMetadata as Record<string, unknown>;
	if (
		errorOrMetadata instanceof Error ||
		(errorOrMetadata &&
			typeof errorOrMetadata === 'object' &&
			typeof obj.name === 'string' &&
			typeof obj.message === 'string' &&
			(typeof obj.stack === 'string' || obj.stack === undefined) &&
			'stack' in obj)
	) {
		const causeObj = obj.cause as Record<string, unknown> | undefined;
		return {
			errorName: obj.name as string,
			errorMessage: obj.message as string,
			stack: obj.stack as string | undefined,
			...(obj.cause != null
				? {
						cause:
							causeObj &&
							typeof causeObj === 'object' &&
							typeof causeObj.message === 'string'
								? (causeObj.message as string)
								: String(obj.cause),
					}
				: {}),
		};
	}

	return errorOrMetadata as Readonly<Record<string, unknown>>;
};

/**
 * Shared mutable state container so that parent and child loggers
 * share the same level and transport list by reference.
 */
interface LoggerState {
	level: LogLevel;
	readonly transports: LogTransport[];
}

export const createLogger = (options: LoggerOptions = {}): Logger => {
	const context = options.context;

	// Allow callers (child()) to pass a shared state object directly.
	const internalOpts = options as LoggerOptions & { _state?: LoggerState };
	const state: LoggerState = internalOpts._state ?? {
		level: options.level ?? 'info',
		transports: options.transports
			? [...options.transports]
			: [createConsoleTransport()],
	};

	const log = (
		level: LogLevel,
		message: string,
		metadata?: Readonly<Record<string, unknown>>,
	): void => {
		if (level === 'none') return;
		if (LOG_LEVEL_PRIORITY[level] < LOG_LEVEL_PRIORITY[state.level]) return;

		const entry: LogEntry = Object.freeze({
			level,
			message,
			timestamp: new Date().toISOString(),
			context,
			metadata,
		});

		for (const transport of state.transports) {
			transport.write(entry);
		}
	};

	const logger: Logger = {
		debug: (message, metadata?) => log('debug', message, metadata),
		info: (message, metadata?) => log('info', message, metadata),
		warn: (message, metadata?) => log('warn', message, metadata),
		error: (message, errorOrMetadata?) =>
			log('error', message, resolveErrorMetadata(errorOrMetadata)),

		child: (childContext: string): Logger => {
			const combined = context ? `${context}:${childContext}` : childContext;
			return createLogger({
				context: combined,
				_state: state,
			} as LoggerOptions);
		},

		setLevel: (level: LogLevel): void => {
			state.level = level;
		},
		getLevel: (): LogLevel => state.level,

		addTransport: (transport: LogTransport): void => {
			state.transports.push(transport);
		},
		clearTransports: (): void => {
			state.transports.length = 0;
		},
	};

	return logger;
};

// ---------------------------------------------------------------------------
// Default singleton
// ---------------------------------------------------------------------------

let _defaultLogger: Logger | undefined;

/**
 * Get (or create) the default application-wide logger.
 * Call `setDefaultLogger()` to replace it.
 */
export const getDefaultLogger = (): Logger => {
	if (!_defaultLogger) {
		_defaultLogger = createLogger({ context: 'simse' });
	}
	return _defaultLogger;
};

/**
 * Replace the default application-wide logger.
 */
export const setDefaultLogger = (logger: Logger): void => {
	_defaultLogger = logger;
};
