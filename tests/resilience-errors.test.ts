import { describe, expect, it } from 'bun:test';
import { isSimseError } from '../src/errors/base.js';
import {
	createCircuitBreakerOpenError,
	createTimeoutError,
	isCircuitBreakerOpenError,
	isTimeoutError,
} from '../src/errors/resilience.js';
import { isTransientError } from '../src/utils/retry.js';

describe('resilience errors', () => {
	describe('createCircuitBreakerOpenError', () => {
		it('creates error with correct fields', () => {
			const err = createCircuitBreakerOpenError('my-breaker');
			expect(err.message).toContain('my-breaker');
			expect(err.code).toBe('CIRCUIT_BREAKER_OPEN');
			expect(err.statusCode).toBe(503);
			expect(err.breakerName).toBe('my-breaker');
			expect(isSimseError(err)).toBe(true);
		});

		it('is detected by isCircuitBreakerOpenError guard', () => {
			const err = createCircuitBreakerOpenError('test');
			expect(isCircuitBreakerOpenError(err)).toBe(true);
			expect(isCircuitBreakerOpenError(new Error('nope'))).toBe(false);
		});

		it('is NOT transient (open breaker should stop retries)', () => {
			const err = createCircuitBreakerOpenError('test');
			expect(isTransientError(err)).toBe(false);
		});
	});

	describe('createTimeoutError', () => {
		it('creates error with correct fields', () => {
			const err = createTimeoutError('fetch-data', 5000);
			expect(err.message).toContain('fetch-data');
			expect(err.message).toContain('5000');
			expect(err.code).toBe('OPERATION_TIMEOUT');
			expect(err.statusCode).toBe(504);
			expect(err.operation).toBe('fetch-data');
			expect(err.timeoutMs).toBe(5000);
			expect(isSimseError(err)).toBe(true);
		});

		it('is detected by isTimeoutError guard', () => {
			const err = createTimeoutError('op', 1000);
			expect(isTimeoutError(err)).toBe(true);
			expect(isTimeoutError(new Error('nope'))).toBe(false);
		});

		it('IS transient (timeouts should be retried)', () => {
			const err = createTimeoutError('op', 1000);
			expect(isTransientError(err)).toBe(true);
		});
	});
});
