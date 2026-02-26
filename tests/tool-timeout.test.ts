import { describe, expect, it } from 'bun:test';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';

describe('tool execution timeout', () => {
	it('times out a slow tool with per-tool timeout', async () => {
		const registry = createToolRegistry({});
		registry.register(
			{
				name: 'slow_tool',
				description: 'A slow tool',
				parameters: {},
				timeoutMs: 50,
			},
			async () => {
				await new Promise((r) => setTimeout(r, 5000));
				return 'done';
			},
		);

		const result = await registry.execute({
			id: 'c1',
			name: 'slow_tool',
			arguments: {},
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('timed out');
	});

	it('times out with global default timeout', async () => {
		const registry = createToolRegistry({
			defaultToolTimeoutMs: 50,
		});
		registry.register(
			{
				name: 'slow_tool',
				description: 'A slow tool',
				parameters: {},
			},
			async () => {
				await new Promise((r) => setTimeout(r, 5000));
				return 'done';
			},
		);

		const result = await registry.execute({
			id: 'c2',
			name: 'slow_tool',
			arguments: {},
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('timed out');
	});

	it('per-tool timeout overrides global default', async () => {
		const registry = createToolRegistry({
			defaultToolTimeoutMs: 5000,
		});
		registry.register(
			{
				name: 'slow_tool',
				description: 'A slow tool',
				parameters: {},
				timeoutMs: 50,
			},
			async () => {
				await new Promise((r) => setTimeout(r, 5000));
				return 'done';
			},
		);

		const result = await registry.execute({
			id: 'c3',
			name: 'slow_tool',
			arguments: {},
		});

		expect(result.isError).toBe(true);
	});

	it('fast tool completes normally with timeout configured', async () => {
		const registry = createToolRegistry({
			defaultToolTimeoutMs: 5000,
		});
		registry.register(
			{
				name: 'fast_tool',
				description: 'A fast tool',
				parameters: {},
			},
			async () => 'quick result',
		);

		const result = await registry.execute({
			id: 'c4',
			name: 'fast_tool',
			arguments: {},
		});

		expect(result.isError).toBe(false);
		expect(result.output).toBe('quick result');
	});
});
