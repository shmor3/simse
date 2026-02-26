import { describe, expect, it } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';
import type { ACPStreamChunk } from '../src/ai/acp/types.js';
import { createConversation } from '../src/ai/conversation/conversation.js';
import { createAgenticLoop } from '../src/ai/loop/agentic-loop.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import { createEventBus } from '../src/events/event-bus.js';

// Minimal stub ACP client that returns tool call XML N times, then final text
function createStubACPClient(
	toolCallCount: number,
	toolName = 'read_file',
	toolArgs: Record<string, unknown> = { path: '/etc/hosts' },
): ACPClient {
	let callNum = 0;

	return {
		initialize: async () => {},
		dispose: async () => {},
		listAgents: async () => [],
		getAgent: async () => ({ id: 'stub', name: 'stub', description: '' }),
		generate: async () => ({
			content: '',
			agentId: 'stub',
			serverName: 'stub',
			sessionId: 's1',
		}),
		chat: async () => ({
			content: '',
			agentId: 'stub',
			serverName: 'stub',
			sessionId: 's1',
		}),
		embed: async () => ({
			embeddings: [],
			agentId: 'stub',
			serverName: 'stub',
		}),
		isAvailable: async () => true,
		setPermissionPolicy: () => {},
		listSessions: async () => [],
		loadSession: async () => ({ sessionId: 's1' }),
		deleteSession: async () => {},
		setSessionMode: async () => {},
		setSessionModel: async () => {},
		getSessionModels: async () => undefined,
		getSessionModes: async () => undefined,
		getServerHealth: () => undefined,
		get serverNames() {
			return ['stub'];
		},
		get serverCount() {
			return 1;
		},
		get defaultServerName() {
			return 'stub';
		},
		get defaultAgent() {
			return 'stub';
		},
		async *generateStream(): AsyncGenerator<ACPStreamChunk> {
			callNum++;
			if (callNum <= toolCallCount) {
				const json = JSON.stringify({
					id: `call_${callNum}`,
					name: toolName,
					arguments: toolArgs,
				});
				yield {
					type: 'delta',
					text: `<tool_use>${json}</tool_use>`,
				};
			} else {
				yield { type: 'delta', text: 'Final answer.' };
			}
			yield { type: 'complete' };
		},
	} as unknown as ACPClient;
}

describe('doom loop detection', () => {
	it('fires onDoomLoop callback after 3 consecutive identical tool calls', async () => {
		const conversation = createConversation();
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'read_file', description: 'reads a file', parameters: {} },
			async () => 'file contents',
		);

		const doomLoopEvents: Array<{
			toolName: string;
			callCount: number;
		}> = [];

		const loop = createAgenticLoop({
			acpClient: createStubACPClient(4),
			toolRegistry: registry,
			conversation,
			maxTurns: 10,
			maxIdenticalToolCalls: 3,
		});

		await loop.run('Read the file', {
			onDoomLoop: (toolName, callCount) => {
				doomLoopEvents.push({ toolName, callCount });
			},
		});

		expect(doomLoopEvents.length).toBeGreaterThanOrEqual(1);
		expect(doomLoopEvents[0].toolName).toBe('read_file');
		expect(doomLoopEvents[0].callCount).toBe(3);
	});

	it('publishes loop.doom_loop event', async () => {
		const conversation = createConversation();
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'read_file', description: 'reads a file', parameters: {} },
			async () => 'file contents',
		);

		const eventBus = createEventBus();
		const events: unknown[] = [];
		eventBus.subscribe('loop.doom_loop', (payload) => {
			events.push(payload);
		});

		const loop = createAgenticLoop({
			acpClient: createStubACPClient(4),
			toolRegistry: registry,
			conversation,
			maxTurns: 10,
			maxIdenticalToolCalls: 3,
			eventBus,
		});

		await loop.run('Read the file');

		expect(events.length).toBeGreaterThanOrEqual(1);
	});

	it('does not fire when tool calls differ in args', async () => {
		let callNum = 0;

		const acpClient = {
			async *generateStream(): AsyncGenerator<ACPStreamChunk> {
				callNum++;
				if (callNum <= 3) {
					const json = JSON.stringify({
						id: `call_${callNum}`,
						name: 'read_file',
						arguments: { path: `/file_${callNum}` },
					});
					yield { type: 'delta', text: `<tool_use>${json}</tool_use>` };
				} else {
					yield { type: 'delta', text: 'Done.' };
				}
				yield { type: 'complete' };
			},
		} as unknown as ACPClient;

		const conversation = createConversation();
		const registry = createToolRegistry({});
		registry.register(
			{ name: 'read_file', description: 'reads a file', parameters: {} },
			async () => 'file contents',
		);

		const doomLoopEvents: unknown[] = [];
		const loop = createAgenticLoop({
			acpClient,
			toolRegistry: registry,
			conversation,
			maxTurns: 10,
			maxIdenticalToolCalls: 3,
		});

		await loop.run('Read files', {
			onDoomLoop: () => {
				doomLoopEvents.push(true);
			},
		});

		expect(doomLoopEvents).toHaveLength(0);
	});
});
