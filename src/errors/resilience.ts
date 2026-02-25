// ---------------------------------------------------------------------------
// Resilience Errors — Circuit Breaker + Timeout
// ---------------------------------------------------------------------------

import type { SimseError } from './base.js';
import { createSimseError, isSimseError } from './base.js';

// ---------------------------------------------------------------------------
// Circuit Breaker Open Error
// ---------------------------------------------------------------------------

export const createCircuitBreakerOpenError = (
	name: string,
): SimseError & { readonly breakerName: string } => {
	const err = createSimseError(
		`Circuit breaker "${name}" is open — requests are blocked`,
		{
			name: 'CircuitBreakerOpenError',
			code: 'CIRCUIT_BREAKER_OPEN',
			statusCode: 503,
			metadata: { breakerName: name },
		},
	) as SimseError & { readonly breakerName: string };

	Object.defineProperty(err, 'breakerName', {
		value: name,
		writable: false,
		enumerable: true,
	});

	return err;
};

// ---------------------------------------------------------------------------
// Timeout Error
// ---------------------------------------------------------------------------

export const createTimeoutError = (
	operation: string,
	timeoutMs: number,
): SimseError & {
	readonly operation: string;
	readonly timeoutMs: number;
} => {
	const err = createSimseError(
		`Operation "${operation}" timed out after ${timeoutMs}ms`,
		{
			name: 'TimeoutError',
			code: 'OPERATION_TIMEOUT',
			statusCode: 504,
			metadata: { operation, timeoutMs },
		},
	) as SimseError & {
		readonly operation: string;
		readonly timeoutMs: number;
	};

	Object.defineProperty(err, 'operation', {
		value: operation,
		writable: false,
		enumerable: true,
	});

	Object.defineProperty(err, 'timeoutMs', {
		value: timeoutMs,
		writable: false,
		enumerable: true,
	});

	return err;
};

// ---------------------------------------------------------------------------
// Type Guards
// ---------------------------------------------------------------------------

export const isCircuitBreakerOpenError = (
	value: unknown,
): value is SimseError & { readonly breakerName: string } =>
	isSimseError(value) && value.code === 'CIRCUIT_BREAKER_OPEN';

export const isTimeoutError = (
	value: unknown,
): value is SimseError & {
	readonly operation: string;
	readonly timeoutMs: number;
} => isSimseError(value) && value.code === 'OPERATION_TIMEOUT';
