import { describe, expect, it } from 'bun:test';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import type { ToolCallRequest } from '../src/ai/tools/types.js';
import { createSilentLogger } from './utils/mocks.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeCall(
	id: string,
	name: string,
	args: Record<string, unknown> = {},
): ToolCallRequest {
	return { id, name, arguments: args };
}

function createRegistryWithDelayTool(delayMs: number) {
	const registry = createToolRegistry({ logger: createSilentLogger() });
	registry.register(
		{
			name: 'delay_tool',
			description: 'Delays then returns',
			parameters: {},
		},
		async () => {
			await new Promise((r) => setTimeout(r, delayMs));
			return 'done';
		},
	);
	return registry;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('batchExecute', () => {
	it('runs multiple tool calls concurrently', async () => {
		const registry = createRegistryWithDelayTool(50);

		const calls = [
			makeCall('a', 'delay_tool'),
			makeCall('b', 'delay_tool'),
			makeCall('c', 'delay_tool'),
		];

		const start = Date.now();
		const results = await registry.batchExecute(calls);
		const elapsed = Date.now() - start;

		expect(results).toHaveLength(3);
		for (const r of results) {
			expect(r.isError).toBe(false);
			expect(r.output).toBe('done');
		}

		// 3 calls at 50ms each, if sequential would take ~150ms.
		// Concurrent should finish well under that.
		expect(elapsed).toBeLessThan(130);
	});

	it('returns results in input order', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });

		// Register tools with varying delays to force out-of-order completion
		registry.register(
			{ name: 'slow', description: 'Slow tool', parameters: {} },
			async () => {
				await new Promise((r) => setTimeout(r, 80));
				return 'slow-result';
			},
		);
		registry.register(
			{ name: 'fast', description: 'Fast tool', parameters: {} },
			async () => {
				await new Promise((r) => setTimeout(r, 10));
				return 'fast-result';
			},
		);

		const calls = [
			makeCall('1', 'slow'),
			makeCall('2', 'fast'),
			makeCall('3', 'slow'),
			makeCall('4', 'fast'),
		];

		const results = await registry.batchExecute(calls);

		expect(results[0].id).toBe('1');
		expect(results[0].output).toBe('slow-result');
		expect(results[1].id).toBe('2');
		expect(results[1].output).toBe('fast-result');
		expect(results[2].id).toBe('3');
		expect(results[2].output).toBe('slow-result');
		expect(results[3].id).toBe('4');
		expect(results[3].output).toBe('fast-result');
	});

	it('isolates errors per call', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });

		registry.register(
			{ name: 'ok_tool', description: 'Succeeds', parameters: {} },
			async () => 'success',
		);
		registry.register(
			{ name: 'bad_tool', description: 'Fails', parameters: {} },
			async () => {
				throw new Error('intentional failure');
			},
		);

		const calls = [
			makeCall('1', 'ok_tool'),
			makeCall('2', 'bad_tool'),
			makeCall('3', 'ok_tool'),
			makeCall('4', 'nonexistent_tool'),
		];

		const results = await registry.batchExecute(calls);

		expect(results).toHaveLength(4);
		expect(results[0].isError).toBe(false);
		expect(results[0].output).toBe('success');
		expect(results[1].isError).toBe(true);
		expect(results[1].output).toContain('intentional failure');
		expect(results[2].isError).toBe(false);
		expect(results[2].output).toBe('success');
		expect(results[3].isError).toBe(true);
		expect(results[3].name).toBe('nonexistent_tool');
	});

	it('respects maxConcurrency', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		let activeCalls = 0;
		let peakConcurrency = 0;

		registry.register(
			{ name: 'tracked', description: 'Tracks concurrency', parameters: {} },
			async () => {
				activeCalls++;
				if (activeCalls > peakConcurrency) {
					peakConcurrency = activeCalls;
				}
				await new Promise((r) => setTimeout(r, 30));
				activeCalls--;
				return 'ok';
			},
		);

		const calls = Array.from({ length: 10 }, (_, i) =>
			makeCall(String(i), 'tracked'),
		);

		const results = await registry.batchExecute(calls, { maxConcurrency: 2 });

		expect(results).toHaveLength(10);
		expect(peakConcurrency).toBeLessThanOrEqual(2);
		expect(peakConcurrency).toBeGreaterThanOrEqual(1);
		for (const r of results) {
			expect(r.isError).toBe(false);
		}
	});

	it('returns empty array for empty input', async () => {
		const registry = createToolRegistry({ logger: createSilentLogger() });
		const results = await registry.batchExecute([]);

		expect(results).toHaveLength(0);
		expect(Array.isArray(results)).toBe(true);
	});
});
