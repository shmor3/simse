import { describe, expect, it } from 'bun:test';
import { createConversation } from '../conversation.js';
import type { LoopTurn } from '../loop.js';
import { createAgenticLoop } from '../loop.js';
import type {
	ToolCallRequest,
	ToolCallResult,
	ToolRegistry,
} from '../tool-registry.js';

// ---------------------------------------------------------------------------
// Helpers — mock ACPClient and ToolRegistry
// ---------------------------------------------------------------------------

interface MockACPClientOptions {
	readonly responses: readonly string[];
}

function createMockACPClient(options: MockACPClientOptions) {
	let callIdx = 0;
	const prompts: string[] = [];

	return {
		generateStream(prompt: string, _opts?: Record<string, unknown>) {
			prompts.push(prompt);
			const response = options.responses[callIdx] ?? '';
			callIdx++;

			return {
				async *[Symbol.asyncIterator]() {
					// Yield entire response as a single delta
					yield { type: 'delta' as const, text: response };
				},
			};
		},
		get promptsSent() {
			return prompts;
		},
		get callCount() {
			return callIdx;
		},
	};
}

function createMockToolRegistry(
	toolHandlers: Record<string, (args: Record<string, unknown>) => string>,
): ToolRegistry {
	return Object.freeze({
		discover: async () => {},
		getToolDefinitions: () => [],
		formatForSystemPrompt: () => 'Tools available: test',
		execute: async (call: ToolCallRequest): Promise<ToolCallResult> => {
			const handler = toolHandlers[call.name];
			if (!handler) {
				return Object.freeze({
					id: call.id,
					name: call.name,
					output: `Unknown tool: "${call.name}"`,
					isError: true,
				});
			}
			return Object.freeze({
				id: call.id,
				name: call.name,
				output: handler(call.arguments),
				isError: false,
			});
		},
		get toolCount() {
			return Object.keys(toolHandlers).length;
		},
	});
}

// ---------------------------------------------------------------------------
// createAgenticLoop
// ---------------------------------------------------------------------------

