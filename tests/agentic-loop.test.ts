import { describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';
import { createConversation } from '../src/ai/conversation/conversation.js';
import { createAgenticLoop } from '../src/ai/loop/agentic-loop.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createMockACPClient(
	responses: string[] = ['Final response.'],
): ACPClient {
	let callIdx = 0;

	return {
		initialize: mock(() => Promise.resolve()),
		dispose: mock(() => Promise.resolve()),
		listAgents: mock(() => Promise.resolve([])),
		getAgent: mock(() => Promise.resolve({ id: 'test', name: 'test' })),
		generate: mock(() =>
			Promise.resolve({
				content: responses[callIdx] ?? 'done',
				agentId: 'test',
				serverName: 'test',
				sessionId: 'sess',
			}),
		),
		chat: mock(() =>
			Promise.resolve({
				content: 'chat',
				agentId: 'test',
				serverName: 'test',
				sessionId: 'sess',
			}),
		),
		generateStream: mock(async function* () {
			const response = responses[callIdx++] ?? 'done';
			yield { type: 'delta' as const, text: response };
			yield { type: 'complete' as const, usage: undefined };
		}),
		embed: mock(() =>
			Promise.resolve({
				embeddings: [[]],
				agentId: 'test',
				serverName: 'test',
			}),
		),
		isAvailable: mock(() => Promise.resolve(true)),
		setPermissionPolicy: mock(() => {}),
		listSessions: mock(() => Promise.resolve([])),
		loadSession: mock(() => Promise.resolve({} as any)),
		deleteSession: mock(() => Promise.resolve()),
		setSessionMode: mock(() => Promise.resolve()),
		setSessionModel: mock(() => Promise.resolve()),
		getServerHealth: mock(() => undefined),
		serverNames: ['test'],
		serverCount: 1,
		defaultServerName: 'test',
		defaultAgent: 'test',
	} as ACPClient;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('createAgenticLoop', () => {
	it('returns a frozen object', () => {
		const loop = createAgenticLoop({
			acpClient: createMockACPClient(),
			toolRegistry: createToolRegistry({}),
			conversation: createConversation(),
		});
		expect(Object.isFrozen(loop)).toBe(true);
	});

	it('runs a single text turn with no tool calls', async () => {
		const acpClient = createMockACPClient(['Hello, world!']);
		const conversation = createConversation();
		const toolRegistry = createToolRegistry({});

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry,
			conversation,
		});

		const result = await loop.run('Hi');
		expect(result.finalText).toBe('Hello, world!');
		expect(result.totalTurns).toBe(1);
		expect(result.hitTurnLimit).toBe(false);
		expect(result.aborted).toBe(false);
		expect(result.turns.length).toBe(1);
		expect(result.turns[0].type).toBe('text');
		expect(typeof result.totalDurationMs).toBe('number');
	});

	it('executes tool calls and continues', async () => {
		const acpClient = createMockACPClient([
			'Let me search.\n\n<tool_use>\n{"id": "call_1", "name": "echo", "arguments": {"text": "hi"}}\n</tool_use>',
			'The search returned: hi',
		]);

		const conversation = createConversation();
		const toolRegistry = createToolRegistry({});
		toolRegistry.register(
			{ name: 'echo', description: 'echo tool', parameters: {} },
			async (args) => String(args.text ?? ''),
		);

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry,
			conversation,
		});

		const result = await loop.run('Search for something');
		expect(result.totalTurns).toBe(2);
		expect(result.turns[0].type).toBe('tool_use');
		expect(result.turns[0].toolCalls?.length).toBe(1);
		expect(result.turns[0].toolResults?.length).toBe(1);
		expect(result.turns[0].toolResults?.[0].output).toBe('hi');
		expect(result.turns[1].type).toBe('text');
		expect(result.finalText).toBe('The search returned: hi');
	});

	it('respects maxTurns limit', async () => {
		// Always returns tool calls to force looping
		const responses = Array(5).fill(
			'<tool_use>\n{"name": "noop", "arguments": {}}\n</tool_use>',
		);
		const acpClient = createMockACPClient(responses);
		const conversation = createConversation();
		const toolRegistry = createToolRegistry({});
		toolRegistry.register(
			{ name: 'noop', description: 'noop', parameters: {} },
			async () => 'ok',
		);

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry,
			conversation,
			maxTurns: 3,
		});

		const result = await loop.run('loop forever');
		expect(result.hitTurnLimit).toBe(true);
		expect(result.totalTurns).toBe(3);
	});

	it('aborts on signal before first turn', async () => {
		const controller = new AbortController();
		controller.abort();

		const loop = createAgenticLoop({
			acpClient: createMockACPClient(),
			toolRegistry: createToolRegistry({}),
			conversation: createConversation(),
			signal: controller.signal,
		});

		const result = await loop.run('test');
		expect(result.aborted).toBe(true);
		expect(result.totalTurns).toBe(0);
	});

	it('calls onStreamDelta callback', async () => {
		const acpClient = createMockACPClient(['chunk']);
		const deltas: string[] = [];

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry: createToolRegistry({}),
			conversation: createConversation(),
		});

		await loop.run('test', {
			onStreamDelta: (text) => deltas.push(text),
		});

		expect(deltas).toEqual(['chunk']);
	});

	it('calls onTurnComplete callback', async () => {
		const acpClient = createMockACPClient(['done']);
		const completedTurns: number[] = [];

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry: createToolRegistry({}),
			conversation: createConversation(),
		});

		await loop.run('test', {
			onTurnComplete: (turn) => completedTurns.push(turn.turn),
		});

		expect(completedTurns).toEqual([1]);
	});

	it('calls onToolCallStart and onToolCallEnd callbacks', async () => {
		const acpClient = createMockACPClient([
			'<tool_use>\n{"name": "echo", "arguments": {"x": 1}}\n</tool_use>',
			'done',
		]);

		const starts: string[] = [];
		const ends: string[] = [];

		const toolRegistry = createToolRegistry({});
		toolRegistry.register(
			{ name: 'echo', description: 'echo', parameters: {} },
			async () => 'ok',
		);

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry,
			conversation: createConversation(),
		});

		await loop.run('test', {
			onToolCallStart: (call) => starts.push(call.name),
			onToolCallEnd: (result) => ends.push(result.name),
		});

		expect(starts).toEqual(['echo']);
		expect(ends).toEqual(['echo']);
	});

	it('calls onError when stream fails', async () => {
		const acpClient = createMockACPClient();
		// Override generateStream to throw
		// biome-ignore lint/correctness/useYield: test needs a throwing generator
		(acpClient as any).generateStream = async function* () {
			throw new Error('connection lost');
		};

		const errors: string[] = [];

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry: createToolRegistry({}),
			conversation: createConversation(),
		});

		const result = await loop.run('test', {
			onError: (err) => errors.push(err.message),
		});

		expect(errors.length).toBeGreaterThan(0);
		expect(result.finalText).toContain('Error');
	});

	it('handles empty response from model', async () => {
		const acpClient = createMockACPClient(['']);
		const errors: string[] = [];

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry: createToolRegistry({}),
			conversation: createConversation(),
		});

		await loop.run('test', {
			onError: (err) => errors.push(err.message),
		});

		expect(errors).toContain('No response received from model.');
	});

	it('sets system prompt from toolRegistry + custom prompt', async () => {
		const conversation = createConversation();
		const toolRegistry = createToolRegistry({});
		toolRegistry.register(
			{ name: 'test', description: 'test tool', parameters: {} },
			async () => 'ok',
		);

		const acpClient = createMockACPClient(['response']);

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry,
			conversation,
			systemPrompt: 'You are helpful.',
		});

		await loop.run('hello');

		// The system prompt should include tool definitions and custom prompt
		const msgs = conversation.toMessages();
		const systemMsg = msgs.find((m) => m.role === 'system');
		expect(systemMsg).toBeDefined();
		expect(systemMsg!.content).toContain('test tool');
		expect(systemMsg!.content).toContain('You are helpful.');
	});

	it('auto-compacts when enabled and conversation needs compaction', async () => {
		const conversation = createConversation({ autoCompactChars: 10 });
		// Pre-fill conversation to trigger compaction
		conversation.addUser('a'.repeat(20));
		conversation.addAssistant('b'.repeat(20));

		const compactionProvider = {
			generate: mock(async () => 'summary of conversation'),
		};

		const acpClient = createMockACPClient(['final']);
		const compactions: string[] = [];

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry: createToolRegistry({}),
			conversation,
			autoCompact: true,
			compactionProvider,
		});

		await loop.run('continue', {
			onCompaction: (summary) => compactions.push(summary),
		});

		expect(compactionProvider.generate).toHaveBeenCalled();
		expect(compactions).toEqual(['summary of conversation']);
	});

	it('retries stream on transient error', async () => {
		let streamCalls = 0;
		const acpClient = createMockACPClient();
		(acpClient as any).generateStream = async function* () {
			streamCalls++;
			if (streamCalls === 1) {
				// First attempt: throw transient error (must match isTransientError patterns)
				const err = new Error('ECONNRESET');
				throw err;
			}
			// Second attempt: succeed
			yield { type: 'delta' as const, text: 'recovered' };
			yield { type: 'complete' as const, usage: undefined };
		};

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry: createToolRegistry({}),
			conversation: createConversation(),
			streamRetry: { maxAttempts: 3, baseDelayMs: 10 },
		});

		const result = await loop.run('test');
		expect(result.finalText).toBe('recovered');
		expect(streamCalls).toBe(2);
	});

	it('retries tool execution on transient-looking error', async () => {
		let toolCalls = 0;
		const acpClient = createMockACPClient([
			'<tool_use>\n{"name": "flaky", "arguments": {}}\n</tool_use>',
			'done',
		]);

		const toolRegistry = createToolRegistry({});
		toolRegistry.register(
			{ name: 'flaky', description: 'flaky tool', parameters: {} },
			async () => {
				toolCalls++;
				if (toolCalls === 1) {
					// Return transient-looking error on first call
					throw new Error('timeout waiting for response');
				}
				return 'success';
			},
		);

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry,
			conversation: createConversation(),
			toolRetry: { maxAttempts: 3, baseDelayMs: 10 },
		});

		const result = await loop.run('test');
		expect(result.turns[0].toolResults?.[0].output).toBe('success');
		expect(toolCalls).toBe(2);
	});

	it('does not retry tool on non-transient error', async () => {
		let toolCalls = 0;
		const acpClient = createMockACPClient([
			'<tool_use>\n{"name": "bad", "arguments": {}}\n</tool_use>',
			'done',
		]);

		const toolRegistry = createToolRegistry({});
		toolRegistry.register(
			{ name: 'bad', description: 'bad tool', parameters: {} },
			async () => {
				toolCalls++;
				throw new Error('invalid argument provided');
			},
		);

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry,
			conversation: createConversation(),
			toolRetry: { maxAttempts: 3, baseDelayMs: 10 },
		});

		await loop.run('test');
		// Tool registry catches the throw and returns isError: true with the message,
		// but the message does not match transient patterns so no retry
		expect(toolCalls).toBe(1);
	});

	it('stream retry exhausts and reports error', async () => {
		const acpClient = createMockACPClient();
		// biome-ignore lint/correctness/useYield: test needs a throwing generator
		(acpClient as any).generateStream = async function* () {
			const err = new Error('ETIMEDOUT');
			throw err;
		};

		const errors: string[] = [];

		const loop = createAgenticLoop({
			acpClient,
			toolRegistry: createToolRegistry({}),
			conversation: createConversation(),
			streamRetry: { maxAttempts: 2, baseDelayMs: 10 },
		});

		const result = await loop.run('test', {
			onError: (err) => errors.push(err.message),
		});

		expect(errors.length).toBeGreaterThan(0);
		expect(result.finalText).toContain('Error');
	});
});
