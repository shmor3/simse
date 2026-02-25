// ---------------------------------------------------------------------------
// Circuit Breaker — prevents cascading failures by short-circuiting
// requests to unhealthy services.
//
// States: closed → open → half_open → closed
// ---------------------------------------------------------------------------

import { createCircuitBreakerOpenError } from '../errors/resilience.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type CircuitBreakerState = 'closed' | 'open' | 'half_open';

export interface CircuitBreakerOptions {
	/** Identifier for this breaker (used in error messages and callbacks). */
	readonly name: string;
	/** Number of consecutive failures before opening the circuit. Default 5. */
	readonly failureThreshold?: number;
	/** Time in ms to wait before transitioning from open to half_open. Default 30_000. */
	readonly resetTimeoutMs?: number;
	/** Max attempts allowed in half_open state before re-opening. Default 1. */
	readonly halfOpenMaxAttempts?: number;
	/** Called whenever the breaker transitions between states. */
	readonly onStateChange?: (
		from: CircuitBreakerState,
		to: CircuitBreakerState,
	) => void;
	/** Filter which errors count towards the failure threshold. Defaults to all errors. */
	readonly shouldCount?: (error: unknown) => boolean;
}

export interface CircuitBreaker {
	readonly execute: <T>(
		fn: () => Promise<T>,
		signal?: AbortSignal,
	) => Promise<T>;
	readonly getState: () => CircuitBreakerState;
	readonly getFailureCount: () => number;
	readonly reset: () => void;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createCircuitBreaker(
	options: CircuitBreakerOptions,
): CircuitBreaker {
	const {
		name,
		failureThreshold = 5,
		resetTimeoutMs = 30_000,
		halfOpenMaxAttempts = 1,
		onStateChange,
		shouldCount = () => true,
	} = options;

	let state: CircuitBreakerState = 'closed';
	let failureCount = 0;
	let lastFailureTime = 0;
	let halfOpenAttempts = 0;

	const transition = (to: CircuitBreakerState): void => {
		if (state === to) return;
		const from = state;
		state = to;
		onStateChange?.(from, to);
	};

	const execute = async <T>(
		fn: () => Promise<T>,
		signal?: AbortSignal,
	): Promise<T> => {
		// Check if we should transition from open → half_open (lazy timer check)
		if (state === 'open') {
			const elapsed = Date.now() - lastFailureTime;
			if (elapsed >= resetTimeoutMs) {
				transition('half_open');
				halfOpenAttempts = 0;
			} else {
				throw createCircuitBreakerOpenError(name);
			}
		}

		// In half_open, enforce max attempts
		if (state === 'half_open' && halfOpenAttempts >= halfOpenMaxAttempts) {
			throw createCircuitBreakerOpenError(name);
		}

		// Check abort signal
		if (signal?.aborted) {
			throw signal.reason ?? new Error('Aborted');
		}

		if (state === 'half_open') {
			halfOpenAttempts++;
		}

		try {
			const result = await fn();

			// Success — reset to closed
			if (state === 'half_open') {
				transition('closed');
			}
			failureCount = 0;

			return result;
		} catch (error) {
			if (shouldCount(error)) {
				failureCount++;
				lastFailureTime = Date.now();

				if (state === 'half_open') {
					// Any counted failure in half_open → re-open
					transition('open');
				} else if (failureCount >= failureThreshold) {
					transition('open');
				}
			}

			throw error;
		}
	};

	const getState = (): CircuitBreakerState => {
		// Lazy check: if open and timeout has elapsed, report half_open
		if (state === 'open') {
			const elapsed = Date.now() - lastFailureTime;
			if (elapsed >= resetTimeoutMs) {
				return 'half_open';
			}
		}
		return state;
	};

	const getFailureCount = (): number => failureCount;

	const reset = (): void => {
		const prev = state;
		state = 'closed';
		failureCount = 0;
		lastFailureTime = 0;
		halfOpenAttempts = 0;
		if (prev !== 'closed') {
			onStateChange?.(prev, 'closed');
		}
	};

	return Object.freeze({
		execute,
		getState,
		getFailureCount,
		reset,
	});
}
