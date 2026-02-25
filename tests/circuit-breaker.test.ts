import { describe, expect, it } from 'bun:test';
import { isCircuitBreakerOpenError } from '../src/errors/resilience.js';
import {
	type CircuitBreakerState,
	createCircuitBreaker,
} from '../src/utils/circuit-breaker.js';

describe('createCircuitBreaker', () => {
	it('starts in closed state', () => {
		const cb = createCircuitBreaker({ name: 'test' });
		expect(cb.getState()).toBe('closed');
		expect(cb.getFailureCount()).toBe(0);
	});

	it('stays closed on success', async () => {
		const cb = createCircuitBreaker({ name: 'test' });
		const result = await cb.execute(async () => 42);
		expect(result).toBe(42);
		expect(cb.getState()).toBe('closed');
	});

	it('opens after failureThreshold consecutive failures', async () => {
		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 3,
		});

		for (let i = 0; i < 3; i++) {
			await cb
				.execute(async () => {
					throw new Error('fail');
				})
				.catch(() => {});
		}

		expect(cb.getState()).toBe('open');
		expect(cb.getFailureCount()).toBe(3);
	});

	it('rejects immediately when open', async () => {
		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 1,
			resetTimeoutMs: 60_000,
		});

		await cb
			.execute(async () => {
				throw new Error('fail');
			})
			.catch(() => {});

		expect(cb.getState()).toBe('open');

		try {
			await cb.execute(async () => 'should not run');
			expect.unreachable('should have thrown');
		} catch (error) {
			expect(isCircuitBreakerOpenError(error)).toBe(true);
		}
	});

	it('transitions to half_open after resetTimeoutMs', async () => {
		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 1,
			resetTimeoutMs: 10,
		});

		await cb
			.execute(async () => {
				throw new Error('fail');
			})
			.catch(() => {});

		expect(cb.getState()).toBe('open');

		// Wait for reset timeout
		await new Promise<void>((r) => setTimeout(r, 20));

		// getState() does lazy check
		expect(cb.getState()).toBe('half_open');
	});

	it('closes from half_open on success', async () => {
		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 1,
			resetTimeoutMs: 10,
		});

		await cb
			.execute(async () => {
				throw new Error('fail');
			})
			.catch(() => {});

		await new Promise<void>((r) => setTimeout(r, 20));

		const result = await cb.execute(async () => 'recovered');
		expect(result).toBe('recovered');
		expect(cb.getState()).toBe('closed');
		expect(cb.getFailureCount()).toBe(0);
	});

	it('re-opens from half_open on failure', async () => {
		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 1,
			resetTimeoutMs: 10,
		});

		await cb
			.execute(async () => {
				throw new Error('fail');
			})
			.catch(() => {});

		await new Promise<void>((r) => setTimeout(r, 20));

		await cb
			.execute(async () => {
				throw new Error('still failing');
			})
			.catch(() => {});

		expect(cb.getState()).toBe('open');
	});

	it('respects halfOpenMaxAttempts', async () => {
		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 1,
			resetTimeoutMs: 10,
			halfOpenMaxAttempts: 1,
		});

		await cb
			.execute(async () => {
				throw new Error('fail');
			})
			.catch(() => {});

		await new Promise<void>((r) => setTimeout(r, 20));

		// First half_open attempt (succeeds or fails — doesn't matter, it's allowed)
		// Here it fails, so breaker re-opens
		await cb
			.execute(async () => {
				throw new Error('fail in half_open');
			})
			.catch(() => {});

		// Second attempt should be blocked
		try {
			await cb.execute(async () => 'blocked');
			expect.unreachable('should have thrown');
		} catch (error) {
			expect(isCircuitBreakerOpenError(error)).toBe(true);
		}
	});

	it('calls onStateChange callback', async () => {
		const transitions: Array<{
			from: CircuitBreakerState;
			to: CircuitBreakerState;
		}> = [];

		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 1,
			resetTimeoutMs: 10,
			onStateChange: (from, to) => {
				transitions.push({ from, to });
			},
		});

		await cb
			.execute(async () => {
				throw new Error('fail');
			})
			.catch(() => {});

		expect(transitions).toEqual([{ from: 'closed', to: 'open' }]);

		await new Promise<void>((r) => setTimeout(r, 20));
		await cb.execute(async () => 'ok');

		expect(transitions).toEqual([
			{ from: 'closed', to: 'open' },
			{ from: 'open', to: 'half_open' },
			{ from: 'half_open', to: 'closed' },
		]);
	});

	it('respects shouldCount filter', async () => {
		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 2,
			shouldCount: (error) =>
				error instanceof Error && error.message === 'transient',
		});

		// Non-matching error — should not count
		await cb
			.execute(async () => {
				throw new Error('permanent');
			})
			.catch(() => {});

		expect(cb.getFailureCount()).toBe(0);
		expect(cb.getState()).toBe('closed');

		// Matching errors
		await cb
			.execute(async () => {
				throw new Error('transient');
			})
			.catch(() => {});
		await cb
			.execute(async () => {
				throw new Error('transient');
			})
			.catch(() => {});

		expect(cb.getFailureCount()).toBe(2);
		expect(cb.getState()).toBe('open');
	});

	it('reset() returns to closed state', async () => {
		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 1,
		});

		await cb
			.execute(async () => {
				throw new Error('fail');
			})
			.catch(() => {});

		expect(cb.getState()).toBe('open');
		cb.reset();
		expect(cb.getState()).toBe('closed');
		expect(cb.getFailureCount()).toBe(0);
	});

	it('respects AbortSignal', async () => {
		const cb = createCircuitBreaker({ name: 'test' });
		const controller = new AbortController();
		controller.abort();

		try {
			await cb.execute(async () => 'should not run', controller.signal);
			expect.unreachable('should have thrown');
		} catch (error) {
			expect(error).toBeDefined();
		}
	});

	it('success resets failure count', async () => {
		const cb = createCircuitBreaker({
			name: 'test',
			failureThreshold: 5,
		});

		// 3 failures
		for (let i = 0; i < 3; i++) {
			await cb
				.execute(async () => {
					throw new Error('fail');
				})
				.catch(() => {});
		}
		expect(cb.getFailureCount()).toBe(3);

		// 1 success resets count
		await cb.execute(async () => 'ok');
		expect(cb.getFailureCount()).toBe(0);
	});

	it('returns frozen interface', () => {
		const cb = createCircuitBreaker({ name: 'test' });
		expect(Object.isFrozen(cb)).toBe(true);
	});
});
