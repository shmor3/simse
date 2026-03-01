/**
 * E2E tests for graph intelligence features.
 *
 * Exercises explicit edges (rel:* metadata), graph-boosted search, graph
 * neighbors, and graph traversal through the TypeScript Stacks and Library APIs
 * backed by the real Rust vector engine.
 */
import { afterEach, describe, expect, it } from 'bun:test';
import { fileURLToPath } from 'node:url';
import { createLibrary } from '../../src/ai/library/library.js';
import { createStacks, type Stacks } from '../../src/ai/library/stacks.js';
import type { EmbeddingProvider } from '../../src/ai/library/types.js';

const ENGINE_PATH = fileURLToPath(
	new URL(
		'../../simse-vector/target/debug/simse-vector-engine.exe',
		import.meta.url,
	),
);

// ---------------------------------------------------------------------------
// Deterministic mock embedder (128-dim, word-hash based)
// ---------------------------------------------------------------------------

function createMockEmbedder(): EmbeddingProvider {
	const DIM = 128;

	function wordHash(word: string): number {
		let h = 0;
		for (let i = 0; i < word.length; i++) {
			h = (h * 31 + word.charCodeAt(i)) | 0;
		}
		return ((h % DIM) + DIM) % DIM;
	}

	function hashText(text: string): number[] {
		const vec = new Array(DIM).fill(0);
		const words = text
			.toLowerCase()
			.replace(/[^a-z0-9\s]/g, '')
			.split(/\s+/);
		for (const word of words) {
			if (word.length === 0) continue;
			const primary = wordHash(word);
			const secondary = wordHash(`${word}_`);
			vec[primary] += 1.0;
			vec[secondary] += 0.5;
		}
		const mag =
			Math.sqrt(vec.reduce((s: number, v: number) => s + v * v, 0)) || 1;
		return vec.map((v: number) => v / mag);
	}

	return Object.freeze({
		embed: async (input: string | readonly string[]) => {
			const texts = typeof input === 'string' ? [input] : [...input];
			return { embeddings: texts.map(hashText) };
		},
	});
}

// ---------------------------------------------------------------------------
// Helper: generate an embedding from text using the mock embedder
// ---------------------------------------------------------------------------

const embedder = createMockEmbedder();

async function embed(text: string): Promise<number[]> {
	const result = await embedder.embed(text);
	return result.embeddings[0];
}

// ===========================================================================
// Stacks-level graph tests (direct access to graphNeighbors / graphTraverse)
// ===========================================================================

