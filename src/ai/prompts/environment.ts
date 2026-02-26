// ---------------------------------------------------------------------------
// Environment Context Collector
//
// Gathers runtime environment information for system prompt construction:
// platform, shell, cwd, date, and git branch/status.
// ---------------------------------------------------------------------------

import { execFileSync } from 'node:child_process';
import type { EnvironmentContext } from './types.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function execQuiet(
	cmd: string,
	args: readonly string[],
	cwd: string,
): string | undefined {
	try {
		return execFileSync(cmd, args as string[], {
			cwd,
			timeout: 5_000,
			encoding: 'utf-8',
			stdio: ['ignore', 'pipe', 'ignore'],
		}).trim();
	} catch {
		return undefined;
	}
}

// ---------------------------------------------------------------------------
// Public
// ---------------------------------------------------------------------------

/**
 * Collect environment context for the current working directory.
 *
 * @param cwd - Directory to inspect for git state. Defaults to `process.cwd()`.
 * @returns A frozen {@link EnvironmentContext} with platform, shell, cwd, date,
 *   and optional git branch/status.
 */
export function collectEnvironmentContext(cwd?: string): EnvironmentContext {
	const resolvedCwd = cwd ?? process.cwd();

	const gitBranch = execQuiet(
		'git',
		['rev-parse', '--abbrev-ref', 'HEAD'],
		resolvedCwd,
	);

	let gitStatus: string | undefined;
	if (gitBranch) {
		const raw = execQuiet('git', ['status', '--porcelain'], resolvedCwd);
		if (raw !== undefined) {
			gitStatus = raw.length === 0 ? 'clean' : raw;
		}
	}

	return Object.freeze({
		platform: process.platform,
		shell: process.env.SHELL ?? process.env.COMSPEC ?? 'unknown',
		cwd: resolvedCwd,
		date: new Date().toISOString().slice(0, 10),
		gitBranch,
		gitStatus,
	});
}
