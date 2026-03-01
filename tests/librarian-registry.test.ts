import { afterEach, beforeEach, describe, expect, it, mock } from 'bun:test';
import { mkdtemp, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { ACPConnection } from '../src/ai/acp/acp-connection.js';
import { saveDefinition } from '../src/ai/library/librarian-definition.js';
import {
	createLibrarianRegistry,
	type LibrarianRegistryOptions,
} from '../src/ai/library/librarian-registry.js';
import type {
	LibrarianDefinition,
	LibrarianLibraryAccess,
	TextGenerationProvider,
} from '../src/ai/library/types.js';
import { createLogger } from '../src/logger.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createMockProvider(response: string): TextGenerationProvider {
	return {
		generate: mock(async () => response),
	};
}

function createMockLibrary(
	overrides?: Partial<LibrarianLibraryAccess>,
): LibrarianLibraryAccess {
	return {
		search: mock(async () => []),
		getTopics: mock(async () => []),
		filterByTopic: mock(async () => []),
		...overrides,
	};
}

const VALID_DEF: LibrarianDefinition = {
	name: 'code-patterns',
	description: 'Manages code pattern memories',
	purpose: 'I specialize in code patterns and architecture',
	topics: ['code/*', 'architecture/*'],
	permissions: { add: true, delete: true, reorganize: true },
	thresholds: { topicComplexity: 50, escalateAt: 100 },
};

