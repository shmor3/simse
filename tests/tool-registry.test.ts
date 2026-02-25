import { describe, expect, it, mock } from 'bun:test';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import type { ToolCallRequest, ToolDefinition } from '../src/ai/tools/types.js';

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('createToolRegistry', () => {
	it('returns a frozen object', () => {
		const registry = createToolRegistry({});
		expect(Object.isFrozen(registry)).toBe(true);
	});

	it('starts with zero tools when no builtins configured', () => {
		const registry = createToolRegistry({});
		expect(registry.toolCount).toBe(0);
		expect(registry.toolNames).toEqual([]);
	});

	it('register adds a tool', () => {
		const registry = createToolRegistry({});
		const def: ToolDefinition = {
			name: 'test_tool',
			description: 'A test tool',
			parameters: {
				input: { type: 'string', description: 'input text', required: true },
			},
		};
		registry.register(def, async () => 'ok');
		expect(registry.toolCount).toBe(1);
		expect(registry.toolNames).toEqual(['test_tool']);
	});

	it('unregister removes a tool', () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'temp', description: 'temp', parameters: {} },
			async () => 'ok',
		);
		expect(registry.toolCount).toBe(1);
		const removed = registry.unregister('temp');
		expect(removed).toBe(true);
		expect(registry.toolCount).toBe(0);
	});

	it('unregister returns false for unknown tool', () => {
		const registry = createToolRegistry({});
		expect(registry.unregister('nonexistent')).toBe(false);
	});

	it('getToolDefinitions returns all registered tools', () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'a', description: 'Tool A', parameters: {} },
			async () => 'a',
		);
		registry.register(
			{ name: 'b', description: 'Tool B', parameters: {} },
			async () => 'b',
		);
		const defs = registry.getToolDefinitions();
		expect(defs.length).toBe(2);
		expect(defs.map((d) => d.name)).toEqual(['a', 'b']);
	});

	it('formatForSystemPrompt includes tool info', () => {
		const registry = createToolRegistry({});
		registry.register(
			{
				name: 'my_tool',
				description: 'Does stuff',
				parameters: {
					query: { type: 'string', description: 'query', required: true },
				},
			},
			async () => 'ok',
		);
		const prompt = registry.formatForSystemPrompt();
		expect(prompt).toContain('my_tool');
		expect(prompt).toContain('Does stuff');
		expect(prompt).toContain('query (string, required)');
		expect(prompt).toContain('<tool_use>');
	});

	it('formatForSystemPrompt returns empty string when no tools', () => {
		const registry = createToolRegistry({});
		expect(registry.formatForSystemPrompt()).toBe('');
	});

	it('execute calls the handler and returns result', async () => {
		const registry = createToolRegistry({});
		const handler = mock(async () => 'result text');
		registry.register(
			{ name: 'echo', description: 'echo', parameters: {} },
			handler,
		);
		const call: ToolCallRequest = {
			id: 'call_1',
			name: 'echo',
			arguments: { text: 'hello' },
		};
		const result = await registry.execute(call);
		expect(result.id).toBe('call_1');
		expect(result.name).toBe('echo');
		expect(result.output).toBe('result text');
		expect(result.isError).toBe(false);
		expect(typeof result.durationMs).toBe('number');
		expect(handler).toHaveBeenCalledWith({ text: 'hello' });
	});

	it('execute returns error for unknown tool', async () => {
		const registry = createToolRegistry({});
		const result = await registry.execute({
			id: 'call_1',
			name: 'unknown',
			arguments: {},
		});
		expect(result.isError).toBe(true);
		expect(result.output).toContain('not found');
	});

	it('execute catches handler errors', async () => {
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'fail', description: 'fail', parameters: {} },
			async () => {
				throw new Error('boom');
			},
		);
		const result = await registry.execute({
			id: 'call_1',
			name: 'fail',
			arguments: {},
		});
		expect(result.isError).toBe(true);
		expect(result.output).toContain('boom');
	});

	it('execute checks permission resolver', async () => {
		const permissionResolver = {
			check: mock(async () => false),
		};
		const registry = createToolRegistry({ permissionResolver });
		registry.register(
			{ name: 'guarded', description: 'guarded', parameters: {} },
			async () => 'ok',
		);
		const result = await registry.execute({
			id: 'call_1',
			name: 'guarded',
			arguments: {},
		});
		expect(result.isError).toBe(true);
		expect(result.output).toContain('Permission denied');
	});

	it('execute allows when permission resolver returns true', async () => {
		const permissionResolver = {
			check: mock(async () => true),
		};
		const registry = createToolRegistry({ permissionResolver });
		registry.register(
			{ name: 'guarded', description: 'guarded', parameters: {} },
			async () => 'allowed',
		);
		const result = await registry.execute({
			id: 'call_1',
			name: 'guarded',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toBe('allowed');
	});
});

describe('parseToolCalls', () => {
	it('parses a single tool call', () => {
		const registry = createToolRegistry({});
		const response = `Let me search for that.

<tool_use>
{"id": "call_1", "name": "memory_search", "arguments": {"query": "hello"}}
</tool_use>`;

		const result = registry.parseToolCalls(response);
		expect(result.toolCalls.length).toBe(1);
		expect(result.toolCalls[0].name).toBe('memory_search');
		expect(result.toolCalls[0].arguments).toEqual({ query: 'hello' });
		expect(result.text).toBe('Let me search for that.');
	});

	it('parses multiple tool calls', () => {
		const registry = createToolRegistry({});
		const response = `I will do two things.

<tool_use>
{"id": "call_1", "name": "tool_a", "arguments": {}}
</tool_use>

<tool_use>
{"id": "call_2", "name": "tool_b", "arguments": {"x": 1}}
</tool_use>`;

		const result = registry.parseToolCalls(response);
		expect(result.toolCalls.length).toBe(2);
		expect(result.toolCalls[0].name).toBe('tool_a');
		expect(result.toolCalls[1].name).toBe('tool_b');
	});

	it('generates IDs when missing', () => {
		const registry = createToolRegistry({});
		const response = `<tool_use>
{"name": "my_tool", "arguments": {}}
</tool_use>`;

		const result = registry.parseToolCalls(response);
		expect(result.toolCalls[0].id).toBe('call_1');
	});

	it('skips malformed JSON', () => {
		const registry = createToolRegistry({});
		const response = `<tool_use>
not valid json
</tool_use>

<tool_use>
{"name": "valid_tool", "arguments": {}}
</tool_use>`;

		const result = registry.parseToolCalls(response);
		expect(result.toolCalls.length).toBe(1);
		expect(result.toolCalls[0].name).toBe('valid_tool');
	});

	it('returns empty tool calls when none present', () => {
		const registry = createToolRegistry({});
		const result = registry.parseToolCalls('Just a plain response.');
		expect(result.toolCalls.length).toBe(0);
		expect(result.text).toBe('Just a plain response.');
	});

	it('strips tool_use blocks from text', () => {
		const registry = createToolRegistry({});
		const response = `Before.

<tool_use>
{"name": "x", "arguments": {}}
</tool_use>

After.`;
		const result = registry.parseToolCalls(response);
		expect(result.text).toBe('Before.\n\n\n\nAfter.');
	});
});
