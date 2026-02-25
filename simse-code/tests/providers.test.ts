import { describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../../src/ai/acp/acp-client.js';
import type {
	ACPEmbedResult,
	ACPGenerateResult,
} from '../../src/ai/acp/types.js';
import { createACPEmbedder, createACPGenerator } from '../providers.js';

// ---------------------------------------------------------------------------
// Mock ACP client
// ---------------------------------------------------------------------------

function createMockACPClient(overrides: Partial<ACPClient> = {}): ACPClient {
	const noop = () => {};
	return {
		initialize: mock(noop),
		dispose: mock(noop),
		generate: mock(noop).mockResolvedValue({
			content: 'generated text',
			agentId: 'default',
			serverName: 'local',
			sessionId: 'session-1',
		} satisfies ACPGenerateResult),
		chat: mock(noop),
		generateStream: mock(noop),
		embed: mock(noop).mockResolvedValue({
			embeddings: [
				[0.1, 0.2, 0.3],
				[0.4, 0.5, 0.6],
			],
			agentId: 'default',
			serverName: 'local',
		} satisfies ACPEmbedResult),
		isAvailable: mock(noop).mockResolvedValue(true),
		listAgents: mock(noop).mockResolvedValue([]),
		getAgent: mock(noop),
		serverNames: ['local'],
		serverCount: 1,
		defaultServerName: 'local',
		defaultAgent: 'default',
		setPermissionPolicy: mock(noop),
		...overrides,
	} as unknown as ACPClient;
}

// ---------------------------------------------------------------------------
// createACPEmbedder
// ---------------------------------------------------------------------------

describe('createACPEmbedder', () => {
	it('should return a frozen EmbeddingProvider', () => {
		const client = createMockACPClient();
		const embedder = createACPEmbedder({ client });
		expect(Object.isFrozen(embedder)).toBe(true);
	});

	it('should call client.embed with the input', async () => {
		const client = createMockACPClient();
		const embedder = createACPEmbedder({ client });

		const result = await embedder.embed(['hello', 'world']);
		expect(client.embed).toHaveBeenCalledWith(
			['hello', 'world'],
			undefined,
			undefined,
		);
		expect(result.embeddings).toHaveLength(2);
	});

	it('should pass model to client.embed', async () => {
		const client = createMockACPClient();
		const embedder = createACPEmbedder({ client, model: 'custom-embed' });

		await embedder.embed('single text');
		expect(client.embed).toHaveBeenCalledWith(
			'single text',
			'custom-embed',
			undefined,
		);
	});

	it('should pass serverName to client.embed', async () => {
		const client = createMockACPClient();
		const embedder = createACPEmbedder({
			client,
			serverName: 'embed-server',
		});

		await embedder.embed('text');
		expect(client.embed).toHaveBeenCalledWith(
			'text',
			undefined,
			'embed-server',
		);
	});

	it('should convert readonly arrays to mutable for client', async () => {
		const client = createMockACPClient();
		const embedder = createACPEmbedder({ client });

		const input: readonly string[] = ['a', 'b'];
		await embedder.embed(input);
		expect(client.embed).toHaveBeenCalled();
	});
});

// ---------------------------------------------------------------------------
// createACPGenerator
// ---------------------------------------------------------------------------

describe('createACPGenerator', () => {
	it('should return a frozen TextGenerationProvider', () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client });
		expect(Object.isFrozen(generator)).toBe(true);
	});

	it('should call client.generate and return content string', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client });

		const result = await generator.generate('hello');
		expect(result).toBe('generated text');
		expect(client.generate).toHaveBeenCalledWith('hello', {
			agentId: undefined,
			systemPrompt: undefined,
		});
	});

	it('should pass agentId and systemPrompt', async () => {
		const client = createMockACPClient();
		const generator = createACPGenerator({ client, agentId: 'my-agent' });

		await generator.generate('prompt', 'system instructions');
		expect(client.generate).toHaveBeenCalledWith('prompt', {
			agentId: 'my-agent',
			systemPrompt: 'system instructions',
		});
	});
});