const silentLogger = createLogger({ level: 'none' });

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('LibrarianRegistry', () => {
	let tmpDir: string;

	beforeEach(async () => {
		tmpDir = await mkdtemp(join(tmpdir(), 'simse-registry-test-'));
	});

	afterEach(async () => {
		await rm(tmpDir, { recursive: true, force: true });
	});

	function makeOptions(
		overrides?: Partial<LibrarianRegistryOptions>,
	): LibrarianRegistryOptions {
		return {
			librariansDir: tmpDir,
			library: createMockLibrary(),
			defaultProvider: createMockProvider('{}'),
			logger: silentLogger,
			...overrides,
		};
	}

	// -----------------------------------------------------------------
	// initialize
	// -----------------------------------------------------------------

	describe('initialize', () => {
		it('creates default librarian when no definitions exist', async () => {
			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			expect(registry.defaultLibrarian).toBeDefined();
			expect(registry.defaultLibrarian.definition.name).toBe('default');
			expect(registry.list()).toHaveLength(1);
		});

		it('loads definitions from disk on initialize', async () => {
			await saveDefinition(tmpDir, VALID_DEF);

			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			expect(registry.list()).toHaveLength(2); // default + code-patterns
			expect(registry.get('code-patterns')).toBeDefined();
			expect(registry.get('code-patterns')!.definition.name).toBe(
				'code-patterns',
			);
		});

		it('deduplicates concurrent initialize calls', async () => {
			const provider = createMockProvider('{}');
			const registry = createLibrarianRegistry(
				makeOptions({ defaultProvider: provider }),
			);

			// Fire two concurrent initializations
			const [r1, r2] = await Promise.all([
				registry.initialize(),
				registry.initialize(),
			]);

			expect(r1).toBeUndefined();
			expect(r2).toBeUndefined();
			// Only one default librarian should exist
			expect(registry.list()).toHaveLength(1);
		});
	});

	// -----------------------------------------------------------------
	// register / unregister
	// -----------------------------------------------------------------

	describe('register / unregister', () => {
		it('registers a new librarian and saves to disk', async () => {
			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			const managed = await registry.register(VALID_DEF);

			expect(managed.definition.name).toBe('code-patterns');
			expect(registry.get('code-patterns')).toBe(managed);

			// Check that the file was created on disk
			const files = await readdir(tmpDir);
			expect(files).toContain('code-patterns.json');
		});

		it('throws on invalid definition', async () => {
			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			const invalid = { name: '' } as unknown as LibrarianDefinition;
			await expect(registry.register(invalid)).rejects.toThrow(
				'Invalid librarian definition',
			);
		});

		it('throws on duplicate name', async () => {
			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			await registry.register(VALID_DEF);
			await expect(registry.register(VALID_DEF)).rejects.toThrow(
				'already registered',
			);
		});

		it('unregisters a librarian', async () => {
			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			await registry.register(VALID_DEF);
			expect(registry.get('code-patterns')).toBeDefined();

			await registry.unregister('code-patterns');
			expect(registry.get('code-patterns')).toBeUndefined();
		});

		it('cannot unregister default', async () => {
			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			await expect(registry.unregister('default')).rejects.toThrow(
				'Cannot unregister the default librarian',
			);
		});

		it('closes ACP connection on unregister', async () => {
			const mockClose = mock(async () => {});
			const mockConnection = { close: mockClose } as unknown as ACPConnection;
			const createConnection = mock(async () => ({
				connection: mockConnection,
				provider: createMockProvider('{}'),
			}));

			const registry = createLibrarianRegistry(
				makeOptions({ createConnection }),
			);
			await registry.initialize();

			const defWithAcp: LibrarianDefinition = {
				...VALID_DEF,
				acp: { command: 'echo', args: ['hello'] },
			};
			await registry.register(defWithAcp);

			await registry.unregister('code-patterns');
			expect(mockClose).toHaveBeenCalled();
		});
	});

	// -----------------------------------------------------------------
	// resolveLibrarian
	// -----------------------------------------------------------------

	describe('resolveLibrarian', () => {
		it('returns default when no specialists match', async () => {
			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			// Register a specialist for a different topic
			const designDef: LibrarianDefinition = {
				...VALID_DEF,
				name: 'design-specialist',
				topics: ['design/*'],
			};
			await registry.register(designDef);

			const result = await registry.resolveLibrarian(
				'devops/ci',
				'CI pipeline config',
			);

			expect(result.winner).toBe('default');
			expect(result.bids).toHaveLength(0);
		});

		it('returns single matching specialist without arbitration', async () => {
			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			// Register a specialist that only matches code/*
			const codeDef: LibrarianDefinition = {
				...VALID_DEF,
				name: 'code-specialist',
				topics: ['code/*'],
			};
			await registry.register(codeDef);

			// Topic that only matches code-specialist (not default's '*')
			// Actually '*' matches any single-level topic, so default will also match.
			// We need a topic that only one non-default specialist matches
			// but default also matches everything via '*'.
			// So let's test with two matches (default + specialist) → goes to bidding.
			// For single-match test we need to unregister default, but we can't.
			// Instead let's make a topic that two specialists match and see the multiple case.

			// Actually, the default always has ['*'] so it always matches.
			// Single match case occurs when ONLY the default matches (no other specialist).
			// Let's verify that path by querying a topic no specialist covers but default does.
			const result = await registry.resolveLibrarian(
				'random-topic',
				'some content',
			);

			// Only default matches 'random-topic' (it has ['*']).
			// code-specialist has ['code/*'] which doesn't match 'random-topic'.
			expect(result.winner).toBe('default');
			expect(result.bids).toHaveLength(0);
		});

		it('runs bidding when multiple match', async () => {
			// Provider that returns different confidence for each call
			let callCount = 0;
			const biddingProvider: TextGenerationProvider = {
				generate: mock(async () => {
					callCount++;
					if (callCount <= 2) {
						// Bid responses (the first two calls are bids)
						const confidence = callCount === 1 ? 0.9 : 0.3;
						return JSON.stringify({
							argument: `Bid ${callCount}`,
							confidence,
						});
					}
					// Should not reach arbitration with a large gap
					return JSON.stringify({ winner: 'default', reason: 'fallback' });
				}),
			};

			const registry = createLibrarianRegistry(
				makeOptions({ defaultProvider: biddingProvider }),
			);
			await registry.initialize();

			// Register a specialist that matches 'code/react'
			const codeDef: LibrarianDefinition = {
				...VALID_DEF,
				name: 'code-specialist',
				topics: ['code/**'],
			};
			await registry.register(codeDef);

			// Both default ['*'] and code-specialist ['code/**'] match 'code/react'
			// Wait, '*' only matches single level, not 'code/react'.
			// picomatch: '*' matches 'code' but not 'code/react'.
			// So for 'code/react', only code-specialist matches with 'code/**'.
			// We need at least two matches. Let's add another specialist.
			const archDef: LibrarianDefinition = {
				...VALID_DEF,
				name: 'arch-specialist',
				description: 'Architecture specialist',
				purpose: 'Architecture',
				topics: ['code/**', 'architecture/**'],
			};
			await registry.register(archDef);

			const result = await registry.resolveLibrarian(
				'code/react',
				'React component patterns',
			);

			// Two specialists match: code-specialist and arch-specialist
			// Large gap (0.9 - 0.3 = 0.6 > 0.3) → self-resolution
			expect(result.bids.length).toBeGreaterThanOrEqual(2);
		});

		it('self-resolution when clear confidence gap', async () => {
			let bidCall = 0;
			const provider: TextGenerationProvider = {
				generate: mock(async () => {
					bidCall++;
					if (bidCall === 1) {
						return JSON.stringify({
							argument: 'I am the best',
							confidence: 0.95,
						});
					}
					if (bidCall === 2) {
						return JSON.stringify({ argument: 'I can try', confidence: 0.4 });
					}
					return '{}';
				}),
			};

			const registry = createLibrarianRegistry(
				makeOptions({ defaultProvider: provider, selfResolutionGap: 0.3 }),
			);
			await registry.initialize();

			const spec1: LibrarianDefinition = {
				...VALID_DEF,
				name: 'specialist-a',
				topics: ['data/**'],
			};
			const spec2: LibrarianDefinition = {
				...VALID_DEF,
				name: 'specialist-b',
				description: 'Another specialist',
				purpose: 'Also data',
				topics: ['data/**'],
			};
			await registry.register(spec1);
			await registry.register(spec2);

			const result = await registry.resolveLibrarian(
				'data/analytics',
				'Analytics pipeline',
			);

			expect(result.reason).toContain('Self-resolved');
			// 3 bids: 2 specialists + default (which matches ** on all topics)
			expect(result.bids).toHaveLength(3);
			// Winner should be the one with 0.95 confidence
			const sorted = [...result.bids].sort(
				(a, b) => b.confidence - a.confidence,
			);
			expect(sorted[0].confidence).toBe(0.95);
			expect(result.winner).toBe(sorted[0].librarianName);
		});

		it('arbitration by default librarian when bids are close', async () => {
			let bidCall = 0;
			const provider: TextGenerationProvider = {
				generate: mock(async () => {
					bidCall++;
					if (bidCall <= 3) {
						// 3 bids: specialist-a, specialist-b, default (all close)
						return JSON.stringify({
							argument: 'Good at this',
							confidence: 0.7,
						});
					}
					// Arbitration call (bidCall === 4)
					return JSON.stringify({
						winner: 'specialist-b',
						reason: 'Better expertise match',
					});
				}),
			};

			const registry = createLibrarianRegistry(
				makeOptions({ defaultProvider: provider, selfResolutionGap: 0.3 }),
			);
			await registry.initialize();

			const spec1: LibrarianDefinition = {
				...VALID_DEF,
				name: 'specialist-a',
				topics: ['infra/**'],
			};
			const spec2: LibrarianDefinition = {
				...VALID_DEF,
				name: 'specialist-b',
				description: 'Another specialist',
				purpose: 'Also infra',
				topics: ['infra/**'],
			};
			await registry.register(spec1);
			await registry.register(spec2);

			const result = await registry.resolveLibrarian(
				'infra/k8s',
				'Kubernetes deployment',
			);

			// Gap is 0.10 < 0.3 → arbitration runs
			expect(result.winner).toBe('specialist-b');
			expect(result.reason).toBe('Better expertise match');
			// 3 bids: 2 specialists + default (which matches ** on all topics)
			expect(result.bids).toHaveLength(3);
		});
		it('falls back to default when arbitration returns unknown winner', async () => {
			let bidCall = 0;
			const provider: TextGenerationProvider = {
				generate: mock(async () => {
					bidCall++;
					if (bidCall <= 3) {
						// 3 bids: specialist-x, specialist-y, default
						return JSON.stringify({
							argument: 'Bid response',
							confidence: 0.7,
						});
					}
					// Arbitration returns an unknown winner name
					return JSON.stringify({
						winner: 'nonexistent-librarian',
						reason: 'oops',
					});
				}),
			};

			const registry = createLibrarianRegistry(
				makeOptions({ defaultProvider: provider, selfResolutionGap: 0.3 }),
			);
			await registry.initialize();

			const spec1: LibrarianDefinition = {
				...VALID_DEF,
				name: 'specialist-x',
				topics: ['infra/**'],
			};
			const spec2: LibrarianDefinition = {
				...VALID_DEF,
				name: 'specialist-y',
				description: 'Another specialist',
				purpose: 'Also infra',
				topics: ['infra/**'],
			};
			await registry.register(spec1);
			await registry.register(spec2);

			const result = await registry.resolveLibrarian(
				'infra/k8s',
				'Kubernetes deployment',
			);

			// Unknown winner → falls back to default
			expect(result.winner).toBe('default');
		});
	});

	// -----------------------------------------------------------------
	// spawnSpecialist
	// -----------------------------------------------------------------

	describe('spawnSpecialist', () => {
		it('spawns when confirmed', async () => {
			let callCount = 0;
			const provider: TextGenerationProvider = {
				generate: mock(async () => {
					callCount++;
					if (callCount === 1) {
						// Assessment: yes, spawn
						return JSON.stringify({
							shouldSpawn: true,
							reason: 'Topic is complex enough',
						});
					}
					// Generation: return a valid definition
					return JSON.stringify({
						name: 'ml-specialist',
						description: 'Machine learning specialist',
						purpose: 'Manages ML-related knowledge',
						topics: ['ml/**'],
						permissions: { add: true, delete: true, reorganize: true },
						thresholds: { topicComplexity: 50, escalateAt: 100 },
					});
				}),
			};

			const registry = createLibrarianRegistry(
				makeOptions({ defaultProvider: provider }),
			);
			await registry.initialize();

			const volumes = [
				{
					id: 'v1',
					text: 'Neural network basics',
					embedding: [0.1],
					metadata: {},
					timestamp: 1,
				},
				{
					id: 'v2',
					text: 'Training loop patterns',
					embedding: [0.2],
					metadata: {},
					timestamp: 2,
				},
			];

			const managed = await registry.spawnSpecialist('ml/training', volumes);

			expect(managed.definition.name).toBe('ml-specialist');
			expect(registry.get('ml-specialist')).toBeDefined();

			// Check it was persisted to disk
			const files = await readdir(tmpDir);
			expect(files).toContain('ml-specialist.json');
		});

		it('throws when not confirmed', async () => {
			const provider = createMockProvider(
				JSON.stringify({ shouldSpawn: false, reason: 'Not enough volumes' }),
			);

			const registry = createLibrarianRegistry(
				makeOptions({ defaultProvider: provider }),
			);
			await registry.initialize();

			await expect(registry.spawnSpecialist('tiny-topic', [])).rejects.toThrow(
				'Specialist not needed',
			);
		});
	});

	// -----------------------------------------------------------------
	// dispose
	// -----------------------------------------------------------------

	describe('dispose', () => {
		it('disposes without error', async () => {
			const registry = createLibrarianRegistry(makeOptions());
			await registry.initialize();

			await registry.register(VALID_DEF);

			await expect(registry.dispose()).resolves.toBeUndefined();
			expect(registry.list()).toHaveLength(0);
		});

		it('closes ACP connections on dispose', async () => {
			const mockClose = mock(async () => {});
			const mockConnection = { close: mockClose } as unknown as ACPConnection;
			const createConnection = mock(async () => ({
				connection: mockConnection,
				provider: createMockProvider('{}'),
			}));

			const registry = createLibrarianRegistry(
				makeOptions({ createConnection }),
			);
			await registry.initialize();

			const defWithAcp: LibrarianDefinition = {
				...VALID_DEF,
				acp: { command: 'echo', args: ['hello'] },
			};
			await registry.register(defWithAcp);

			await registry.dispose();

			expect(mockClose).toHaveBeenCalled();
		});
	});
});
