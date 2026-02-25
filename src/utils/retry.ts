// ---------------------------------------------------------------------------
// Retry Utility with Exponential Backoff
// ---------------------------------------------------------------------------

import {
	createSimseError,
	isSimseError,
	type SimseError,
} from '../errors/index.js';

export interface RetryOptions {
	/** Maximum number of attempts (including the first). Minimum 1. */
	maxAttempts?: number;
	/** Base delay in milliseconds before the first retry. */
	baseDelayMs?: number;
	/** Maximum delay cap in milliseconds. */
	maxDelayMs?: number;
	/** Multiplier applied to the delay after each retry. */
	backoffMultiplier?: number;
	/** Optional jitter factor (0–1). Adds randomness to prevent thundering herd. */
	jitterFactor?: number;
	/**
	 * Predicate that decides whether the operation should be retried for a given error.
	 * Return `false` to abort immediately (e.g. for non-retryable errors).
	 * Defaults to always retry.
	 */
	shouldRetry?: (error: unknown, attempt: number) => boolean;
	/** Called before each retry with the error and upcoming attempt number. */
	onRetry?: (error: unknown, attempt: number, delayMs: number) => void;
	/** Optional AbortSignal to cancel retries externally. */
	signal?: AbortSignal;
}

const DEFAULT_OPTIONS: Required<
	Omit<RetryOptions, 'shouldRetry' | 'onRetry' | 'signal'>
> = {
	maxAttempts: 3,
	baseDelayMs: 500,
	maxDelayMs: 30_000,
	backoffMultiplier: 2,
	jitterFactor: 0.25,
};

/**
 * Create a `RetryExhaustedError` — thrown when all retry attempts have been
 * exhausted without success.
 */
export const createRetryExhaustedError = (
	attempts: number,
	lastError: unknown,
): SimseError & { readonly attempts: number } => {
	const err = createSimseError(`All ${attempts} retry attempts exhausted`, {
		name: 'RetryExhaustedError',
		code: 'RETRY_EXHAUSTED',
		cause: lastError,
		metadata: { attempts },
	}) as SimseError & { readonly attempts: number };

	Object.defineProperty(err, 'attempts', {
		value: attempts,
		writable: false,
		enumerable: true,
	});

	return err;
};

/**
 * Type-guard for `RetryExhaustedError`.
 */
export const isRetryExhaustedError = (
	value: unknown,
): value is SimseError & { readonly attempts: number } =>
	isSimseError(value) && value.code === 'RETRY_EXHAUSTED';

/**
 * Create a `RetryAbortedError` — thrown when the retry is cancelled via AbortSignal.
 */
export const createRetryAbortedError = (
	message: string,
	options: { cause?: unknown } = {},
): SimseError =>
	createSimseError(message, {
		name: 'RetryAbortedError',
		code: 'RETRY_ABORTED',
		cause: options.cause,
	});

/**
 * Type-guard for abort errors thrown when the retry signal fires.
 */
export const isRetryAbortedError = (value: unknown): value is SimseError =>
	isSimseError(value) && value.code === 'RETRY_ABORTED';

/**
 * Execute an async function with automatic retries and exponential backoff.
 *
 * @example
 * ```ts
 * const result = await retry(() => fetchFromAPI(), {
 *   maxAttempts: 4,
 *   baseDelayMs: 1000,
 *   shouldRetry: (err) => isTransientError(err),
 * });
 * ```
 */