describe('createAgenticLoop', () => {
	it('should return a frozen object', () => {
		const loop = createAgenticLoop({
			acpClient: createMockACPClient({ responses: [] }) as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});
		expect(Object.isFrozen(loop)).toBe(true);
	});

	it('should return final text when no tool calls in response', async () => {
		const acpClient = createMockACPClient({
			responses: ['Hello! How can I help?'],
		});
		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});

		const result = await loop.run('Hi there');
		expect(result.finalText).toBe('Hello! How can I help?');
		expect(result.totalTurns).toBe(1);
		expect(result.hitTurnLimit).toBe(false);
		expect(result.aborted).toBe(false);
	});

	it('should parse and execute tool calls', async () => {
		const acpClient = createMockACPClient({
			responses: [
				'Let me search.\n<tool_use>\n{"id": "call_1", "name": "test_tool", "arguments": {"q": "hello"}}\n</tool_use>',
				'Found results!',
			],
		});

		const executed: string[] = [];
		const toolRegistry = createMockToolRegistry({
			test_tool: (args) => {
				executed.push(String(args.q));
				return 'result data';
			},
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry,
			conversation: createConversation(),
		});

		const result = await loop.run('Search for hello');
		expect(result.finalText).toBe('Found results!');
		expect(result.totalTurns).toBe(2);
		expect(executed).toEqual(['hello']);
	});

	it('should handle multiple tool calls in one response', async () => {
		const acpClient = createMockACPClient({
			responses: [
				[
					'Calling two tools.',
					'<tool_use>',
					'{"id": "call_1", "name": "tool_a", "arguments": {}}',
					'</tool_use>',
					'<tool_use>',
					'{"id": "call_2", "name": "tool_b", "arguments": {}}',
					'</tool_use>',
				].join('\n'),
				'Done!',
			],
		});

		const called: string[] = [];
		const toolRegistry = createMockToolRegistry({
			tool_a: () => {
				called.push('a');
				return 'result_a';
			},
			tool_b: () => {
				called.push('b');
				return 'result_b';
			},
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry,
			conversation: createConversation(),
		});

		const result = await loop.run('Do both');
		expect(called).toEqual(['a', 'b']);
		expect(result.finalText).toBe('Done!');
		expect(result.totalTurns).toBe(2);
	});

	it('should respect maxTurns limit', async () => {
		// Every response has a tool call — loop should stop at maxTurns
		const acpClient = createMockACPClient({
			responses: [
				'<tool_use>\n{"id": "c1", "name": "t", "arguments": {}}\n</tool_use>',
				'<tool_use>\n{"id": "c2", "name": "t", "arguments": {}}\n</tool_use>',
				'<tool_use>\n{"id": "c3", "name": "t", "arguments": {}}\n</tool_use>',
			],
		});

		const toolRegistry = createMockToolRegistry({
			t: () => 'ok',
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry,
			conversation: createConversation(),
			maxTurns: 2,
		});

		const result = await loop.run('Loop forever');
		expect(result.hitTurnLimit).toBe(true);
		expect(result.totalTurns).toBe(2);
	});

	it('should use default maxTurns of 10', async () => {
		// Just verify it doesn't crash — we won't actually run 10 turns
		const acpClient = createMockACPClient({
			responses: ['Simple response'],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});

		const result = await loop.run('test');
		expect(result.totalTurns).toBe(1);
	});

	// -- Abort signal ----------------------------------------------------------

	it('should abort before first turn when signal is already aborted', async () => {
		const controller = new AbortController();
		controller.abort();

		const acpClient = createMockACPClient({
			responses: ['Should not reach'],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
			signal: controller.signal,
		});

		const result = await loop.run('test');
		expect(result.aborted).toBe(true);
		expect(result.totalTurns).toBe(0);
		expect(acpClient.callCount).toBe(0);
	});

	// -- Callbacks -------------------------------------------------------------

	it('should call onStreamDelta with response text', async () => {
		const acpClient = createMockACPClient({
			responses: ['Hello world'],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});

		const deltas: string[] = [];
		await loop.run('test', {
			onStreamDelta: (text) => deltas.push(text),
		});

		expect(deltas).toEqual(['Hello world']);
	});

	it('should call onStreamStart before streaming', async () => {
		const acpClient = createMockACPClient({
			responses: ['response'],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});

		let started = false;
		await loop.run('test', {
			onStreamStart: () => {
				started = true;
			},
		});

		expect(started).toBe(true);
	});

	it('should call onToolCallStart and onToolCallEnd', async () => {
		const acpClient = createMockACPClient({
			responses: [
				'<tool_use>\n{"id": "c1", "name": "my_tool", "arguments": {"x": 1}}\n</tool_use>',
				'Done',
			],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({
				my_tool: () => 'result',
			}),
			conversation: createConversation(),
		});

		const starts: ToolCallRequest[] = [];
		const ends: ToolCallResult[] = [];

		await loop.run('test', {
			onToolCallStart: (call) => starts.push(call),
			onToolCallEnd: (result) => ends.push(result),
		});

		expect(starts).toHaveLength(1);
		expect(starts[0].name).toBe('my_tool');
		expect(ends).toHaveLength(1);
		expect(ends[0].output).toBe('result');
		expect(ends[0].isError).toBe(false);
	});

	it('should call onTurnComplete for each turn', async () => {
		const acpClient = createMockACPClient({
			responses: [
				'<tool_use>\n{"id": "c1", "name": "t", "arguments": {}}\n</tool_use>',
				'Final answer',
			],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({
				t: () => 'ok',
			}),
			conversation: createConversation(),
		});

		const completedTurns: LoopTurn[] = [];
		await loop.run('test', {
			onTurnComplete: (turn) => completedTurns.push(turn),
		});

		expect(completedTurns).toHaveLength(2);
		expect(completedTurns[0].type).toBe('tool_use');
		expect(completedTurns[1].type).toBe('text');
		expect(completedTurns[1].text).toBe('Final answer');
	});

	it('should call onError when streaming fails', async () => {
		const acpClient = {
			generateStream() {
				return {
					[Symbol.asyncIterator]() {
						return {
							next() {
								return Promise.reject(new Error('Connection failed'));
							},
						};
					},
				};
			},
		};

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});

		const errors: Error[] = [];
		const result = await loop.run('test', {
			onError: (err) => errors.push(err),
		});

		expect(errors.length).toBeGreaterThan(0);
		expect(result.finalText).toContain('Error');
	});

	// -- Turn tracking ---------------------------------------------------------

	it('should record turns in result', async () => {
		const acpClient = createMockACPClient({
			responses: [
				'<tool_use>\n{"id": "c1", "name": "t", "arguments": {}}\n</tool_use>',
				'Done',
			],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({
				t: () => 'ok',
			}),
			conversation: createConversation(),
		});

		const result = await loop.run('test');
		expect(result.turns).toHaveLength(2);
		expect(result.turns[0].turn).toBe(1);
		expect(result.turns[0].type).toBe('tool_use');
		expect(result.turns[0].toolCalls).toHaveLength(1);
		expect(result.turns[0].toolResults).toHaveLength(1);
		expect(result.turns[1].turn).toBe(2);
		expect(result.turns[1].type).toBe('text');
	});

	it('should freeze the result object', async () => {
		const acpClient = createMockACPClient({
			responses: ['response'],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});

		const result = await loop.run('test');
		expect(Object.isFrozen(result)).toBe(true);
		expect(Object.isFrozen(result.turns)).toBe(true);
	});

	// -- Unknown tool -----------------------------------------------------------

	it('should handle unknown tool calls gracefully', async () => {
		const acpClient = createMockACPClient({
			responses: [
				'<tool_use>\n{"id": "c1", "name": "nonexistent", "arguments": {}}\n</tool_use>',
				'OK',
			],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});

		const results: ToolCallResult[] = [];
		const finalResult = await loop.run('test', {
			onToolCallEnd: (r) => results.push(r),
		});

		expect(results).toHaveLength(1);
		expect(results[0].isError).toBe(true);
		expect(results[0].output).toContain('Unknown tool');
		expect(finalResult.finalText).toBe('OK');
	});

	// -- Empty response --------------------------------------------------------

	it('should handle empty model response', async () => {
		const acpClient = createMockACPClient({
			responses: [''],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});

		const errors: Error[] = [];
		const result = await loop.run('test', {
			onError: (err) => errors.push(err),
		});

		expect(errors.length).toBeGreaterThan(0);
		expect(result.finalText).toContain('No response');
	});

	// -- Tool call ID auto-generation ------------------------------------------

	it('should auto-generate tool call IDs when missing', async () => {
		const acpClient = createMockACPClient({
			responses: [
				'<tool_use>\n{"name": "t", "arguments": {}}\n</tool_use>',
				'Done',
			],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({
				t: () => 'ok',
			}),
			conversation: createConversation(),
		});

		const starts: ToolCallRequest[] = [];
		await loop.run('test', {
			onToolCallStart: (call) => starts.push(call),
		});

		expect(starts).toHaveLength(1);
		expect(starts[0].id).toBe('call_1');
	});

	// -- Malformed tool call JSON -----------------------------------------------

	it('should skip malformed tool call JSON', async () => {
		const acpClient = createMockACPClient({
			responses: [
				'Some text <tool_use>\n{invalid json}\n</tool_use> more text',
			],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: createConversation(),
		});

		const result = await loop.run('test');
		// Should treat as text-only since the tool call JSON was malformed
		expect(result.totalTurns).toBe(1);
		expect(result.turns[0].type).toBe('text');
		expect(result.finalText).toContain('Some text');
		expect(result.finalText).toContain('more text');
	});

	// -- Conversation accumulation ---------------------------------------------

	it('should add user input and assistant response to conversation', async () => {
		const conv = createConversation();
		const acpClient = createMockACPClient({
			responses: ['I am the assistant'],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({}),
			conversation: conv,
		});

		await loop.run('User says hello');

		// Conversation should have system + user + assistant messages
		// (loop injects a system prompt with tool definitions)
		const msgs = conv.toMessages();
		expect(msgs[0].role).toBe('system');
		// Find user and assistant messages (skip system prompt)
		const userMsg = msgs.find((m) => m.role === 'user');
		const assistantMsg = msgs.find((m) => m.role === 'assistant');
		expect(userMsg?.content).toBe('User says hello');
		expect(assistantMsg?.content).toBe('I am the assistant');
	});

	it('should add tool results to conversation', async () => {
		const conv = createConversation();
		const acpClient = createMockACPClient({
			responses: [
				'<tool_use>\n{"id": "c1", "name": "t", "arguments": {}}\n</tool_use>',
				'Done',
			],
		});

		const loop = createAgenticLoop({
			acpClient: acpClient as never,
			toolRegistry: createMockToolRegistry({
				t: () => 'tool output',
			}),
			conversation: conv,
		});

		await loop.run('test');

		// system + user + assistant(tool_use) + tool_result + assistant(done)
		const msgs = conv.toMessages();
		const toolResultMsg = msgs.find((m) => m.role === 'tool_result');
		expect(toolResultMsg).toBeDefined();
		expect(toolResultMsg?.content).toBe('tool output');
	});
});
