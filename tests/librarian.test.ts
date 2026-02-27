// tests/librarian.test.ts
import { describe, expect, it, mock } from 'bun:test';
import { createLibrarian } from '../src/ai/library/librarian.js';
import type { TextGenerationProvider, Volume } from '../src/ai/library/types.js';

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
			response:
				'We decided to use UUID primary keys for the users table.',
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
		const result = await librarian.classifyTopic(
			'Users table uses UUID PKs',
			['architecture/database', 'bugs/open'],
		);
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
		const result = await librarian.reorganize(
			'architecture/database',
			volumes,
		);
		expect(result.moves.length).toBe(1);
		expect(result.moves[0].newTopic).toBe(
			'architecture/database/optimization',
		);
	});
});
