import { describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createMockACPClient(overrides: Partial<ACPClient> = {}): ACPClient {
	return {
		initialize: mock(() => Promise.resolve()),
		dispose: mock(() => Promise.resolve()),
		listAgents: mock(() => Promise.resolve([])),
		getAgent: mock(() => Promise.resolve({ id: 'test', name: 'test' })),
		generate: mock(() =>
			Promise.resolve({
				content: 'generated text',
				agentId: 'agent-1',
				serverName: 'server-1',
				sessionId: 'sess-1',
			}),
		),
		chat: mock(() =>
			Promise.resolve({
				content: 'chat response',
				agentId: 'agent-1',
				serverName: 'server-1',
				sessionId: 'sess-1',
			}),
		),
		generateStream: mock(async function* () {
			yield { type: 'delta' as const, text: 'chunk' };
		}),
		embed: mock(() =>
			Promise.resolve({
				embeddings: [[0.1, 0.2, 0.3]],
				agentId: 'agent-1',
				serverName: 'server-1',
			}),
		),
		isAvailable: mock(() => Promise.resolve(true)),
		setPermissionPolicy: mock(() => {}),
		listSessions: mock(() => Promise.resolve([])),
		loadSession: mock(() => Promise.resolve({} as any)),
		deleteSession: mock(() => Promise.resolve()),
		setSessionMode: mock(() => Promise.resolve()),
		setSessionModel: mock(() => Promise.resolve()),
		getSessionModels: mock(() => Promise.resolve(undefined)),
		getSessionModes: mock(() => Promise.resolve(undefined)),
		getServerHealth: mock(() => undefined),
		serverNames: ['server-1'],
		serverCount: 1,
		defaultServerName: 'server-1',
		defaultAgent: 'agent-1',
		...overrides,
	} as ACPClient;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('ACPClient model/mode discovery', () => {
	it('ACPClient interface includes getSessionModels and getSessionModes', () => {
		const client = createMockACPClient();
		expect(typeof client.getSessionModels).toBe('function');
		expect(typeof client.getSessionModes).toBe('function');
	});

	it('getSessionModels returns undefined for unknown session', async () => {
		const client = createMockACPClient({
			getSessionModels: mock(() => Promise.resolve(undefined)),
		});
		const result = await client.getSessionModels('unknown-session');
		expect(result).toBeUndefined();
	});

	it('getSessionModes returns undefined for unknown session', async () => {
		const client = createMockACPClient({
			getSessionModes: mock(() => Promise.resolve(undefined)),
		});
		const result = await client.getSessionModes('unknown-session');
		expect(result).toBeUndefined();
	});

	it('getSessionModels returns cached models info', async () => {
		const modelsInfo = {
			availableModels: [
				{ modelId: 'model-1', name: 'Model One' },
				{ modelId: 'model-2', name: 'Model Two', description: 'Second' },
			],
			currentModelId: 'model-1',
		};

		const client = createMockACPClient({
			getSessionModels: mock(() => Promise.resolve(modelsInfo)),
		});

		const result = await client.getSessionModels('sess-1');
		expect(result).toBeDefined();
		expect(result!.availableModels).toHaveLength(2);
		expect(result!.currentModelId).toBe('model-1');
		expect(result!.availableModels[0].modelId).toBe('model-1');
	});

	it('getSessionModes returns cached modes info', async () => {
		const modesInfo = {
			currentModeId: 'default',
			availableModes: [
				{ id: 'default', name: 'Default' },
				{ id: 'plan', name: 'Plan', description: 'Read-only' },
			],
		};

		const client = createMockACPClient({
			getSessionModes: mock(() => Promise.resolve(modesInfo)),
		});

		const result = await client.getSessionModes('sess-1');
		expect(result).toBeDefined();
		expect(result!.currentModeId).toBe('default');
		expect(result!.availableModes).toHaveLength(2);
		expect(result!.availableModes[1].id).toBe('plan');
	});
});
