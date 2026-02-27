/**
 * SimSE Code — @-Mentions with Autocomplete
 *
 * Detects @path, @vfs://path, and @noteId mentions in user input,
 * resolves them to content, and provides Tab autocomplete.
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
	readonly kind: 'file' | 'vfs' | 'note';
	/** For note mentions: topic label. */
	readonly topic?: string;
}

export interface FileMentionResult {
	/** User input with @mentions removed. */
	readonly cleanInput: string;
	/** Resolved mentions with their content. */
	readonly mentions: readonly FileMention[];
}

export interface FileMentionResolverOptions {
	/** Base directory for resolving relative paths. Default: process.cwd() */
	readonly baseDir?: string;
	/** Max file size to include (bytes). Default: 100KB */
	readonly maxFileSize?: number;
	/** Directories to exclude from autocomplete. */
	readonly excludeDirs?: readonly string[];
	/** Resolve a VFS path to its content. */
	readonly resolveVFS?: (
		path: string,
	) => { content: string; size: number } | undefined;
	/** Resolve a library note by 8-char ID prefix. */
	readonly resolveNote?: (
		idPrefix: string,
	) => { id: string; text: string; topic: string } | undefined;
}

export interface AtMentionCompleteOptions extends FileMentionResolverOptions {
	/** Complete VFS paths given a partial (after `vfs://`). */
	readonly completeVFS?: (partial: string) => readonly string[];
	/** Complete note IDs given a partial prefix. */
	readonly completeNote?: (partial: string) => readonly string[];
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

/** Matches @vfs://path, @file/path.ext, and @hexid */
const MENTION_PATTERN = /@(vfs:\/\/[\w./-]+|[\w./-]+(?:\.\w+)?)/g;

/** 8-char lowercase alphanumeric — note ID prefix. */
const NOTE_ID_RE = /^[a-f0-9]{8}$/;

function isNoteIdPrefix(s: string): boolean {
	return NOTE_ID_RE.test(s);
}

/**
 * Resolve @-mentions in user input.
 * Returns the cleaned input and an array of resolved mentions.
 */
export function resolveFileMentions(
	input: string,
	options?: FileMentionResolverOptions,
): FileMentionResult {
	const baseDir = options?.baseDir ?? process.cwd();
	const maxSize = options?.maxFileSize ?? DEFAULT_MAX_SIZE;
	const mentions: FileMention[] = [];
	const seen = new Set<string>();

	let match: RegExpExecArray | null = MENTION_PATTERN.exec(input);

	while (match !== null) {
		const raw = match[1];

		if (raw.startsWith('vfs://')) {
			// VFS mention
			const vfsPath = raw.slice(6); // strip "vfs://"
			if (!seen.has(raw) && options?.resolveVFS) {
				const resolved = options.resolveVFS(vfsPath);
				if (resolved) {
					mentions.push(
						Object.freeze({
							path: raw,
							content: resolved.content,
							size: resolved.size,
							kind: 'vfs' as const,
						}),
					);
					seen.add(raw);
				}
			}
		} else if (
			isNoteIdPrefix(raw) &&
			!raw.includes('/') &&
			!raw.includes('.')
		) {
			// Note ID mention
			if (!seen.has(raw) && options?.resolveNote) {
				const resolved = options.resolveNote(raw);
				if (resolved) {
					mentions.push(
						Object.freeze({
							path: raw,
							content: resolved.text,
							size: resolved.text.length,
							kind: 'note' as const,
							topic: resolved.topic,
						}),
					);
					seen.add(raw);
				}
			}
		} else {
			// Filesystem mention
			const fullPath = resolve(baseDir, raw);
			if (!seen.has(fullPath) && existsSync(fullPath)) {
				try {
					const stat = statSync(fullPath);
					if (stat.isFile() && stat.size <= maxSize) {
						const content = readFileSync(fullPath, 'utf-8');
						mentions.push(
							Object.freeze({
								path: raw,
								content,
								size: stat.size,
								kind: 'file' as const,
							}),
						);
						seen.add(fullPath);
					}
				} catch {
					// Skip unreadable files
				}
			}
		}

		match = MENTION_PATTERN.exec(input);
	}

	// Remove mentions from input text
	const cleanInput = input.replace(MENTION_PATTERN, '').trim();

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
		if (mention.kind === 'note') {
			parts.push(
				`<note id="${mention.path}"${mention.topic ? ` topic="${mention.topic}"` : ''}>\n${mention.content}\n</note>`,
			);
		} else {
			parts.push(
				`<file path="${mention.path}">\n${mention.content}\n</file>`,
			);
		}
	}
	return parts.join('\n\n');
}

// ---------------------------------------------------------------------------
// Autocomplete
// ---------------------------------------------------------------------------

/**
 * Provide Tab-completion candidates for @-mentions.
 * Dispatches to VFS, note, or filesystem completion based on prefix.
 */
export function completeAtMention(
	partial: string,
	options?: AtMentionCompleteOptions,
): readonly string[] {
	if (partial.startsWith('vfs://')) {
		const vfsPartial = partial.slice(6);
		return options?.completeVFS?.(vfsPartial) ?? [];
	}

	// Short alphanum → note ID prefix
	if (/^[a-f0-9]+$/.test(partial) && partial.length <= 8) {
		const noteResults = options?.completeNote?.(partial) ?? [];
		// Also try filesystem as fallback
		const fileResults = completeFilePath(partial, options);
		// Combine, notes first
		const combined = [...noteResults, ...fileResults];
		return [...new Set(combined)];
	}

	return completeFilePath(partial, options);
}

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