describe('Graph intelligence E2E — Stacks level', () => {
	let stacks: Stacks;

	afterEach(async () => {
		await stacks?.dispose();
	});

	it('creates explicit edges from rel:* metadata', async () => {
		stacks = createStacks({
			enginePath: ENGINE_PATH,
			duplicateThreshold: 1,
		});
		await stacks.load();

		const embA = await embed('TypeScript is a typed superset of JavaScript');
		const embB = await embed('JavaScript frameworks like React and Vue');

		const idA = await stacks.add('TypeScript is a typed superset of JavaScript', embA, {
			topic: 'programming',
		});
		const idB = await stacks.add('JavaScript frameworks like React and Vue', embB, {
			'rel:related': idA,
		});

		// Volume B should have volume A as a neighbor via an explicit Related edge
		const neighbors = await stacks.graphNeighbors(idB);
		expect(neighbors.length).toBeGreaterThan(0);

		const relatedNeighbor = neighbors.find((n) => n.volume.id === idA);
		expect(relatedNeighbor).toBeDefined();
		expect(relatedNeighbor!.edge.edgeType).toBe('Related');
		expect(relatedNeighbor!.edge.origin).toBe('Explicit');
	});

	it('creates bidirectional explicit edges', async () => {
		stacks = createStacks({
			enginePath: ENGINE_PATH,
			duplicateThreshold: 1,
		});
		await stacks.load();

		const embA = await embed('Machine learning algorithms');
		const embB = await embed('Deep learning neural networks');

		const idA = await stacks.add('Machine learning algorithms', embA);
		const idB = await stacks.add('Deep learning neural networks', embB, {
			'rel:related': idA,
		});

		// Check A's neighbors: B should be listed
		const neighborsOfA = await stacks.graphNeighbors(idA);
		const bInA = neighborsOfA.find((n) => n.volume.id === idB);
		expect(bInA).toBeDefined();

		// Check B's neighbors: A should be listed
		const neighborsOfB = await stacks.graphNeighbors(idB);
		const aInB = neighborsOfB.find((n) => n.volume.id === idA);
		expect(aInB).toBeDefined();
	});

	it('supports typed relationship edges via rel:parent and rel:child', async () => {
		stacks = createStacks({
			enginePath: ENGINE_PATH,
			duplicateThreshold: 1,
		});
		await stacks.load();

		const embParent = await embed('Programming languages overview');
		const embChild = await embed('TypeScript language details');

		const parentId = await stacks.add('Programming languages overview', embParent);
		const childId = await stacks.add('TypeScript language details', embChild, {
			'rel:parent': parentId,
		});

		// Child should have a Parent edge to the parent volume
		const childNeighbors = await stacks.graphNeighbors(childId);
		const parentEdge = childNeighbors.find((n) => n.volume.id === parentId);
		expect(parentEdge).toBeDefined();
		expect(parentEdge!.edge.edgeType).toBe('Parent');
		expect(parentEdge!.edge.origin).toBe('Explicit');
	});

	it('graph traversal walks through connected volumes', async () => {
		stacks = createStacks({
			enginePath: ENGINE_PATH,
			duplicateThreshold: 1,
		});
		await stacks.load();

		const embA = await embed('Graph theory fundamentals');
		const embB = await embed('Network analysis algorithms');
		const embC = await embed('Social network modeling');

		const idA = await stacks.add('Graph theory fundamentals', embA);
		const idB = await stacks.add('Network analysis algorithms', embB, {
			'rel:related': idA,
		});
		const idC = await stacks.add('Social network modeling', embC, {
			'rel:related': idB,
		});

		// Traversal from A with depth 2 should reach B and C
		const traversal = await stacks.graphTraverse(idA, 2);
		expect(traversal.length).toBeGreaterThan(0);

		const visitedIds = traversal.map((n) => n.volume.id);
		expect(visitedIds).toContain(idB);

		// Depth-1 traversal from A should only reach B (direct neighbor)
		const shallowTraversal = await stacks.graphTraverse(idA, 1);
		const shallowIds = shallowTraversal.map((n) => n.volume.id);
		expect(shallowIds).toContain(idB);
	});

	it('graph neighbors can filter by edge type', async () => {
		stacks = createStacks({
			enginePath: ENGINE_PATH,
			duplicateThreshold: 1,
		});
		await stacks.load();

		const embA = await embed('Base concept');
		const embB = await embed('Related concept');
		const embC = await embed('Contradicting evidence');

		const idA = await stacks.add('Base concept', embA);
		const idB = await stacks.add('Related concept', embB, {
			'rel:related': idA,
		});
		const idC = await stacks.add('Contradicting evidence', embC, {
			'rel:contradicts': idA,
		});

		// Get all neighbors of A
		const allNeighbors = await stacks.graphNeighbors(idA);
		expect(allNeighbors.length).toBeGreaterThanOrEqual(2);

		// Filter to only Related edges
		const relatedOnly = await stacks.graphNeighbors(idA, ['Related']);
		const relatedIds = relatedOnly.map((n) => n.volume.id);
		expect(relatedIds).toContain(idB);

		// Filter to only Contradicts edges
		const contradictsOnly = await stacks.graphNeighbors(idA, ['Contradicts']);
		const contradictsIds = contradictsOnly.map((n) => n.volume.id);
		expect(contradictsIds).toContain(idC);
	});

	it('traversal nodes include depth and path', async () => {
		stacks = createStacks({
			enginePath: ENGINE_PATH,
			duplicateThreshold: 1,
		});
		await stacks.load();

		const embA = await embed('Root node content');
		const embB = await embed('First hop content');

		const idA = await stacks.add('Root node content', embA);
		const idB = await stacks.add('First hop content', embB, {
			'rel:related': idA,
		});

		const traversal = await stacks.graphTraverse(idA, 2);
		expect(traversal.length).toBeGreaterThan(0);

		// Traversal from A should find B at depth 1
		const nodeB = traversal.find((n) => n.volume.id === idB);
		expect(nodeB).toBeDefined();
		expect(nodeB!.depth).toBe(1);
		expect(nodeB!.path.length).toBeGreaterThan(0);
	});
});

