/**
 * /init command — scan current directory and generate SIMSE.md via ACP.
 *
 * Usage:
 *   /init          — Scan cwd, generate SIMSE.md + .simse/
 *   /init --force  — Overwrite existing SIMSE.md
 */

import {
	existsSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	statSync,
	writeFileSync,
} from 'node:fs';
import { join, relative } from 'node:path';
import type { ACPClient } from 'simse';
import type { CommandDefinition } from '../../ink-types.js';

// ---------------------------------------------------------------------------
// Directory scanning
// ---------------------------------------------------------------------------

const SKIP_DIRS = new Set([
	'node_modules',
	'.git',
	'dist',
	'build',
	'__pycache__',
	'.next',
	'.turbo',
	'.cache',
	'coverage',
]);

const KEY_FILES = new Set([
	'package.json',
	'Cargo.toml',
	'go.mod',
	'pyproject.toml',
	'requirements.txt',
	'tsconfig.json',
	'.gitignore',
	'README.md',
]);

const MAX_FILES = 200;
const MAX_KEY_FILE_LINES = 200;

interface ScanResult {
	readonly tree: string;
	readonly keyFileContents: string;
}

function scanDirectory(cwd: string, maxDepth = 3): ScanResult {
	const paths: string[] = [];
	const keyFiles: { path: string; content: string }[] = [];

	function walk(dir: string, depth: number): void {
		if (depth > maxDepth || paths.length >= MAX_FILES) return;

		let entries: string[];
		try {
			entries = readdirSync(dir);
		} catch {
			return;
		}

		for (const entry of entries) {
			if (paths.length >= MAX_FILES) break;
			if (SKIP_DIRS.has(entry)) continue;

			const fullPath = join(dir, entry);
			const relPath = relative(cwd, fullPath);

			let stat: ReturnType<typeof statSync>;
			try {
				stat = statSync(fullPath);
			} catch {
				continue;
			}

			if (stat.isDirectory()) {
				paths.push(`${relPath}/`);
				walk(fullPath, depth + 1);
			} else {
				paths.push(relPath);

				if (KEY_FILES.has(entry)) {
					try {
						const raw = readFileSync(fullPath, 'utf-8');
						const lines = raw.split('\n');
						const truncated =
							lines.length > MAX_KEY_FILE_LINES
								? lines.slice(0, MAX_KEY_FILE_LINES).join('\n') +
									`\n... (truncated at ${MAX_KEY_FILE_LINES} lines)`
								: raw;
						keyFiles.push({ path: relPath, content: truncated });
					} catch {
						// skip unreadable files
					}
				}
			}
		}
	}

	walk(cwd, 0);

	const tree = paths.join('\n');
	const keyFileContents = keyFiles
		.map((f) => `--- ${f.path} ---\n${f.content}`)
		.join('\n\n');

	return { tree, keyFileContents };
}

// ---------------------------------------------------------------------------
// Command factory
// ---------------------------------------------------------------------------

export interface InitCommandContext {
	readonly getAcpClient: () => ACPClient;
	readonly getServerName: () => string | undefined;
	readonly hasACP: () => boolean;
}

export function createInitCommands(
	ctx: InitCommandContext,
): readonly CommandDefinition[] {
	return [
		{
			name: 'init',
			aliases: ['initialize'],
			usage: '/init [--force]',
			description: 'Initialize project: scan cwd and generate SIMSE.md via AI',
			category: 'config',
			execute: async (args) => {
				const force = args.trim() === '--force';

				if (!ctx.hasACP()) {
					return {
						text: 'No ACP server connected. Run /setup to configure one first.',
					};
				}

				const cwd = process.cwd();
				const simseMdPath = join(cwd, 'SIMSE.md');

				if (existsSync(simseMdPath) && !force) {
					return {
						text: 'SIMSE.md already exists. Use /init --force to overwrite.',
					};
				}

				// Scan directory
				const { tree, keyFileContents } = scanDirectory(cwd);

				const prompt = `Analyze this project and generate a SIMSE.md file — a concise reference for an AI coding assistant working in this codebase.

Include sections for:
- Project overview (1-2 sentences)
- Tech stack and key dependencies
- Build, test, and lint commands
- Project structure overview
- Key patterns and conventions
- Important notes

Directory tree:
${tree}

Key files:
${keyFileContents}`;

				try {
					const result = await ctx.getAcpClient().generate(prompt, {
						serverName: ctx.getServerName(),
					});

					writeFileSync(simseMdPath, result.content, 'utf-8');

					// Create .simse/ with empty settings if missing
					const simseDir = join(cwd, '.simse');
					const settingsPath = join(simseDir, 'settings.json');
					if (!existsSync(settingsPath)) {
						if (!existsSync(simseDir)) {
							mkdirSync(simseDir, { recursive: true });
						}
						writeFileSync(
							settingsPath,
							`${JSON.stringify({}, null, 2)}\n`,
							'utf-8',
						);
					}

					return {
						text: 'Created SIMSE.md and .simse/ — your project is ready.',
					};
				} catch (err) {
					return {
						text: `Failed to generate SIMSE.md: ${err instanceof Error ? err.message : 'Unknown error'}`,
					};
				}
			},
		},
	];
}
