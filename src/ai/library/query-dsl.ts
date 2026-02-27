// ---------------------------------------------------------------------------
// Query DSL — parse a human-friendly query string into SearchOptions
// ---------------------------------------------------------------------------

import type { MetadataFilter, TextSearchMode } from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ParsedQuery {
	readonly textSearch?: {
		readonly query: string;
		readonly mode: TextSearchMode;
	};
	readonly topicFilter?: readonly string[];
	readonly metadataFilters?: readonly MetadataFilter[];
	readonly minScore?: number;
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/**
 * Split input on whitespace while preserving quoted strings as single tokens.
 * Unterminated quotes consume the rest of the string.
 */
function tokenize(input: string): string[] {
	const tokens: string[] = [];
	let i = 0;
	const len = input.length;

	while (i < len) {
		// Skip whitespace
		while (i < len && input[i] === ' ') i++;
		if (i >= len) break;

		if (input[i] === '"') {
			// Quoted token — find closing quote
			const start = i;
			i++; // skip opening quote
			const closingIdx = input.indexOf('"', i);
			if (closingIdx === -1) {
				// Unterminated quote — take rest of string including opening quote
				tokens.push(input.slice(start));
				break;
			}
			tokens.push(input.slice(start, closingIdx + 1));
			i = closingIdx + 1;
		} else {
			// Unquoted token — read until whitespace
			const start = i;
			while (i < len && input[i] !== ' ') i++;
			tokens.push(input.slice(start, i));
		}
	}

	return tokens;
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/**
 * Parse a DSL query string into a structured {@link ParsedQuery}.
 *
 * Supported syntax:
 * - `topic:path` — filter by topic
 * - `metadata:key=value` — metadata equals filter
 * - `"quoted text"` — exact phrase search
 * - `fuzzy~term` — fuzzy text search
 * - `score>N` — minimum score threshold
 * - Plain text — BM25 search (default)
 */
export function parseQuery(dsl: string): ParsedQuery {
	const tokens = tokenize(dsl);

	const topics: string[] = [];
	const metadataFilters: MetadataFilter[] = [];
	const plainParts: string[] = [];

	let quotedText: string | undefined;
	let fuzzyText: string | undefined;
	let minScore: number | undefined;

	for (const token of tokens) {
		if (token.startsWith('topic:')) {
			const value = token.slice('topic:'.length);
			if (value.length > 0) {
				topics.push(value);
			}
		} else if (token.startsWith('metadata:')) {
			const rest = token.slice('metadata:'.length);
			const eqIdx = rest.indexOf('=');
			if (eqIdx > 0) {
				metadataFilters.push({
					key: rest.slice(0, eqIdx),
					value: rest.slice(eqIdx + 1),
					mode: 'eq',
				});
			}
		} else if (token.startsWith('"')) {
			// Quoted string — strip surrounding quotes if present
			if (token.endsWith('"') && token.length > 1) {
				quotedText = token.slice(1, -1);
			} else {
				// Unterminated quote — strip opening quote only
				quotedText = token.slice(1);
			}
		} else if (token.startsWith('fuzzy~')) {
			const value = token.slice('fuzzy~'.length);
			if (value.length > 0) {
				fuzzyText = value;
			}
		} else if (token.startsWith('score>')) {
			const value = Number.parseFloat(token.slice('score>'.length));
			if (!Number.isNaN(value)) {
				minScore = value;
			}
		} else {
			plainParts.push(token);
		}
	}

	// Determine textSearch
	let textSearch: ParsedQuery['textSearch'];

	if (quotedText !== undefined) {
		// Quoted takes precedence
		textSearch = { query: quotedText, mode: 'exact' };
	} else if (fuzzyText !== undefined) {
		textSearch = { query: fuzzyText, mode: 'fuzzy' };
	} else {
		// Plain text tokens joined with space, or empty string
		textSearch = { query: plainParts.join(' '), mode: 'bm25' };
	}

	const result: Record<string, unknown> = {};

	if (textSearch) {
		result.textSearch = Object.freeze(textSearch);
	}

	if (topics.length > 0) {
		result.topicFilter = Object.freeze([...topics]);
	}

	if (metadataFilters.length > 0) {
		result.metadataFilters = Object.freeze(
			metadataFilters.map((f) => Object.freeze(f)),
		);
	}

	if (minScore !== undefined) {
		result.minScore = minScore;
	}

	return Object.freeze(result as ParsedQuery);
}
