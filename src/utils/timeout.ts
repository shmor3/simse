// ---------------------------------------------------------------------------
// Timeout Utility — wraps an async function with a timeout that rejects
// with a structured OPERATION_TIMEOUT error.
// ---------------------------------------------------------------------------

import { createTimeoutError } from '../errors/resilience.js';

export interface TimeoutOptions {
	/** Label for the operation (used in error messages). */
	readonly operation?: string;
	/** Optional AbortSignal to compose with the timeout. */
	readonly signal?: AbortSignal;
}

/**
 * Run an async function with a timeout. Rejects with `createTimeoutError()`
 * if the function doesn't settle within `timeoutMs`.
 *
 * The timer is cleaned up on resolution or rejection.
 */
export async function withTimeout<T>(
	fn: () => Promise<T>,
	timeoutMs: number,
	options?: TimeoutOptions,
): Promise<T> {
	const operation = options?.operation ?? 'unknown';
	const signal = options?.signal;

	if (signal?.aborted) {
		throw signal.reason ?? new Error('Aborted');
	}

	return new Promise<T>((resolve, reject) => {
		let settled = false;

		const timer = setTimeout(() => {
			if (!settled) {
				settled = true;
				reject(createTimeoutError(operation, timeoutMs));
			}
		}, timeoutMs);

		const onAbort = (): void => {
			if (!settled) {
				settled = true;
				clearTimeout(timer);
				reject(signal?.reason ?? new Error('Aborted'));
			}
		};

		if (signal) {
			signal.addEventListener('abort', onAbort, { once: true });
		}

		fn().then(
			(value) => {
				if (!settled) {
					settled = true;
					clearTimeout(timer);
					if (signal) signal.removeEventListener('abort', onAbort);
					resolve(value);
				}
			},
			(error) => {
				if (!settled) {
					settled = true;
					clearTimeout(timer);
					if (signal) signal.removeEventListener('abort', onAbort);
					reject(error);
				}
			},
		);
	});
}
