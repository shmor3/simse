import { describe, expect, it } from 'bun:test';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';

describe('tool execution metrics', () => {
	it('returns undefined metrics for unknown tool', () => {
		const registry = createToolRegistry({});
		const metrics = registry.getToolMetrics('nonexistent');
		expect(metrics).toBeUndefined();
	});

	it('tracks call count after successful execution', async () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'test_tool', description: 'test', parameters: {} },
			async () => 'ok',
		);

		await registry.execute({ id: 'c1', name: 'test_tool', arguments: {} });
		await registry.execute({ id: 'c2', name: 'test_tool', arguments: {} });

		const metrics = registry.getToolMetrics('test_tool');
		expect(metrics).toBeDefined();
		expect(metrics!.callCount).toBe(2);
		expect(metrics!.errorCount).toBe(0);
	});

	it('tracks error count', async () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'fail_tool', description: 'fails', parameters: {} },
			async () => {
				throw new Error('boom');
			},
		);

		await registry.execute({ id: 'c1', name: 'fail_tool', arguments: {} });

		const metrics = registry.getToolMetrics('fail_tool');
		expect(metrics!.callCount).toBe(1);
		expect(metrics!.errorCount).toBe(1);
	});

	it('tracks duration', async () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'timed_tool', description: 'timed', parameters: {} },
			async () => {
				await new Promise((r) => setTimeout(r, 10));
				return 'ok';
			},
		);

		await registry.execute({ id: 'c1', name: 'timed_tool', arguments: {} });

		const metrics = registry.getToolMetrics('timed_tool');
		expect(metrics!.totalDurationMs).toBeGreaterThan(0);
		expect(metrics!.avgDurationMs).toBeGreaterThan(0);
	});

	it('getAllToolMetrics returns all tracked tools', async () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'tool_a', description: 'a', parameters: {} },
			async () => 'a',
		);
		registry.register(
			{ name: 'tool_b', description: 'b', parameters: {} },
			async () => 'b',
		);

		await registry.execute({ id: 'c1', name: 'tool_a', arguments: {} });
		await registry.execute({ id: 'c2', name: 'tool_b', arguments: {} });

		const allMetrics = registry.getAllToolMetrics();
		expect(allMetrics).toHaveLength(2);
	});

	it('metrics include lastCalledAt timestamp', async () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'ts_tool', description: 'ts', parameters: {} },
			async () => 'ok',
		);

		const before = Date.now();
		await registry.execute({ id: 'c1', name: 'ts_tool', arguments: {} });

		const metrics = registry.getToolMetrics('ts_tool');
		expect(metrics!.lastCalledAt).toBeGreaterThanOrEqual(before);
	});

	it('clearMetrics resets all metrics', async () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'tool_a', description: 'a', parameters: {} },
			async () => 'a',
		);

		await registry.execute({ id: 'c1', name: 'tool_a', arguments: {} });
		expect(registry.getToolMetrics('tool_a')).toBeDefined();

		registry.clearMetrics();
		expect(registry.getToolMetrics('tool_a')).toBeUndefined();
		expect(registry.getAllToolMetrics()).toHaveLength(0);
	});

	it('isRegistered returns true for registered tools', () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'my_tool', description: 'test', parameters: {} },
			async () => 'ok',
		);

		expect(registry.isRegistered('my_tool')).toBe(true);
		expect(registry.isRegistered('nonexistent')).toBe(false);
	});

	it('isRegistered updates after unregister', () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'temp_tool', description: 'temp', parameters: {} },
			async () => 'ok',
		);

		expect(registry.isRegistered('temp_tool')).toBe(true);
		registry.unregister('temp_tool');
		expect(registry.isRegistered('temp_tool')).toBe(false);
	});
});

describe('tool output truncation', () => {
	it('truncates output exceeding default maxOutputChars', async () => {
		const registry = createToolRegistry({
			maxOutputChars: 100,
		});
		registry.register(
			{ name: 'big_tool', description: 'returns big output', parameters: {} },
			async () => 'x'.repeat(200),
		);

		const result = await registry.execute({
			id: 'c1',
			name: 'big_tool',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output.length).toBeLessThan(200);
		expect(result.output).toContain('[OUTPUT TRUNCATED');
		expect(result.output).toContain('200 chars total');
		expect(result.output).toContain('showing first 100');
	});

	it('does not truncate output under the limit', async () => {
		const registry = createToolRegistry({
			maxOutputChars: 1000,
		});
		registry.register(
			{ name: 'small_tool', description: 'small output', parameters: {} },
			async () => 'hello',
		);

		const result = await registry.execute({
			id: 'c1',
			name: 'small_tool',
			arguments: {},
		});
		expect(result.output).toBe('hello');
	});

	it('per-tool maxOutputChars overrides registry default', async () => {
		const registry = createToolRegistry({
			maxOutputChars: 1000,
		});
		registry.register(
			{
				name: 'limited_tool',
				description: 'limited',
				parameters: {},
				maxOutputChars: 50,
			},
			async () => 'y'.repeat(200),
		);

		const result = await registry.execute({
			id: 'c1',
			name: 'limited_tool',
			arguments: {},
		});
		expect(result.output).toContain('[OUTPUT TRUNCATED');
		expect(result.output).toContain('showing first 50');
	});
});

describe('tool getToolDefinition', () => {
	it('returns definition for registered tool', () => {
		const registry = createToolRegistry({});
		registry.register(
			{
				name: 'lookup_tool',
				description: 'test lookup',
				parameters: { query: { type: 'string', description: 'search query' } },
				category: 'search',
			},
			async () => 'ok',
		);

		const def = registry.getToolDefinition('lookup_tool');
		expect(def).toBeDefined();
		expect(def!.name).toBe('lookup_tool');
		expect(def!.description).toBe('test lookup');
		expect(def!.category).toBe('search');
	});

	it('returns undefined for unknown tool', () => {
		const registry = createToolRegistry({});
		expect(registry.getToolDefinition('nonexistent')).toBeUndefined();
	});

	it('returns undefined after tool is unregistered', () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'temp', description: 'temp', parameters: {} },
			async () => 'ok',
		);
		registry.unregister('temp');
		expect(registry.getToolDefinition('temp')).toBeUndefined();
	});
});
