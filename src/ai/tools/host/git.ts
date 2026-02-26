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

	// -- git_add -------------------------------------------------------------

	registerTool(
		registry,
		{
			name: 'git_add',
			description:
				'Stage files for the next commit. Provide specific paths or use all=true to stage everything.',
			parameters: {
				paths: {
					type: 'string',
					description:
						'Space-separated file paths to stage (relative to working directory)',
				},
				all: {
					type: 'boolean',
					description: 'If true, stage all changes (git add -A)',
				},
			},
			category: 'execute',
		},
		async (args) => {
			try {
				if (args.all === true) {
					return runGit(['add', '-A'], workingDirectory);
				}

				const paths = String(args.paths ?? '').trim();
				if (paths.length === 0) {
					throw new Error('Either provide paths to stage or set all=true');
				}

				const pathList = paths.split(/\s+/);
				return runGit(['add', ...pathList], workingDirectory);
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -- git_stash -----------------------------------------------------------

	registerTool(
		registry,
		{
			name: 'git_stash',
			description:
				'Save, apply, pop, or list stashed changes. Default action is save.',
			parameters: {
				action: {
					type: 'string',
					description:
						'Stash action: save, pop, apply, or list (default: save)',
				},
				message: {
					type: 'string',
					description: 'Optional message for the stash (only for save)',
				},
			},
			category: 'execute',
		},
		async (args) => {
			try {
				const action = String(args.action ?? 'save');

				switch (action) {
					case 'save': {
						const gitArgs = ['stash'];
						const message =
							typeof args.message === 'string' ? args.message : '';
						if (message.length > 0) {
							gitArgs.push('-m', message);
						}
						return runGit(gitArgs, workingDirectory);
					}
					case 'pop':
						return runGit(['stash', 'pop'], workingDirectory);
					case 'apply':
						return runGit(['stash', 'apply'], workingDirectory);
					case 'list':
						return runGit(['stash', 'list'], workingDirectory);
					default:
						throw new Error(
							`Unknown stash action: "${action}". Use save, pop, apply, or list.`,
						);
				}
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -- git_push ------------------------------------------------------------

	registerTool(
		registry,
		{
			name: 'git_push',
			description: 'Push commits to a remote repository. Defaults to origin.',
			parameters: {
				remote: {
					type: 'string',
					description: 'Remote name (default: origin)',
				},
				branch: {
					type: 'string',
					description: 'Branch to push (default: current branch)',
				},
				setUpstream: {
					type: 'boolean',
					description: 'If true, set upstream tracking (-u flag)',
				},
			},
			category: 'execute',
			annotations: { destructive: true },
		},
		async (args) => {
			try {
				const remote = String(args.remote ?? 'origin');
				const gitArgs = ['push'];

				if (args.setUpstream === true) {
					gitArgs.push('-u');
				}

				gitArgs.push(remote);

				const branch = typeof args.branch === 'string' ? args.branch : '';
				if (branch.length > 0) {
					gitArgs.push(branch);
				}

				return runGit(gitArgs, workingDirectory);
			} catch (err) {
				throw toError(err);
			}
		},
	);

	// -- git_pull ------------------------------------------------------------

	registerTool(
		registry,
		{
			name: 'git_pull',
			description: 'Pull changes from a remote repository. Defaults to origin.',
			parameters: {
				remote: {
					type: 'string',
					description: 'Remote name (default: origin)',
				},
				branch: {
					type: 'string',
					description: 'Branch to pull (default: current branch tracking)',
				},
			},
			category: 'execute',
		},
		async (args) => {
			try {
				const remote = String(args.remote ?? 'origin');
				const gitArgs = ['pull', remote];

				const branch = typeof args.branch === 'string' ? args.branch : '';
				if (branch.length > 0) {
					gitArgs.push(branch);
				}

				return runGit(gitArgs, workingDirectory);
			} catch (err) {
				throw toError(err);
			}
		},
	);
}
