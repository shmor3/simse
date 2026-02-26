// ---------------------------------------------------------------------------
// Instruction File Discovery
//
// Scans a project root for well-known instruction files (CLAUDE.md,
// AGENTS.md, .simse/instructions.md, etc.) and returns their contents.
// Missing files are silently skipped.
// ---------------------------------------------------------------------------

import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import type {
	DiscoveredInstruction,
	InstructionDiscoveryOptions,
} from './types.js';

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

const DEFAULT_PATTERNS: readonly string[] = [
	'CLAUDE.md',
	'AGENTS.md',
	'.simse/instructions.md',
];

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

export async function discoverInstructions(
	options: InstructionDiscoveryOptions,
): Promise<readonly DiscoveredInstruction[]> {
	const patterns = options.patterns ?? DEFAULT_PATTERNS;
	const results: DiscoveredInstruction[] = [];

	for (const pattern of patterns) {
		const fullPath = resolve(options.rootDir, pattern);
		try {
			const content = await readFile(fullPath, 'utf-8');
			results.push(Object.freeze({ path: fullPath, content }));
		} catch {
			// File not found or unreadable — skip silently.
		}
	}

	return Object.freeze(results);
}
