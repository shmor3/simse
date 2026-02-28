import { describe, expect, it, mock } from 'bun:test';
import {
	createACPEmbedder,
	createACPGenerator,
} from '../src/ai/acp/acp-adapters.js';
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

describe('createACPEmbedder', () => {
	it('returns a frozen EmbeddingProvider', () => {
		const client = createMockACPClient();
		const embedder = createACPEmbedder({ client });
		expect(Object.isFrozen(embedder)).toBe(true);
		expect(typeof embedder.embed).toBe('function');
	});

	it('delegates single string to client.embed', async () => {
		const client = createMockACPClient();
		const embedder = createACPEmbedder({ client });
		const result = await embedder.embed('hello');
		expect(client.embed).toHaveBeenCalledWith('hello', undefined, undefined);
		expect(result.embeddings).toEqual([[0.1, 0.2, 0.3]]);
	});

	it('delegates string array to client.embed', async () => {
		const client = createMockACPClient();
		const embedder = createACPEmbedder({ client });
		const input = ['hello', 'world'] as const;
		await embedder.embed(input);
		expect(client.embed).toHaveBeenCalledWith(
			['hello', 'world'],
			undefined,
			undefined,
		);
	});

	it('passes model and serverName options', async () => {
		const client = createMockACPClient();
		const embedder = createACPEmbedder({
			client,
			model: 'embed-model',
			serverName: 'embed-server',
		});
		await embedder.embed('test');
		expect(client.embed).toHaveBeenCalledWith(
			'test',
			'embed-model',
			'embed-server',
		);
	});
});

describe('createACPGenerator', () => {
	it('returns a frozen TextGenerationProvider', () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client });
		expect(Object.isFrozen(generator)).toBe(true);
		expect(typeof generator.generate).toBe('function');
	});

	it('delegates to client.generate', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client });
		const result = await generator.generate('tell me a joke');
		expect(result).toBe('generated text');
		expect(client.generate).toHaveBeenCalledWith('tell me a joke', {
			agentId: undefined,
			serverName: undefined,
			systemPrompt: undefined,
		});
	});

	it('passes agentId and serverName', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({
			client,
			agentId: 'my-agent',
			serverName: 'my-server',
		});
		await generator.generate('prompt');
		expect(client.generate).toHaveBeenCalledWith('prompt', {
			agentId: 'my-agent',
			serverName: 'my-server',
			systemPrompt: undefined,
		});
	});

	it('passes systemPrompt when provided', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client });
		await generator.generate('prompt', 'you are helpful');
		expect(client.generate).toHaveBeenCalledWith('prompt', {
			agentId: undefined,
			serverName: undefined,
			systemPrompt: 'you are helpful',
		});
	});

	it('combines systemPromptPrefix with systemPrompt', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({
			client,
			systemPromptPrefix: 'prefix',
		});
		await generator.generate('prompt', 'suffix');
		expect(client.generate).toHaveBeenCalledWith('prompt', {
			agentId: undefined,
			serverName: undefined,
			systemPrompt: 'prefix\n\nsuffix',
		});
	});

	it('uses only prefix when no systemPrompt given', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({
			client,
			systemPromptPrefix: 'prefix only',
		});
		await generator.generate('prompt');
		expect(client.generate).toHaveBeenCalledWith('prompt', {
			agentId: undefined,
			serverName: undefined,
			systemPrompt: 'prefix only',
		});
	});
});

describe('createACPGenerator generateWithModel', () => {
	it('returns a provider with generateWithModel', () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client });
		expect(typeof generator.generateWithModel).toBe('function');
	});

	it('delegates to client.generate with modelId option', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client });
		const result = await generator.generateWithModel!(
			'optimize this',
			'claude-opus-4-6',
		);
		expect(result).toBe('generated text');
		expect(client.generate).toHaveBeenCalledWith('optimize this', {
			agentId: undefined,
			serverName: undefined,
			systemPrompt: undefined,
			modelId: 'claude-opus-4-6',
		});
	});

	it('passes systemPrompt through generateWithModel', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({
			client,
			systemPromptPrefix: 'prefix',
		});
		await generator.generateWithModel!('prompt', 'model-id', 'system');
		expect(client.generate).toHaveBeenCalledWith('prompt', {
			agentId: undefined,
			serverName: undefined,
			systemPrompt: 'prefix\n\nsystem',
			modelId: 'model-id',
		});
	});
});
