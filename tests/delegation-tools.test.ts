import { describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';
import {
	_resetDelegationCounter,
	registerDelegationTools,
} from '../src/ai/tools/delegation-tools.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createMockACPClient(
	serverNames: string[] = ['main', 'ollama'],
	content = 'delegated result',
): ACPClient {
	return {
		initialize: mock(() => Promise.resolve()),
		dispose: mock(() => Promise.resolve()),
		listAgents: mock(() => Promise.resolve([])),
		getAgent: mock(() => Promise.resolve({ id: 'test', name: 'test' })),
		generate: mock(() =>
			Promise.resolve({
				content,
				agentId: 'test',
				serverName: serverNames[0],
				sessionId: 'sess',
			}),
		),
		chat: mock(() =>
			Promise.resolve({
				content: 'chat',
				agentId: 'test',
				serverName: serverNames[0],
				sessionId: 'sess',
			}),
		),
		generateStream: mock(async function* () {
			yield { type: 'delta' as const, text: content };
			yield { type: 'complete' as const, usage: undefined };
		}),
		embed: mock(() =>
			Promise.resolve({
				embeddings: [[]],
				agentId: 'test',
				serverName: serverNames[0],
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
		getServerModelInfo: mock(() => Promise.resolve(undefined)),
		getServerStatuses: mock(() => Promise.resolve([])),
		getSessionModels: mock(() => Promise.resolve(undefined)),
		getSessionModes: mock(() => Promise.resolve(undefined)),
		serverNames,
		serverCount: serverNames.length,
		defaultServerName: serverNames[0],
		defaultAgent: 'test',
	} as ACPClient;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('registerDelegationTools', () => {
	it('registers a delegation tool for each non-primary server', () => {
		_resetDelegationCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient(['main', 'ollama', 'local']);

		registerDelegationTools(registry, {
			acpClient,

			primaryServer: 'main',
		});

		expect(registry.toolNames).toContain('delegate_ollama');
		expect(registry.toolNames).toContain('delegate_local');
		expect(registry.toolNames).not.toContain('delegate_main');
	});

	it('registers no tools when only one server exists', () => {
		_resetDelegationCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient(['main']);

		registerDelegationTools(registry, {
			acpClient,

			primaryServer: 'main',
		});

		const delegationTools = registry.toolNames.filter((n) =>
			n.startsWith('delegate_'),
		);
		expect(delegationTools).toHaveLength(0);
	});

	it('sanitizes server names with special characters', () => {
		_resetDelegationCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient(['main', 'my-server.local']);

		registerDelegationTools(registry, {
			acpClient,

			primaryServer: 'main',
		});

		expect(registry.toolNames).toContain('delegate_my_server_local');
	});

	it('executes delegation tool and returns content', async () => {
		_resetDelegationCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient(['main', 'ollama'], 'ollama says hi');

		registerDelegationTools(registry, {
			acpClient,

			primaryServer: 'main',
		});

		const result = await registry.execute({
			id: 'call_1',
			name: 'delegate_ollama',
			arguments: { task: 'Say hello' },
		});

		expect(result.output).toBe('ollama says hi');
		expect(result.isError).toBe(false);
		expect(acpClient.generate).toHaveBeenCalledWith('Say hello', {
			serverName: 'ollama',
			systemPrompt: undefined,
		});
	});

	it('passes systemPrompt to generate', async () => {
		_resetDelegationCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient(['main', 'ollama']);

		registerDelegationTools(registry, {
			acpClient,

			primaryServer: 'main',
		});

		await registry.execute({
			id: 'call_2',
			name: 'delegate_ollama',
			arguments: { task: 'Translate', systemPrompt: 'You are a translator' },
		});

		expect(acpClient.generate).toHaveBeenCalledWith('Translate', {
			serverName: 'ollama',
			systemPrompt: 'You are a translator',
		});
	});

	it('fires onDelegationStart and onDelegationComplete callbacks', async () => {
		_resetDelegationCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient(['main', 'ollama']);

		const startCalls: string[] = [];
		const completeCalls: string[] = [];

		registerDelegationTools(registry, {
			acpClient,

			primaryServer: 'main',
			callbacks: {
				onDelegationStart: (info) => startCalls.push(info.id),
				onDelegationComplete: (id) => completeCalls.push(id),
			},
		});

		await registry.execute({
			id: 'call_3',
			name: 'delegate_ollama',
			arguments: { task: 'test' },
		});

		expect(startCalls).toHaveLength(1);
		expect(completeCalls).toHaveLength(1);
		expect(startCalls[0]).toBe('del_1');
	});

	it('fires onDelegationError on failure', async () => {
		_resetDelegationCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient(['main', 'ollama']);
		(acpClient.generate as any).mockImplementation(() =>
			Promise.reject(new Error('connection failed')),
		);

		const errorCalls: string[] = [];

		registerDelegationTools(registry, {
			acpClient,

			primaryServer: 'main',
			callbacks: {
				onDelegationError: (id) => errorCalls.push(id),
			},
		});

		const result = await registry.execute({
			id: 'call_4',
			name: 'delegate_ollama',
			arguments: { task: 'test' },
		});

		expect(result.isError).toBe(true);
		expect(errorCalls).toHaveLength(1);
	});

	it('registers no tools when primaryServer is not specified', () => {
		_resetDelegationCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient(['main', 'ollama']);

		registerDelegationTools(registry, {
			acpClient,

			// No primaryServer — all servers get delegation tools
		});

		expect(registry.toolNames).toContain('delegate_main');
		expect(registry.toolNames).toContain('delegate_ollama');
	});
});
