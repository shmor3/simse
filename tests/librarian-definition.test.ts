import { describe, expect, it } from 'bun:test';
import type {
	ArbitrationResult,
	LibrarianBid,
	LibrarianDefinition,
} from '../src/ai/library/types.js';

describe('LibrarianDefinition types', () => {
	it('allows constructing a valid LibrarianDefinition', () => {
		const def: LibrarianDefinition = {
			name: 'code-patterns',
			description: 'Manages code pattern memories',
			purpose: 'I specialize in code patterns and architecture',
			topics: ['code/*', 'architecture/*'],
			permissions: { add: true, delete: true, reorganize: true },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
		};
		expect(def.name).toBe('code-patterns');
		expect(def.topics).toHaveLength(2);
	});

	it('allows LibrarianDefinition with ACP config', () => {
		const def: LibrarianDefinition = {
			name: 'test',
			description: 'Test librarian',
			purpose: 'Testing',
			topics: ['*'],
			permissions: { add: true, delete: false, reorganize: false },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
			acp: {
				command: 'simse-engine',
				args: ['--mode', 'librarian'],
				agentId: 'test-agent',
			},
		};
		expect(def.acp?.command).toBe('simse-engine');
	});

	it('allows constructing a LibrarianBid', () => {
		const bid: LibrarianBid = {
			librarianName: 'code-patterns',
			argument: 'I already manage 15 volumes about React patterns',
			confidence: 0.85,
		};
		expect(bid.confidence).toBeGreaterThan(0);
	});

	it('allows constructing an ArbitrationResult', () => {
		const result: ArbitrationResult = {
			winner: 'code-patterns',
			reason: 'Best expertise match',
			bids: [
				{
					librarianName: 'code-patterns',
					argument: 'I manage code patterns',
					confidence: 0.9,
				},
			],
		};
		expect(result.bids).toHaveLength(1);
	});
});
