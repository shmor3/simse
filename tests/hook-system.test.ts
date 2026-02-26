import { describe, expect, test } from 'bun:test';
import type { ConversationMessage } from '../src/ai/conversation/types.js';
import type { ToolCallRequest, ToolCallResult } from '../src/ai/tools/types.js';
import { createHookSystem } from '../src/hooks/hook-system.js';
import type { BlockedResult } from '../src/hooks/types.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeRequest(overrides?: Partial<ToolCallRequest>): ToolCallRequest {
	return {
		id: 'call-1',
		name: 'test-tool',
		arguments: { input: 'hello' },
		...overrides,
	};
}

function makeResult(overrides?: Partial<ToolCallResult>): ToolCallResult {
	return {
		id: 'call-1',
		name: 'test-tool',
		output: 'world',
		isError: false,
		...overrides,
	};
}

// ---------------------------------------------------------------------------
// tool.execute.before
// ---------------------------------------------------------------------------

describe('tool.execute.before', () => {
	test('modifies request', async () => {
		const hooks = createHookSystem();

		hooks.register('tool.execute.before', async ({ request }) => ({
			...request,
			arguments: { ...request.arguments, injected: true },
		}));

		const result = await hooks.run('tool.execute.before', {
			request: makeRequest(),
		});

		expect(result).toEqual({
			id: 'call-1',
			name: 'test-tool',
			arguments: { input: 'hello', injected: true },
		});
	});

	test('blocks execution with BlockedResult', async () => {
		const hooks = createHookSystem();

		hooks.register('tool.execute.before', async () => ({
			blocked: true as const,
			reason: 'not allowed',
		}));

		const result = await hooks.run('tool.execute.before', {
			request: makeRequest(),
		});

		expect((result as BlockedResult).blocked).toBe(true);
		expect((result as BlockedResult).reason).toBe('not allowed');
	});

	test('stops at first BlockedResult in chain', async () => {
		const hooks = createHookSystem();
		const calls: string[] = [];

		hooks.register('tool.execute.before', async ({ request }) => {
			calls.push('first');
			return request;
		});

		hooks.register('tool.execute.before', async () => {
			calls.push('second');
			return { blocked: true as const, reason: 'blocked by second' };
		});

		hooks.register('tool.execute.before', async ({ request }) => {
			calls.push('third');
			return request;
		});

		const result = await hooks.run('tool.execute.before', {
			request: makeRequest(),
		});

		expect(calls).toEqual(['first', 'second']);
		expect((result as BlockedResult).blocked).toBe(true);
	});

	test('returns original request when no hooks registered', async () => {
		const hooks = createHookSystem();
		const request = makeRequest();

		const result = await hooks.run('tool.execute.before', { request });

		expect(result).toEqual(request);
	});
});

// ---------------------------------------------------------------------------
// tool.execute.after
// ---------------------------------------------------------------------------

describe('tool.execute.after', () => {
	test('modifies result', async () => {
		const hooks = createHookSystem();

		hooks.register('tool.execute.after', async ({ result }) => ({
			...result,
			output: `${result.output} [modified]`,
		}));

		const result = await hooks.run('tool.execute.after', {
			request: makeRequest(),
			result: makeResult(),
		});

		expect(result).toEqual({
			id: 'call-1',
			name: 'test-tool',
			output: 'world [modified]',
			isError: false,
		});
	});

	test('returns original result when no hooks registered', async () => {
		const hooks = createHookSystem();
		const toolResult = makeResult();

		const result = await hooks.run('tool.execute.after', {
			request: makeRequest(),
			result: toolResult,
		});

		expect(result).toEqual(toolResult);
	});
});

// ---------------------------------------------------------------------------
// tool.result.validate
// ---------------------------------------------------------------------------

describe('tool.result.validate', () => {
	test('returns concatenated messages from multiple hooks', async () => {
		const hooks = createHookSystem();

		hooks.register('tool.result.validate', async () => [
			'warning: output too long',
		]);
		hooks.register('tool.result.validate', async () => [
			'info: tool deprecated',
			'info: consider alternative',
		]);

		const messages = await hooks.run('tool.result.validate', {
			request: makeRequest(),
			result: makeResult(),
		});

		expect(messages).toEqual([
			'warning: output too long',
			'info: tool deprecated',
			'info: consider alternative',
		]);
	});

	test('returns empty array when no hooks registered', async () => {
		const hooks = createHookSystem();

		const messages = await hooks.run('tool.result.validate', {
			request: makeRequest(),
			result: makeResult(),
		});

		expect(messages).toEqual([]);
	});
});

// ---------------------------------------------------------------------------
// prompt.system.transform
// ---------------------------------------------------------------------------

