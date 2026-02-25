import { describe, expect, it } from 'bun:test';
import { createHealthMonitor } from '../src/utils/health-monitor.js';

describe('createHealthMonitor', () => {
	it('starts healthy with zero counters', () => {
		const hm = createHealthMonitor();
		const snap = hm.getHealth();
		expect(snap.status).toBe('healthy');
		expect(snap.consecutiveFailures).toBe(0);
		expect(snap.totalCalls).toBe(0);
		expect(snap.totalFailures).toBe(0);
		expect(snap.failureRate).toBe(0);
		expect(snap.lastSuccessTime).toBeUndefined();
		expect(snap.lastFailureTime).toBeUndefined();
		expect(snap.lastError).toBeUndefined();
		expect(hm.isHealthy()).toBe(true);
	});

	it('records successes', () => {
		const hm = createHealthMonitor();
		hm.recordSuccess();
		hm.recordSuccess();
		const snap = hm.getHealth();
		expect(snap.totalCalls).toBe(2);
		expect(snap.totalFailures).toBe(0);
		expect(snap.consecutiveFailures).toBe(0);
		expect(snap.lastSuccessTime).toBeDefined();
	});

	it('records failures with error', () => {
		const hm = createHealthMonitor();
		const error = new Error('boom');
		hm.recordFailure(error);
		const snap = hm.getHealth();
		expect(snap.totalCalls).toBe(1);
		expect(snap.totalFailures).toBe(1);
		expect(snap.consecutiveFailures).toBe(1);
		expect(snap.lastError).toBe(error);
		expect(snap.lastFailureTime).toBeDefined();
	});

	it('converts non-Error failures to Error', () => {
		const hm = createHealthMonitor();
		hm.recordFailure('string error');
		const snap = hm.getHealth();
		expect(snap.lastError).toBeInstanceOf(Error);
		expect(snap.lastError!.message).toBe('string error');
	});

	it('transitions to degraded at threshold', () => {
		const hm = createHealthMonitor({ degradedThreshold: 2 });
		hm.recordFailure();
		expect(hm.getHealth().status).toBe('healthy');
		hm.recordFailure();
		expect(hm.getHealth().status).toBe('degraded');
		expect(hm.isHealthy()).toBe(false);
	});

	it('transitions to unhealthy at threshold', () => {
		const hm = createHealthMonitor({
			degradedThreshold: 2,
			unhealthyThreshold: 4,
		});
		for (let i = 0; i < 4; i++) {
			hm.recordFailure();
		}
		expect(hm.getHealth().status).toBe('unhealthy');
	});

	it('recovers to healthy on success', () => {
		const hm = createHealthMonitor({ degradedThreshold: 2 });
		hm.recordFailure();
		hm.recordFailure();
		expect(hm.getHealth().status).toBe('degraded');
		hm.recordSuccess();
		expect(hm.getHealth().status).toBe('healthy');
		expect(hm.isHealthy()).toBe(true);
	});

	it('computes failure rate from windowed events', () => {
		const hm = createHealthMonitor({ windowMs: 60_000 });
		hm.recordSuccess();
		hm.recordFailure();
		hm.recordSuccess();
		hm.recordFailure();
		// 2 failures out of 4 = 0.5
		expect(hm.getHealth().failureRate).toBe(0.5);
	});

	it('prunes old events outside window', async () => {
		const hm = createHealthMonitor({ windowMs: 50 });
		hm.recordFailure();
		hm.recordFailure();

		// Wait for window to expire
		await new Promise<void>((r) => setTimeout(r, 60));

		hm.recordSuccess();
		const snap = hm.getHealth();
		// Only the recent success is in the window
		expect(snap.failureRate).toBe(0);
		// But total counts remain
		expect(snap.totalFailures).toBe(2);
		expect(snap.totalCalls).toBe(3);
	});

	it('reset clears everything', () => {
		const hm = createHealthMonitor();
		hm.recordFailure(new Error('err'));
		hm.recordFailure(new Error('err'));
		hm.recordSuccess();
		hm.reset();
		const snap = hm.getHealth();
		expect(snap.status).toBe('healthy');
		expect(snap.totalCalls).toBe(0);
		expect(snap.totalFailures).toBe(0);
		expect(snap.consecutiveFailures).toBe(0);
		expect(snap.lastSuccessTime).toBeUndefined();
		expect(snap.lastFailureTime).toBeUndefined();
		expect(snap.lastError).toBeUndefined();
		expect(snap.failureRate).toBe(0);
	});

	it('returns frozen snapshots', () => {
		const hm = createHealthMonitor();
		hm.recordSuccess();
		expect(Object.isFrozen(hm.getHealth())).toBe(true);
	});

	it('returns frozen interface', () => {
		const hm = createHealthMonitor();
		expect(Object.isFrozen(hm)).toBe(true);
	});
});
