/**
 * SimSE Code — @-File Mentions with Autocomplete
 *
 * Detects @path mentions in user input, resolves them to file content,
 * and provides Tab autocomplete for file paths.
 * No external deps.
 */

import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { basename, dirname, join, relative, resolve } from 'node:path';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface FileMention {
	readonly path: string;
	readonly content: string;
	readonly size: number;
}

export interface FileMentionResult {
	/** User input with @mentions removed. */
	readonly cleanInput: string;
	/** Resolved file mentions with their content. */
	readonly mentions: readonly FileMention[];
}

export interface FileMentionResolverOptions {
	/** Base directory for resolving relative paths. Default: process.cwd() */
	readonly baseDir?: string;
	/** Max file size to include (bytes). Default: 100KB */
	readonly maxFileSize?: number;
	/** Directories to exclude from autocomplete. */
	readonly excludeDirs?: readonly string[];
}

// ---------------------------------------------------------------------------
// Default config
// ---------------------------------------------------------------------------

const DEFAULT_MAX_SIZE = 100 * 1024; // 100KB

const DEFAULT_EXCLUDE_DIRS = new Set([
	'node_modules',
	'.git',
	'.svn',
	'.hg',
	'dist',
	'build',
	'out',
	'.next',
	'.cache',
	'coverage',
	'__pycache__',
	'.venv',
	'venv',
]);

// ---------------------------------------------------------------------------
// Mention resolution
// ---------------------------------------------------------------------------

const MENTION_PATTERN = /@([\w./-]+(?:\.\w+)?)/g;

/**
 * Resolve @-file mentions in user input.
 * Returns the cleaned input and an array of file mentions with content.
 */
export function resolveFileMentions(
	input: string,
	options?: FileMentionResolverOptions,
): FileMentionResult {
	const baseDir = options?.baseDir ?? process.cwd();
	const maxSize = options?.maxFileSize ?? DEFAULT_MAX_SIZE;
	const mentions: FileMention[] = [];
	const seen = new Set<string>();

	let cleanInput = input;
	let match: RegExpExecArray | null = MENTION_PATTERN.exec(input);

	while (match !== null) {
		const rawPath = match[1];
		const fullPath = resolve(baseDir, rawPath);

		if (!seen.has(fullPath) && existsSync(fullPath)) {
			try {
				const stat = statSync(fullPath);
				if (stat.isFile() && stat.size <= maxSize) {
					const content = readFileSync(fullPath, 'utf-8');
					mentions.push(
						Object.freeze({
							path: rawPath,
							content,
							size: stat.size,
						}),
					);
					seen.add(fullPath);
				}
			} catch {
				// Skip unreadable files
			}
		}

		match = MENTION_PATTERN.exec(input);
	}

	// Remove mentions from input text
	cleanInput = input.replace(MENTION_PATTERN, '').trim();

	return Object.freeze({
		cleanInput,
		mentions: Object.freeze(mentions),
	});
}

/**
 * Format resolved mentions as a context prefix for the AI prompt.
 */
export function formatMentionsAsContext(
	mentions: readonly FileMention[],
): string {
	if (mentions.length === 0) return '';

	const parts: string[] = [];
	for (const mention of mentions) {
		parts.push(`<file path="${mention.path}">\n${mention.content}\n</file>`);
	}
	return parts.join('\n\n');
}

// ---------------------------------------------------------------------------
// File autocomplete
// ---------------------------------------------------------------------------

/**
 * Provide Tab-completion candidates for @-file mentions.
 * Returns matching file paths relative to baseDir.
 */
export function completeFilePath(
	partial: string,
	options?: FileMentionResolverOptions,
): readonly string[] {
	const baseDir = options?.baseDir ?? process.cwd();
	const excludeDirs = new Set(options?.excludeDirs ?? DEFAULT_EXCLUDE_DIRS);

	// Determine directory to list and prefix to match
	const partialPath = resolve(baseDir, partial);
	let searchDir: string;
	let prefix: string;

	try {
		if (existsSync(partialPath) && statSync(partialPath).isDirectory()) {
			searchDir = partialPath;
			prefix = '';
		} else {
			searchDir = dirname(partialPath);
			prefix = basename(partialPath).toLowerCase();
		}
	} catch {
		return [];
	}

	if (!existsSync(searchDir)) return [];

	try {
		const entries = readdirSync(searchDir, { withFileTypes: true });
		const matches: string[] = [];

		for (const entry of entries) {
			if (excludeDirs.has(entry.name)) continue;
			if (entry.name.startsWith('.')) continue;

			const nameLC = entry.name.toLowerCase();
			if (prefix && !nameLC.startsWith(prefix) && !fuzzyMatch(prefix, nameLC)) {
				continue;
			}

			const fullEntryPath = join(searchDir, entry.name);
			const relPath = relative(baseDir, fullEntryPath).replace(/\\/g, '/');

			if (entry.isDirectory()) {
				matches.push(`${relPath}/`);
			} else {
				matches.push(relPath);
			}
		}

		return matches.sort();
	} catch {
		return [];
	}
}

/**
 * Simple fuzzy matching: chars in order.
 */
function fuzzyMatch(query: string, target: string): boolean {
	let qi = 0;
	for (let ti = 0; ti < target.length && qi < query.length; ti++) {
		if (query[qi] === target[ti]) qi++;
	}
	return qi === query.length;
}