export async function retry<T>(
	fn: (attempt: number) => Promise<T>,
	options: RetryOptions = {},
): Promise<T> {
	const maxAttempts = Math.max(
		1,
		options.maxAttempts ?? DEFAULT_OPTIONS.maxAttempts,
	);
	const baseDelayMs = options.baseDelayMs ?? DEFAULT_OPTIONS.baseDelayMs;
	const maxDelayMs = options.maxDelayMs ?? DEFAULT_OPTIONS.maxDelayMs;
	const backoffMultiplier =
		options.backoffMultiplier ?? DEFAULT_OPTIONS.backoffMultiplier;
	const jitterFactor = Math.max(
		0,
		Math.min(1, options.jitterFactor ?? DEFAULT_OPTIONS.jitterFactor),
	);
	const shouldRetry = options.shouldRetry ?? (() => true);
	const onRetry = options.onRetry;
	const signal = options.signal;

	let lastError: unknown;

	for (let attempt = 1; attempt <= maxAttempts; attempt++) {
		// Check if externally aborted before each attempt
		if (signal?.aborted) {
			throw createRetryAbortedError('Retry aborted by signal', {
				cause: lastError,
			});
		}

		try {
			return await fn(attempt);
		} catch (error) {
			lastError = error;

			// Don't retry on the last attempt
			if (attempt >= maxAttempts) {
				break;
			}

			// Check whether this error is retryable
			if (!shouldRetry(error, attempt)) {
				throw error;
			}

			// Calculate delay with exponential backoff and jitter
			const exponentialDelay = baseDelayMs * backoffMultiplier ** (attempt - 1);
			const cappedDelay = Math.min(exponentialDelay, maxDelayMs);
			const jitter =
				jitterFactor > 0
					? cappedDelay * jitterFactor * (Math.random() * 2 - 1)
					: 0;
			const finalDelay = Math.max(0, Math.round(cappedDelay + jitter));

			onRetry?.(error, attempt + 1, finalDelay);

			// Wait before retrying — sleep abort errors are re-thrown directly
			// without polluting lastError, so RetryExhaustedError always wraps
			// the last real domain error.
			await sleep(finalDelay, signal);
		}
	}

	throw createRetryExhaustedError(maxAttempts, lastError);
}

/**
 * Sleep for a given number of milliseconds.
 * Resolves immediately if `ms` is 0 or negative.
 * Rejects early if the optional AbortSignal fires.
 */
export function sleep(ms: number, signal?: AbortSignal): Promise<void> {
	if (ms <= 0) return Promise.resolve();

	return new Promise<void>((resolve, reject) => {
		if (signal?.aborted) {
			reject(createRetryAbortedError('Sleep aborted'));
			return;
		}

		const onAbort = () => {
			clearTimeout(timer);
			reject(createRetryAbortedError('Sleep aborted'));
		};

		const timer = setTimeout(() => {
			if (signal) signal.removeEventListener('abort', onAbort);
			resolve();
		}, ms);

		if (signal) {
			signal.addEventListener('abort', onAbort, { once: true });
		}
	});
}

/**
 * Convenience: check if an error is likely transient and worth retrying.
 * Works for common network / timeout scenarios.
 */
const STATUS_503 = /\b503\b/;
const STATUS_429 = /\b429\b/;

const TRANSIENT_CODES = new Set([
	'PROVIDER_TIMEOUT',
	'PROVIDER_UNAVAILABLE',
	'MCP_CONNECTION_ERROR',
	'OPERATION_TIMEOUT',
]);

export function isTransientError(error: unknown): boolean {
	if (isSimseError(error)) {
		if (TRANSIENT_CODES.has(error.code)) return true;

		// HTTP errors with transient status codes (5xx, 429)
		if (error.code === 'PROVIDER_HTTP_ERROR') {
			return (
				error.statusCode === 429 ||
				(error.statusCode >= 500 && error.statusCode < 600)
			);
		}

		return false;
	}

	if (error instanceof Error) {
		const lowerMessage = error.message.toLowerCase();
		return (
			lowerMessage.includes('econnrefused') ||
			lowerMessage.includes('econnreset') ||
			lowerMessage.includes('etimedout') ||
			lowerMessage.includes('socket hang up') ||
			lowerMessage.includes('network') ||
			lowerMessage.includes('timeout') ||
			lowerMessage.includes('unavailable') ||
			STATUS_503.test(lowerMessage) ||
			STATUS_429.test(lowerMessage)
		);
	}

	return false;
}
