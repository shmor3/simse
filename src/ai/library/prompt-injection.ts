// ---------------------------------------------------------------------------
// Structured Prompt Injection — Memory Context Formatter
// ---------------------------------------------------------------------------
//
// Formats memory search results as structured XML tags or natural text
// for injection into the system prompt. This gives the LLM relevant
// context from past conversations without polluting the user's message.
// ---------------------------------------------------------------------------

import type { SearchResult } from './types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface PromptInjectionOptions {
	/** Maximum number of results to include. */
	readonly maxResults?: number;
	/** Minimum relevance score to include (0–1). */
	readonly minScore?: number;
	/** Output format. Defaults to `'structured'` (XML tags). */
	readonly format?: 'structured' | 'natural';
	/** XML tag name used for the outer wrapper. Defaults to `'memory-context'`. */
	readonly tag?: string;
	/** Maximum total characters in the output. Defaults to `4000`. */
	readonly maxChars?: number;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatAge(ms: number): string {
	const seconds = Math.floor(ms / 1000);
	if (seconds < 60) return `${seconds}s`;
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) return `${minutes}m`;
	const hours = Math.floor(minutes / 60);
	if (hours < 24) return `${hours}h`;
	const days = Math.floor(hours / 24);
	return `${days}d`;
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export function formatMemoryContext(
	results: readonly SearchResult[],
	options?: PromptInjectionOptions,
): string {
	if (results.length === 0) return '';

	const maxResults = options?.maxResults ?? results.length;
	const minScore = options?.minScore ?? 0;
	const format = options?.format ?? 'structured';
	const tag = options?.tag ?? 'memory-context';
	const maxChars = options?.maxChars ?? 4000;

	const filtered = results
		.filter((r) => r.score >= minScore)
		.slice(0, maxResults);

	if (filtered.length === 0) return '';

	const now = Date.now();

	if (format === 'natural') {
		const lines = ['Relevant context from memory:'];
		let chars = lines[0].length;
		for (const r of filtered) {
			const topic = r.entry.metadata.topic ?? 'uncategorized';
			const line = `- [${topic}] (relevance: ${r.score.toFixed(2)}) ${r.entry.text}`;
			if (chars + line.length > maxChars) break;
			lines.push(line);
			chars += line.length;
		}
		return lines.join('\n');
	}

	const entries: string[] = [];
	let chars = 0;
	for (const r of filtered) {
		const topic = r.entry.metadata.topic ?? 'uncategorized';
		const age = formatAge(now - r.entry.timestamp);
		const text = r.entry.text;
		const entry = `<entry topic="${topic}" relevance="${r.score.toFixed(2)}" age="${age}">\n${text}\n</entry>`;
		if (chars + entry.length > maxChars) break;
		entries.push(entry);
		chars += entry.length;
	}

	return `<${tag}>\n${entries.join('\n')}\n</${tag}>`;
}