describe('prompt.system.transform', () => {
	test('modifies system prompt', async () => {
		const hooks = createHookSystem();

		hooks.register(
			'prompt.system.transform',
			async ({ prompt }) => `${prompt}\nExtra instruction.`,
		);

		const result = await hooks.run('prompt.system.transform', {
			prompt: 'You are a helpful assistant.',
		});

		expect(result).toBe('You are a helpful assistant.\nExtra instruction.');
	});

	test('returns original prompt when no hooks registered', async () => {
		const hooks = createHookSystem();

		const result = await hooks.run('prompt.system.transform', {
			prompt: 'original',
		});

		expect(result).toBe('original');
	});
});

// ---------------------------------------------------------------------------
// prompt.messages.transform
// ---------------------------------------------------------------------------

describe('prompt.messages.transform', () => {
	test('modifies messages array', async () => {
		const hooks = createHookSystem();
		const messages: ConversationMessage[] = [
			{ role: 'user', content: 'hello' },
		];

		hooks.register('prompt.messages.transform', async ({ messages }) => [
			...messages,
			{ role: 'assistant' as const, content: 'injected' },
		]);

		const result = await hooks.run('prompt.messages.transform', { messages });

		expect(result).toEqual([
			{ role: 'user', content: 'hello' },
			{ role: 'assistant', content: 'injected' },
		]);
	});

	test('returns original messages when no hooks registered', async () => {
		const hooks = createHookSystem();
		const messages: ConversationMessage[] = [
			{ role: 'user', content: 'hello' },
		];

		const result = await hooks.run('prompt.messages.transform', { messages });

		expect(result).toEqual(messages);
	});
});

// ---------------------------------------------------------------------------
// session.compacting
// ---------------------------------------------------------------------------

describe('session.compacting', () => {
	test('modifies summary', async () => {
		const hooks = createHookSystem();
		const messages: ConversationMessage[] = [
			{ role: 'user', content: 'hello' },
			{ role: 'assistant', content: 'world' },
		];

		hooks.register(
			'session.compacting',
			async ({ summary }) => `${summary} [compacted]`,
		);

		const result = await hooks.run('session.compacting', {
			messages,
			summary: 'conversation about greetings',
		});

		expect(result).toBe('conversation about greetings [compacted]');
	});

	test('returns original summary when no hooks registered', async () => {
		const hooks = createHookSystem();

		const result = await hooks.run('session.compacting', {
			messages: [],
			summary: 'original summary',
		});

		expect(result).toBe('original summary');
	});
});

// ---------------------------------------------------------------------------
// Unregister
// ---------------------------------------------------------------------------

describe('unregister', () => {
	test('removes hook so it is not called', async () => {
		const hooks = createHookSystem();
		const calls: string[] = [];

		const unsubscribe = hooks.register(
			'prompt.system.transform',
			async ({ prompt }) => {
				calls.push('called');
				return `${prompt} [hook]`;
			},
		);

		// Hook fires
		const first = await hooks.run('prompt.system.transform', {
			prompt: 'base',
		});
		expect(first).toBe('base [hook]');
		expect(calls).toEqual(['called']);

		// Unregister
		unsubscribe();

		// Hook no longer fires
		const second = await hooks.run('prompt.system.transform', {
			prompt: 'base',
		});
		expect(second).toBe('base');
		expect(calls).toEqual(['called']);
	});
});

// ---------------------------------------------------------------------------
// Chaining multiple hooks
// ---------------------------------------------------------------------------

describe('chaining', () => {
	test('chains multiple hooks in registration order', async () => {
		const hooks = createHookSystem();

		hooks.register(
			'prompt.system.transform',
			async ({ prompt }) => `${prompt} [A]`,
		);
		hooks.register(
			'prompt.system.transform',
			async ({ prompt }) => `${prompt} [B]`,
		);
		hooks.register(
			'prompt.system.transform',
			async ({ prompt }) => `${prompt} [C]`,
		);

		const result = await hooks.run('prompt.system.transform', {
			prompt: 'start',
		});

		expect(result).toBe('start [A] [B] [C]');
	});

	test('chains tool.execute.before hooks passing modified request', async () => {
		const hooks = createHookSystem();

		hooks.register('tool.execute.before', async ({ request }) => ({
			...request,
			arguments: { ...request.arguments, step1: true },
		}));

		hooks.register('tool.execute.before', async ({ request }) => ({
			...request,
			arguments: { ...request.arguments, step2: true },
		}));

		const result = await hooks.run('tool.execute.before', {
			request: makeRequest(),
		});

		expect(result).toEqual({
			id: 'call-1',
			name: 'test-tool',
			arguments: { input: 'hello', step1: true, step2: true },
		});
	});

	test('chains tool.execute.after hooks passing modified result', async () => {
		const hooks = createHookSystem();

		hooks.register('tool.execute.after', async ({ result }) => ({
			...result,
			output: `${result.output}+A`,
		}));

		hooks.register('tool.execute.after', async ({ result }) => ({
			...result,
			output: `${result.output}+B`,
		}));

		const result = await hooks.run('tool.execute.after', {
			request: makeRequest(),
			result: makeResult(),
		});

		expect(result).toEqual({
			id: 'call-1',
			name: 'test-tool',
			output: 'world+A+B',
			isError: false,
		});
	});
});
