// tests/librarian.test.ts
import { describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';
import {
	createDefaultLibrarian,
	createLibrarian,
} from '../src/ai/library/librarian.js';
import type {
	LibrarianLibraryAccess,
	TextGenerationProvider,
	Volume,
} from '../src/ai/library/types.js';

function createMockGenerator(response: string): TextGenerationProvider {
	return {
		generate: mock(async () => response),
	};
}

describe('Librarian', () => {
	it('extract() parses LLM JSON into ExtractionResult', async () => {
		const generator = createMockGenerator(
			JSON.stringify({
				memories: [
					{
						text: 'Users table uses UUID primary keys',
						topic: 'architecture/database/schema',
						tags: ['postgresql', 'uuid', 'schema'],
						entryType: 'fact',
					},
				],
			}),
		);
		const librarian = createLibrarian(generator);
		const result = await librarian.extract({
			userInput: 'What PK type should we use?',
			response: 'We decided to use UUID primary keys for the users table.',
		});
		expect(result.memories.length).toBe(1);
		expect(result.memories[0].topic).toBe('architecture/database/schema');
		expect(result.memories[0].entryType).toBe('fact');
	});

	it('extract() returns empty memories on LLM garbage', async () => {
		const generator = createMockGenerator('not valid json');
		const librarian = createLibrarian(generator);
		const result = await librarian.extract({
			userInput: 'hello',
			response: 'hi',
		});
		expect(result.memories).toEqual([]);
	});

	it('summarize() returns a CompendiumResult', async () => {
		const generator = createMockGenerator(
			'PostgreSQL uses UUID PKs across all tables for consistency.',
		);
		const librarian = createLibrarian(generator);
		const volumes: Volume[] = [
			{
				id: 'v1',
				text: 'Users table has UUID PK',
				embedding: [0.1],
				metadata: {},
				timestamp: 1,
			},
			{
				id: 'v2',
				text: 'Orders table has UUID PK',
				embedding: [0.2],
				metadata: {},
				timestamp: 2,
			},
		];
		const result = await librarian.summarize(volumes, 'architecture/database');
		expect(result.text.length).toBeGreaterThan(0);
		expect(result.sourceIds).toEqual(['v1', 'v2']);
	});

	it('classifyTopic() returns classification result', async () => {
		const generator = createMockGenerator(
			JSON.stringify({
				topic: 'architecture/database/schema',
				confidence: 0.9,
			}),
		);
		const librarian = createLibrarian(generator);
		const result = await librarian.classifyTopic('Users table uses UUID PKs', [
			'architecture/database',
			'bugs/open',
		]);
		expect(result.topic).toBe('architecture/database/schema');
	});

	it('reorganize() returns a plan', async () => {
		const generator = createMockGenerator(
			JSON.stringify({
				moves: [
					{
						volumeId: 'v1',
						newTopic: 'architecture/database/optimization',
					},
				],
				newSubtopics: ['architecture/database/optimization'],
				merges: [],
			}),
		);
		const librarian = createLibrarian(generator);
		const volumes: Volume[] = [
			{
				id: 'v1',
				text: 'Index optimization',
				embedding: [0.1],
				metadata: {},
				timestamp: 1,
			},
		];
		const result = await librarian.reorganize('architecture/database', volumes);
		expect(result.moves.length).toBe(1);
		expect(result.moves[0].newTopic).toBe('architecture/database/optimization');
	});
});

function createMockGeneratorWithModel(
	defaultResponse: string,
	modelResponse: string,
): TextGenerationProvider {
	return {
		generate: mock(async () => defaultResponse),
		generateWithModel: mock(async () => modelResponse),
	};
}

describe('Librarian optimize', () => {
	it('optimize() uses generateWithModel when available', async () => {
		const optimizationResponse = JSON.stringify({
			pruned: ['v2'],
			summary: 'Condensed summary of database architecture.',
			reorganization: {
				moves: [],
				newSubtopics: [],
				merges: [],
			},
		});
		const generator = createMockGeneratorWithModel(
			'unused',
			optimizationResponse,
		);
		const librarian = createLibrarian(generator);
		const volumes: Volume[] = [
			{
				id: 'v1',
				text: 'Users table uses UUID PKs',
				embedding: [0.1],
				metadata: {},
				timestamp: 1,
			},
			{
				id: 'v2',
				text: 'Users table has UUID primary keys',
				embedding: [0.2],
				metadata: {},
				timestamp: 2,
			},
		];
		const result = await librarian.optimize(
			volumes,
			'architecture/database',
			'claude-opus-4-6',
		);
		expect(result.pruned).toEqual(['v2']);
		expect(result.summary.length).toBeGreaterThan(0);
		expect(result.modelUsed).toBe('claude-opus-4-6');
		expect(generator.generateWithModel).toHaveBeenCalled();
		expect(generator.generate).not.toHaveBeenCalled();
	});

	it('optimize() falls back to generate when generateWithModel is absent', async () => {
		const optimizationResponse = JSON.stringify({
			pruned: [],
			summary: 'Summary using default model.',
			reorganization: { moves: [], newSubtopics: [], merges: [] },
		});
		const generator = createMockGenerator(optimizationResponse);
		const librarian = createLibrarian(generator);
		const volumes: Volume[] = [
			{
				id: 'v1',
				text: 'Some fact',
				embedding: [0.1],
				metadata: {},
				timestamp: 1,
			},
		];
		const result = await librarian.optimize(volumes, 'test', 'any-model');
		expect(result.pruned).toEqual([]);
		expect(result.modelUsed).toBe('any-model');
		expect(generator.generate).toHaveBeenCalled();
	});

	it('optimize() returns safe defaults on LLM garbage', async () => {
		const generator = createMockGeneratorWithModel(
			'unused',
			'not valid json at all',
		);
		const librarian = createLibrarian(generator);
		const result = await librarian.optimize(
			[
				{
					id: 'v1',
					text: 'fact',
					embedding: [0.1],
					metadata: {},
					timestamp: 1,
				},
			],
			'test',
			'model-id',
		);
		expect(result.pruned).toEqual([]);
		expect(result.summary).toBe('');
		expect(result.reorganization.moves).toEqual([]);
		expect(result.modelUsed).toBe('model-id');
	});
});

// ---------------------------------------------------------------------------
// createDefaultLibrarian
// ---------------------------------------------------------------------------

function createMockACPClientForLibrarian(): ACPClient {
	return {
		initialize: mock(() => Promise.resolve()),
		dispose: mock(() => Promise.resolve()),
		listAgents: mock(() => Promise.resolve([])),
		getAgent: mock(() => Promise.resolve({ id: 'test', name: 'test' })),
		generate: mock(() =>
			Promise.resolve({
				content: JSON.stringify({ memories: [] }),
				agentId: 'agent-1',
				serverName: 'server-1',
				sessionId: 'sess-1',
			}),
		),
		chat: mock(() =>
			Promise.resolve({
				content: 'chat',
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
				embeddings: [[0.1]],
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
	} as ACPClient;
}

describe('createDefaultLibrarian', () => {
	it('creates a Librarian from an ACP client', async () => {
		const client = createMockACPClientForLibrarian();
		const librarian = createDefaultLibrarian(client);
		expect(typeof librarian.extract).toBe('function');
		expect(typeof librarian.optimize).toBe('function');
		const result = await librarian.extract({
			userInput: 'hello',
			response: 'world',
		});
		expect(result.memories).toEqual([]);
		expect(client.generate).toHaveBeenCalled();
	});
});

// ---------------------------------------------------------------------------
// bid
// ---------------------------------------------------------------------------

function createMockLibrary(
	overrides?: Partial<LibrarianLibraryAccess>,
): LibrarianLibraryAccess {
	return {
		search: mock(async () => []),
		getTopics: mock(() => []),
		filterByTopic: mock(() => []),
		...overrides,
	};
}

describe('bid', () => {
	it('produces a bid with argument and confidence', async () => {
		const generator = createMockGenerator(
			JSON.stringify({
				argument: 'This content aligns with my database expertise.',
				confidence: 0.85,
			}),
		);
		const librarian = createLibrarian(generator, {
			name: 'db-librarian',
			purpose: 'Manages database-related knowledge.',
		});
		const library = createMockLibrary();
		const result = await librarian.bid(
			'Users table uses UUID primary keys',
			'architecture/database',
			library,
		);
		expect(result.librarianName).toBe('db-librarian');
		expect(result.argument).toBe(
			'This content aligns with my database expertise.',
		);
		expect(result.confidence).toBe(0.85);
	});

	it('returns zero confidence on parse failure', async () => {
		const generator = createMockGenerator('not valid json at all');
		const librarian = createLibrarian(generator, {
			name: 'broken-librarian',
			purpose: 'Will fail to parse.',
		});
		const library = createMockLibrary();
		const result = await librarian.bid('some content', 'any/topic', library);
		expect(result.librarianName).toBe('broken-librarian');
		expect(result.argument).toBe('');
		expect(result.confidence).toBe(0);
	});

	it('uses default identity when none provided', async () => {
		const generator = createMockGenerator(
			JSON.stringify({
				argument: 'I can handle this.',
				confidence: 0.5,
			}),
		);
		const librarian = createLibrarian(generator);
		const library = createMockLibrary();
		const result = await librarian.bid(
			'some content',
			'general/topic',
			library,
		);
		expect(result.librarianName).toBe('default');
		expect(result.confidence).toBe(0.5);
	});

	it('clamps confidence to 0-1 range', async () => {
		const generator = createMockGenerator(
			JSON.stringify({
				argument: 'Very confident.',
				confidence: 2.5,
			}),
		);
		const librarian = createLibrarian(generator, {
			name: 'over-confident',
			purpose: 'Returns inflated confidence.',
		});
		const library = createMockLibrary();
		const result = await librarian.bid('some content', 'any/topic', library);
		expect(result.confidence).toBe(1);
	});
});
