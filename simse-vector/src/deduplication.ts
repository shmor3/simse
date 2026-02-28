// ---------------------------------------------------------------------------
// Deduplication — detect and group near-duplicate vector entries
// ---------------------------------------------------------------------------
//
// Pure functions that operate on Volume arrays. No side effects,
// no external dependencies beyond cosine similarity.
// ---------------------------------------------------------------------------

import { cosineSimilarity } from './cosine.js';
import type {
	DuplicateCheckResult,
	DuplicateVolumes,
	Volume,
} from './types.js';

// ---------------------------------------------------------------------------
// Single-entry duplicate check
// ---------------------------------------------------------------------------

/**
 * Check whether `newEmbedding` is a near-duplicate of any existing entry.
 *
 * Returns the closest match above `threshold`, or `{ isDuplicate: false }`
 * if no match is found. Linear scan — O(N).
 */
export function checkDuplicate(
	newEmbedding: readonly number[],
	entries: readonly Volume[],
	threshold: number,
): DuplicateCheckResult {
	let bestEntry: Volume | undefined;
	let bestSimilarity = -Infinity;

	for (const entry of entries) {
		if (entry.embedding.length !== newEmbedding.length) continue;

		const sim = cosineSimilarity(newEmbedding, entry.embedding);
		if (sim >= threshold && sim > bestSimilarity) {
			bestSimilarity = sim;
			bestEntry = entry;
		}
	}

	if (bestEntry) {
		return {
			isDuplicate: true,
			existingVolume: bestEntry,
			similarity: bestSimilarity,
		};
	}

	return { isDuplicate: false };
}

// ---------------------------------------------------------------------------
// Group duplicate detection
// ---------------------------------------------------------------------------

/**
 * Find groups of near-duplicate entries using greedy clustering.
 *
 * Entries are processed in timestamp order (oldest first). For each entry,
 * if it is similar enough to an existing group's representative, it joins
 * that group. Otherwise it starts a new group.
 *
 * O(N^2) — intended for explicit user-triggered deduplication, not hot paths.
 */
export function findDuplicateVolumes(
	entries: readonly Volume[],
	threshold: number,
): DuplicateVolumes[] {
	if (entries.length < 2) return [];

	// Sort by timestamp (oldest first) so the representative is the original
	const sorted = [...entries].sort((a, b) => a.timestamp - b.timestamp);

	const groups: Array<{
		representative: Volume;
		duplicates: Volume[];
		totalSimilarity: number;
	}> = [];

	for (const entry of sorted) {
		let assigned = false;

		for (const group of groups) {
			if (group.representative.embedding.length !== entry.embedding.length)
				continue;

			const sim = cosineSimilarity(
				group.representative.embedding,
				entry.embedding,
			);
			if (sim >= threshold) {
				group.duplicates.push(entry);
				group.totalSimilarity += sim;
				assigned = true;
				break;
			}
		}

		if (!assigned) {
			groups.push({
				representative: entry,
				duplicates: [],
				totalSimilarity: 0,
			});
		}
	}

	// Only return groups that actually have duplicates
	return groups
		.filter((g) => g.duplicates.length > 0)
		.map((g) => ({
			representative: g.representative,
			duplicates: g.duplicates,
			averageSimilarity:
				g.duplicates.length > 0 ? g.totalSimilarity / g.duplicates.length : 0,
		}));
}
