// ---------------------------------------------------------------------------
// Format search results for chain injection
// ---------------------------------------------------------------------------

import type { Lookup } from 'simse-vector';

export interface FormatSearchResultsOptions {
	/** Message returned when there are no results. Defaults to `'(no relevant memories found)'`. */
	readonly emptyMessage?: string;
	/** Number of decimal places for score display. Defaults to `3`. */
	readonly scorePrecision?: number;
}

/**
 * Format search results as a readable string for chain injection.
 */
export function formatSearchResults(
	results: Lookup[],
	options?: FormatSearchResultsOptions,
): string {
	if (results.length === 0) {
		return options?.emptyMessage ?? '(no relevant volumes found)';
	}
	const precision = options?.scorePrecision ?? 3;
	return results
		.map(
			(r, i) =>
				`[${i + 1}] (score: ${r.score.toFixed(precision)})\n${r.volume.text}`,
		)
		.join('\n\n');
}
