/**
 * E2E test: Local embedder with real ONNX model.
 *
 * Downloads and runs nomic-ai/nomic-embed-text-v1.5 (q8) in-process.
 * First run downloads the model (~33MB), subsequent runs use cache.
 */
import { describe, expect, it } from 'bun:test';
import { createLocalEmbedder } from '../src/ai/acp/local-embedder.js';
import { cosineSimilarity } from '../src/ai/memory/cosine.js';

describe('Local embedder E2E', () => {
	// Use a small model for faster CI — all-MiniLM-L6-v2 is ~23MB
	const embedder = createLocalEmbedder({
		model: 'Xenova/all-MiniLM-L6-v2',
		dtype: 'q8',
	});

	it('embeds a single string and returns a vector', async () => {
		const result = await embedder.embed('Hello world');

		expect(result.embeddings).toHaveLength(1);
		expect(result.embeddings[0].length).toBeGreaterThan(100);
		// Should be normalized (magnitude ~1)
		const mag = Math.sqrt(
			result.embeddings[0].reduce((sum, v) => sum + v * v, 0),
		);
		expect(mag).toBeCloseTo(1.0, 1);
	}, 120_000);

	it('embeds multiple strings', async () => {
		const result = await embedder.embed([
			'The cat sat on the mat',
			'A dog played in the park',
			'Machine learning is fascinating',
		]);

		expect(result.embeddings).toHaveLength(3);
		// All same dimensionality
		const dim = result.embeddings[0].length;
		expect(result.embeddings[1].length).toBe(dim);
		expect(result.embeddings[2].length).toBe(dim);
	}, 120_000);

	it('produces semantically meaningful embeddings', async () => {
		const result = await embedder.embed([
			'The cat sat on the mat',
			'A feline rested on the rug',
			'Quantum physics describes subatomic particles',
		]);

		const [cat, feline, physics] = result.embeddings;

		const simCatFeline = cosineSimilarity(cat, feline);
		const simCatPhysics = cosineSimilarity(cat, physics);

		// Similar sentences should have higher similarity
		expect(simCatFeline).toBeGreaterThan(simCatPhysics);
		expect(simCatFeline).toBeGreaterThan(0.5);
		expect(simCatPhysics).toBeLessThan(0.5);
	}, 120_000);

	it('reuses the pipeline across calls (no re-download)', async () => {
		const start = Date.now();
		await embedder.embed('Quick test');
		const elapsed = Date.now() - start;

		// Should be fast since model is already loaded from prior tests
		expect(elapsed).toBeLessThan(5_000);
	}, 10_000);
});
