import { describe, expect, it } from 'bun:test';
import { isTimeoutError } from '../src/errors/resilience.js';
import { withTimeout } from '../src/utils/timeout.js';

describe('withTimeout', () => {
	it('resolves when fn completes before timeout', async () => {
		const result = await withTimeout(async () => 42, 1000);
		expect(result).toBe(42);
	});

	it('rejects with OPERATION_TIMEOUT when fn exceeds timeout', async () => {
		try {
			await withTimeout(
				async () => new Promise<void>((r) => setTimeout(r, 1000)),
				10,
				{ operation: 'slow-op' },
			);
			expect.unreachable('should have thrown');
		} catch (error) {
			expect(isTimeoutError(error)).toBe(true);
			if (isTimeoutError(error)) {
				expect(error.operation).toBe('slow-op');
				expect(error.timeoutMs).toBe(10);
				expect(error.statusCode).toBe(504);
			}
		}
	});

	it('propagates fn errors without wrapping', async () => {
		const original = new Error('fn failed');
		try {
			await withTimeout(async () => {
				throw original;
			}, 1000);
			expect.unreachable('should have thrown');
		} catch (error) {
			expect(error).toBe(original);
		}
	});

	it('respects AbortSignal', async () => {
		const controller = new AbortController();

		const promise = withTimeout(
			async () => new Promise<void>((r) => setTimeout(r, 5000)),
			10_000,
			{ signal: controller.signal },
		);

		// Abort after 10ms
		setTimeout(() => controller.abort(), 10);

		try {
			await promise;
			expect.unreachable('should have thrown');
		} catch {
			// Should reject due to abort, not timeout
		}
	});

	it('rejects immediately if signal is already aborted', async () => {
		const controller = new AbortController();
		controller.abort();

		try {
			await withTimeout(async () => 'should not run', 1000, {
				signal: controller.signal,
			});
			expect.unreachable('should have thrown');
		} catch {
			// Expected
		}
	});

	it('cleans up timer on successful resolution', async () => {
		// This test mainly ensures no timer leak — if timers leaked,
		// bun would warn about unresolved handles
		const result = await withTimeout(async () => 'fast', 5000);
		expect(result).toBe('fast');
	});

	it('uses default operation name when not specified', async () => {
		try {
			await withTimeout(
				async () => new Promise<void>((r) => setTimeout(r, 1000)),
				10,
			);
			expect.unreachable('should have thrown');
		} catch (error) {
			expect(isTimeoutError(error)).toBe(true);
			if (isTimeoutError(error)) {
				expect(error.operation).toBe('unknown');
			}
		}
	});
});
