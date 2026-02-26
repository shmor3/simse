// ---------------------------------------------------------------------------
// Git Tools
//
// Registers git operations (status, diff, log, commit, branch) with a
// ToolRegistry. All commands run via Bun.spawnSync in a configurable
// working directory.
// ---------------------------------------------------------------------------

import { toError } from '../../../errors/base.js';
import type { ToolDefinition, ToolHandler, ToolRegistry } from '../types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface GitToolOptions {
	readonly workingDirectory: string;
}

// ---------------------------------------------------------------------------
// Internal helper
// ---------------------------------------------------------------------------

function runGit(args: readonly string[], cwd: string): string {
	const result = Bun.spawnSync(['git', ...args], {
		cwd,
		stdout: 'pipe',
		stderr: 'pipe',
	});

	const stdout = new TextDecoder().decode(result.stdout).trim();
	const stderr = new TextDecoder().decode(result.stderr).trim();

	if (result.exitCode !== 0) {
		throw new Error(
			stderr || `git ${args[0]} failed with exit code ${result.exitCode}`,
		);
	}

	return stdout;
}

// ---------------------------------------------------------------------------
// Registration helper
// ---------------------------------------------------------------------------

const registerTool = (
	registry: ToolRegistry,
	definition: ToolDefinition,
	handler: ToolHandler,
): void => {
	registry.register(definition, handler);
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export function registerGitTools(
	registry: ToolRegistry,
	options: GitToolOptions,
): void {
	const { workingDirectory } = options;

	// -- git_status ----------------------------------------------------------

	registerTool(
		registry,
		{
			name: 'git_status',
			description: 'Show the working tree status of the git repository.',
			parameters: {},
			category: 'read',
			annotations: { readOnly: true },
		},
		async () => {
			try {
				return runGit(['status'], workingDirectory);
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -- git_diff ------------------------------------------------------------

	registerTool(
		registry,
		{
			name: 'git_diff',
			description:
				'Show changes in the working tree or staging area. Use staged=true for staged changes.',
			parameters: {
				staged: {
					type: 'boolean',
					description: 'If true, show staged changes (--cached)',
				},
				path: {
					type: 'string',
					description: 'Limit diff to a specific file path',
				},
			},
			category: 'read',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const gitArgs: string[] = ['diff'];
				if (args.staged === true) {
					gitArgs.push('--cached');
				}
				if (typeof args.path === 'string' && args.path.length > 0) {
					gitArgs.push('--', args.path);
				}
				return runGit(gitArgs, workingDirectory);
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -- git_log -------------------------------------------------------------

	registerTool(
		registry,
		{
			name: 'git_log',
			description:
				'Show commit log history. Defaults to 10 commits in oneline format.',
			parameters: {
				count: {
					type: 'number',
					description: 'Number of commits to show (default: 10)',
				},
				oneline: {
					type: 'boolean',
					description: 'Use oneline format (default: true)',
				},
			},
			category: 'read',
			annotations: { readOnly: true },
		},
		async (args) => {
			try {
				const count = typeof args.count === 'number' ? args.count : 10;
				const oneline = args.oneline !== false;
				const gitArgs: string[] = ['log', `-${count}`];
				if (oneline) {
					gitArgs.push('--oneline');
				}
				return runGit(gitArgs, workingDirectory);
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -- git_commit ----------------------------------------------------------

	registerTool(
		registry,
		{
			name: 'git_commit',
			description:
				'Create a git commit with the staged changes and the given message.',
			parameters: {
				message: {
					type: 'string',
					description: 'The commit message',
					required: true,
				},
			},
			category: 'execute',
			annotations: { destructive: true },
		},
		async (args) => {
			try {
				const message = String(args.message ?? '');
				if (message.length === 0) {
					throw new Error('Commit message is required');
				}
				return runGit(['commit', '-m', message], workingDirectory);
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -- git_branch ----------------------------------------------------------

	registerTool(
		registry,
		{
			name: 'git_branch',
			description:
				'List, create, or switch branches. With no name, lists branches. With create=true, creates a new branch. Otherwise, switches to the named branch.',
			parameters: {
				name: {
					type: 'string',
					description: 'Branch name to create or switch to',
				},
				create: {
					type: 'boolean',
					description: 'If true, create a new branch with the given name',
				},
			},
			category: 'execute',
		},
		async (args) => {
			try {
				const name = typeof args.name === 'string' ? args.name : '';
				const create = args.create === true;

				if (name.length === 0) {
					return runGit(['branch'], workingDirectory);
				}

				if (create) {
					return runGit(['branch', name], workingDirectory);
				}

				return runGit(['checkout', name], workingDirectory);
			} catch (err) {
				throw toError(err);
			}
		},
	);
}
