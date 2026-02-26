// ---------------------------------------------------------------------------
// Host Filesystem Tools
//
// Registers read, write, edit, glob, grep, and list tools on a ToolRegistry.
// All paths are sandboxed to the configured working directory.
// ---------------------------------------------------------------------------

import { mkdir, readdir, readFile, stat, writeFile } from 'node:fs/promises';
import { dirname, join, relative, resolve } from 'node:path';
import { toError } from '../../../errors/base.js';
import type { ToolDefinition, ToolHandler, ToolRegistry } from '../types.js';
import { fuzzyMatch } from './fuzzy-edit.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface FilesystemToolOptions {
	readonly workingDirectory: string;
	readonly allowedPaths?: readonly string[];
}

// ---------------------------------------------------------------------------
// Path sandboxing
// ---------------------------------------------------------------------------

function resolveSandboxed(
	workingDirectory: string,
	inputPath: string,
	allowedPaths?: readonly string[],
): string {
	const resolved = resolve(workingDirectory, inputPath);
	const rel = relative(workingDirectory, resolved);

	// Escape detection: relative path must not start with '..' or be absolute
	if (rel.startsWith('..') || resolve(rel) === rel) {
		throw new Error(
			`Path "${inputPath}" escapes the working directory "${workingDirectory}"`,
		);
	}

	// If allowedPaths are specified, check against them too
	if (allowedPaths && allowedPaths.length > 0) {
		const allowed = allowedPaths.some((ap) => {
			const resolvedAllowed = resolve(ap);
			const relToAllowed = relative(resolvedAllowed, resolved);
			return (
				!relToAllowed.startsWith('..') && resolve(relToAllowed) !== relToAllowed
			);
		});
		if (!allowed) {
			throw new Error(`Path "${inputPath}" is not within any allowed path`);
		}
	}

	return resolved;
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

const registerTool = (
	registry: ToolRegistry,
	definition: ToolDefinition,
	handler: ToolHandler,
): void => {
	registry.register(definition, handler);
};

// ---------------------------------------------------------------------------
// Recursive directory listing
// ---------------------------------------------------------------------------

interface DirEntry {
	readonly name: string;
	readonly type: 'file' | 'directory';
}

async function listDirRecursive(
	dir: string,
	baseDir: string,
	maxDepth: number,
	currentDepth: number,
): Promise<DirEntry[]> {
	if (currentDepth > maxDepth) return [];

	const entries: DirEntry[] = [];
	const items = await readdir(dir, { withFileTypes: true });

	for (const item of items) {
		const relPath = relative(baseDir, join(dir, item.name));
		const normalizedPath = relPath.split('\\').join('/');

		if (item.isDirectory()) {
			entries.push({ name: normalizedPath, type: 'directory' });
			if (currentDepth < maxDepth) {
				const subEntries = await listDirRecursive(
					join(dir, item.name),
					baseDir,
					maxDepth,
					currentDepth + 1,
				);
				entries.push(...subEntries);
			}
		} else {
			entries.push({ name: normalizedPath, type: 'file' });
		}
	}

	return entries;
}

// ---------------------------------------------------------------------------
// Public registration
// ---------------------------------------------------------------------------

export function registerFilesystemTools(
	registry: ToolRegistry,
	options: FilesystemToolOptions,
): void {
	const { workingDirectory, allowedPaths } = options;

	const sandboxPath = (inputPath: string): string =>
		resolveSandboxed(workingDirectory, inputPath, allowedPaths);

	// -----------------------------------------------------------------------
	// fs_read — read file with optional line range
	// -----------------------------------------------------------------------
	registerTool(
		registry,
		{
			name: 'fs_read',
			description:
				'Read a file from the filesystem. Supports optional offset and limit for reading specific line ranges.',
			parameters: {
				path: {
					type: 'string',
					description: 'File path (relative to working directory)',
					required: true,
				},
				offset: {
					type: 'number',
					description: 'Starting line number (1-based, default: 1)',
				},
				limit: {
					type: 'number',
					description: 'Maximum number of lines to return',
				},
			},
			category: 'read',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const filePath = sandboxPath(String(args.path ?? ''));
				const content = await readFile(filePath, 'utf-8');
				const lines = content.split('\n');

				const offset =
					typeof args.offset === 'number' ? Math.max(1, args.offset) : 1;
				const limit =
					typeof args.limit === 'number' ? args.limit : lines.length;

				const start = offset - 1; // Convert to 0-based
				const sliced = lines.slice(start, start + limit);

				// Format with line numbers
				return sliced
					.map(
						(line, i) => `${String(start + i + 1).padStart(6, ' ')}\t${line}`,
					)
					.join('\n');
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -----------------------------------------------------------------------
	// fs_write — create or overwrite a file
	// -----------------------------------------------------------------------
	registerTool(
		registry,
		{
			name: 'fs_write',
			description:
				'Write content to a file. Creates parent directories automatically. Overwrites existing files.',
			parameters: {
				path: {
					type: 'string',
					description: 'File path (relative to working directory)',
					required: true,
				},
				content: {
					type: 'string',
					description: 'The content to write',
					required: true,
				},
			},
			category: 'edit',
			annotations: { destructive: true },
		},
		async (args) => {
			try {
				const filePath = sandboxPath(String(args.path ?? ''));
				const content = String(args.content ?? '');

				// Auto-create parent directories
				await mkdir(dirname(filePath), { recursive: true });
				await writeFile(filePath, content, 'utf-8');

				const bytes = Buffer.byteLength(content, 'utf-8');
				return `Wrote ${bytes} bytes to ${String(args.path)}`;
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -----------------------------------------------------------------------
	// fs_edit — fuzzy edit using fuzzyMatch
	// -----------------------------------------------------------------------
	registerTool(
		registry,
		{
			name: 'fs_edit',
			description:
				'Edit a file by replacing text. Uses 5-strategy fuzzy matching: exact, line-trimmed, whitespace-normalized, indentation-flexible, and block-anchor with Levenshtein distance.',
			parameters: {
				path: {
					type: 'string',
					description: 'File path (relative to working directory)',
					required: true,
				},
				old_string: {
					type: 'string',
					description: 'The text to find and replace',
					required: true,
				},
				new_string: {
					type: 'string',
					description: 'The replacement text',
					required: true,
				},
			},
			category: 'edit',
		},
		async (args) => {
			try {
				const filePath = sandboxPath(String(args.path ?? ''));
				const oldStr = String(args.old_string ?? '');
				const newStr = String(args.new_string ?? '');

				const content = await readFile(filePath, 'utf-8');
				const result = fuzzyMatch(content, oldStr, newStr);

				if (!result) {
					throw new Error(
						'No match found for the provided old_string in the file. Ensure the text exists in the file.',
					);
				}

				await writeFile(filePath, result.replaced, 'utf-8');
				return `Edited ${String(args.path)} using strategy: ${result.strategy}`;
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -----------------------------------------------------------------------
	// fs_glob — find files using Bun.Glob
	// -----------------------------------------------------------------------
	registerTool(
		registry,
		{
			name: 'fs_glob',
			description:
				'Find files matching a glob pattern within the working directory. Returns up to 1000 results.',
			parameters: {
				pattern: {
					type: 'string',
					description: 'Glob pattern (e.g. "**/*.ts", "src/**/*.json")',
					required: true,
				},
			},
			category: 'search',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const pattern = String(args.pattern ?? '**/*');
				const glob = new Bun.Glob(pattern);
				const matches: string[] = [];
				const limit = 1000;

				for await (const path of glob.scan({
					cwd: workingDirectory,
					dot: false,
				})) {
					matches.push(path);
					if (matches.length >= limit) break;
				}

				if (matches.length === 0) return 'No files found matching the pattern.';

				// Normalize to forward slashes
				const normalized = matches.map((p) => p.split('\\').join('/'));
				normalized.sort();

				let output = normalized.join('\n');
				if (matches.length >= limit) {
					output += `\n\n(Results limited to ${limit} entries)`;
				}
				return output;
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -----------------------------------------------------------------------
	// fs_grep — regex search file contents
	// -----------------------------------------------------------------------
	registerTool(
		registry,
		{
			name: 'fs_grep',
			description:
				'Search file contents using a regex pattern. Searches all files in the working directory or a specific path. Returns up to 500 results.',
			parameters: {
				pattern: {
					type: 'string',
					description: 'Regular expression pattern to search for',
					required: true,
				},
				path: {
					type: 'string',
					description:
						'File or directory path to search in (default: working directory)',
				},
				glob: {
					type: 'string',
					description: 'Glob pattern to filter files (e.g. "*.ts")',
				},
			},
			category: 'search',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const searchPattern = String(args.pattern ?? '');
				const regex = new RegExp(searchPattern, 'g');
				const searchPath = args.path
					? sandboxPath(String(args.path))
					: workingDirectory;
				const fileGlob = String(args.glob ?? '**/*');

				const limit = 500;
				const results: string[] = [];

				// Find files to search
				const glob = new Bun.Glob(fileGlob);

				for await (const filePath of glob.scan({
					cwd: searchPath,
					dot: false,
				})) {
					if (results.length >= limit) break;

					const fullPath = join(searchPath, filePath);

					try {
						const fileStat = await stat(fullPath);
						if (!fileStat.isFile()) continue;
						// Skip large files (> 1MB)
						if (fileStat.size > 1024 * 1024) continue;

						const content = await readFile(fullPath, 'utf-8');
						const lines = content.split('\n');
						const normalizedFilePath = filePath.split('\\').join('/');

						for (let i = 0; i < lines.length; i++) {
							if (results.length >= limit) break;
							// Reset regex state for each line
							regex.lastIndex = 0;
							if (regex.test(lines[i])) {
								results.push(`${normalizedFilePath}:${i + 1}: ${lines[i]}`);
							}
						}
					} catch {
						// Skip unreadable files
					}
				}

				if (results.length === 0) return 'No matches found.';

				let output = results.join('\n');
				if (results.length >= limit) {
					output += `\n\n(Results limited to ${limit} entries)`;
				}
				return output;
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -----------------------------------------------------------------------
	// fs_list — list directory with configurable depth
	// -----------------------------------------------------------------------
	registerTool(
		registry,
		{
			name: 'fs_list',
			description:
				'List files and directories in a path with configurable depth.',
			parameters: {
				path: {
					type: 'string',
					description:
						'Directory path (relative to working directory, default: ".")',
				},
				depth: {
					type: 'number',
					description: 'Maximum directory depth to recurse (default: 1)',
				},
			},
			category: 'read',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const dirPath = args.path
					? sandboxPath(String(args.path))
					: workingDirectory;
				const maxDepth =
					typeof args.depth === 'number' ? Math.max(0, args.depth) : 1;

				const entries = await listDirRecursive(dirPath, dirPath, maxDepth, 0);

				if (entries.length === 0) return 'Directory is empty.';

				return entries
					.map((e) => `${e.type === 'directory' ? 'd' : 'f'} ${e.name}`)
					.join('\n');
			} catch (err) {
				throw toError(err);
			}
		},
	);
}