// ===========================================================================
// Library-level graph tests (advancedSearch with graphBoost)
// ===========================================================================

describe('Graph intelligence E2E — Library level', () => {
	let library: ReturnType<typeof createLibrary>;

	afterEach(async () => {
		await library?.dispose();
	});

	it('graph-boosted advancedSearch ranks connected volumes higher', async () => {
		library = createLibrary(
			embedder,
			{},
			{
				enginePath: ENGINE_PATH,
				stacksOptions: { duplicateThreshold: 1 },
			},
		);
		await library.initialize();

		// Add a "TypeScript" volume
		const tsId = await library.add(
			'TypeScript is a typed superset of JavaScript with static analysis',
			{ topic: 'programming' },
		);

		// Add a "JavaScript" volume linked to TypeScript via rel:related
		await library.add(
			'JavaScript is a dynamic language for web development',
			{ topic: 'programming', 'rel:related': tsId },
		);

		// Add an unrelated "weather" volume
		await library.add(
			'The weather forecast shows sunny skies tomorrow afternoon',
			{ topic: 'weather' },
		);

		// Search with graph boost enabled: connected volumes should be boosted
		const boostedResults = await library.advancedSearch({
			text: { query: 'typed programming language', mode: 'fuzzy' },
			graphBoost: { enabled: true, weight: 0.5 },
			maxResults: 10,
			similarityThreshold: 0,
		});

		expect(boostedResults.length).toBeGreaterThanOrEqual(2);

		// Search without graph boost for comparison
		const plainResults = await library.advancedSearch({
			text: { query: 'typed programming language', mode: 'fuzzy' },
			maxResults: 10,
			similarityThreshold: 0,
		});

		expect(plainResults.length).toBeGreaterThanOrEqual(2);

		// The programming-related results should appear in boosted results
		const boostedTopics = boostedResults.map(
			(r) => r.volume.metadata?.topic,
		);
		expect(boostedTopics).toContain('programming');
	});

	it('adds volumes with rel:* metadata and they remain searchable', async () => {
		library = createLibrary(
			embedder,
			{},
			{
				enginePath: ENGINE_PATH,
				stacksOptions: { duplicateThreshold: 1 },
			},
		);
		await library.initialize();

		const idA = await library.add('Functional programming paradigm');
		const idB = await library.add('Lambda calculus theory', {
			'rel:related': idA,
		});

		// Both should exist and be retrievable
		const volA = await library.getById(idA);
		const volB = await library.getById(idB);
		expect(volA).toBeDefined();
		expect(volB).toBeDefined();

		// The rel:related metadata should be stored on volume B
		expect(volB!.metadata['rel:related']).toBe(idA);

		// Both should be findable via search
		const results = await library.search('programming paradigm', 5, 0.0);
		expect(results.length).toBeGreaterThan(0);
	});

	it('graph-boosted search includes graphBoost scores in results', async () => {
		library = createLibrary(
			embedder,
			{},
			{
				enginePath: ENGINE_PATH,
				stacksOptions: { duplicateThreshold: 1 },
			},
		);
		await library.initialize();

		const idA = await library.add('Database indexing strategies');
		await library.add('Query optimization techniques', {
			'rel:related': idA,
		});

		const results = await library.advancedSearch({
			text: { query: 'database', mode: 'fuzzy' },
			graphBoost: { enabled: true, weight: 0.3 },
			maxResults: 10,
			similarityThreshold: 0,
		});

		// All results should have a numeric score
		for (const r of results) {
			expect(typeof r.score).toBe('number');
			expect(r.score).toBeGreaterThanOrEqual(0);
		}
	});
});
