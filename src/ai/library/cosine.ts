// ---------------------------------------------------------------------------
// Cosine similarity — pure math, no dependencies
// ---------------------------------------------------------------------------

/**
 * Compute the cosine similarity between two vectors.
 * Returns 0 for zero-magnitude vectors or dimension mismatches.
 */
export function cosineSimilarity(
	a: readonly number[],
	b: readonly number[],
): number {
	if (a.length !== b.length || a.length === 0) return 0;

	let dot = 0;
	let normA = 0;
	let normB = 0;

	for (let i = 0; i < a.length; i++) {
		const ai = a[i];
		const bi = b[i];
		dot += ai * bi;
		normA += ai * ai;
		normB += bi * bi;
	}

	const denom = Math.sqrt(normA) * Math.sqrt(normB);
	const result = denom === 0 ? 0 : dot / denom;
	// Guard against NaN/Infinity and clamp to [-1, 1] for floating-point rounding
	if (!Number.isFinite(result)) return 0;
	return Math.min(1, Math.max(-1, result));
}
