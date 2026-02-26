import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { registerGitTools } from '../src/ai/tools/host/git.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import type { ToolRegistry } from '../src/ai/tools/types.js';
import { createSilentLogger } from './utils/mocks.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function shell(cmd: string, cwd: string): void {
	const result = Bun.spawnSync(['bash', '-c', cmd], {
		cwd,
		stdout: 'pipe',
		stderr: 'pipe',
	});
	if (result.exitCode !== 0) {
		const stderr = new TextDecoder().decode(result.stderr).trim();
		throw new Error(`Shell command failed: ${cmd}\n${stderr}`);
	}
}

function initGitRepo(cwd: string): void {
	shell('git init', cwd);
	shell('git config user.email "test@test.com"', cwd);
	shell('git config user.name "Test User"', cwd);
	shell(
		'echo "initial" > README.md && git add README.md && git commit -m "Initial commit"',
		cwd,
	);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('registerGitTools', () => {
	let registry: ToolRegistry;
	let tmpDir: string;

	beforeEach(() => {
		tmpDir = mkdtempSync(join(tmpdir(), 'simse-git-test-'));
		initGitRepo(tmpDir);
		registry = createToolRegistry({ logger: createSilentLogger() });
		registerGitTools(registry, { workingDirectory: tmpDir });
	});

	afterEach(() => {
		rmSync(tmpDir, { recursive: true, force: true });
	});

	it('registers all five git tools', () => {
		expect(registry.toolNames).toContain('git_status');
		expect(registry.toolNames).toContain('git_diff');
		expect(registry.toolNames).toContain('git_log');
		expect(registry.toolNames).toContain('git_commit');
		expect(registry.toolNames).toContain('git_branch');
	});

	// -----------------------------------------------------------------------
	// git_status
	// -----------------------------------------------------------------------

	it('git_status shows clean working tree', async () => {
		const result = await registry.execute({
			id: 'gs1',
			name: 'git_status',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('nothing to commit');
	});

	it('git_status shows untracked files', async () => {
		shell('echo "new file" > untracked.txt', tmpDir);

		const result = await registry.execute({
			id: 'gs2',
			name: 'git_status',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('untracked.txt');
	});

	// -----------------------------------------------------------------------
	// git_diff
	// -----------------------------------------------------------------------

	it('git_diff shows unstaged changes', async () => {
		shell('echo "modified" >> README.md', tmpDir);

		const result = await registry.execute({
			id: 'gd1',
			name: 'git_diff',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('modified');
	});

	it('git_diff shows staged changes with staged=true', async () => {
		shell('echo "staged change" >> README.md && git add README.md', tmpDir);

		const result = await registry.execute({
			id: 'gd2',
			name: 'git_diff',
			arguments: { staged: true },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('staged change');
	});

	it('git_diff filters by path', async () => {
		shell('echo "a" > a.txt && echo "b" > b.txt', tmpDir);

		const result = await registry.execute({
			id: 'gd3',
			name: 'git_diff',
			arguments: { path: 'a.txt' },
		});
		// Untracked files don't show in diff, so this should be empty
		expect(result.isError).toBe(false);
	});

	// -----------------------------------------------------------------------
	// git_log
	// -----------------------------------------------------------------------

	it('git_log shows commit history', async () => {
		const result = await registry.execute({
			id: 'gl1',
			name: 'git_log',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('Initial commit');
	});

	it('git_log respects count parameter', async () => {
		shell(
			'echo "second" > second.txt && git add second.txt && git commit -m "Second commit"',
			tmpDir,
		);
		shell(
			'echo "third" > third.txt && git add third.txt && git commit -m "Third commit"',
			tmpDir,
		);

		const result = await registry.execute({
			id: 'gl2',
			name: 'git_log',
			arguments: { count: 1 },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('Third commit');
		expect(result.output).not.toContain('Initial commit');
	});

	it('git_log supports oneline=false for full format', async () => {
		const result = await registry.execute({
			id: 'gl3',
			name: 'git_log',
			arguments: { oneline: false },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('Author:');
	});

	// -----------------------------------------------------------------------
	// git_commit
	// -----------------------------------------------------------------------

	it('git_commit creates a commit with staged changes', async () => {
		shell('echo "new" > new.txt && git add new.txt', tmpDir);

		const result = await registry.execute({
			id: 'gc1',
			name: 'git_commit',
			arguments: { message: 'Add new file' },
		});
		expect(result.isError).toBe(false);
		expect(result.output).toContain('Add new file');

		// Verify the commit appears in log
		const log = await registry.execute({
			id: 'gl-verify',
			name: 'git_log',
			arguments: { count: 1 },
		});
		expect(log.output).toContain('Add new file');
	});

	it('git_commit fails with empty message', async () => {
		shell('echo "staged" > staged.txt && git add staged.txt', tmpDir);

		const result = await registry.execute({
			id: 'gc2',
			name: 'git_commit',
			arguments: { message: '' },
		});
		expect(result.isError).toBe(true);
	});

	// -----------------------------------------------------------------------
	// git_branch
	// -----------------------------------------------------------------------

	it('git_branch lists branches when no name given', async () => {
		const result = await registry.execute({
			id: 'gb1',
			name: 'git_branch',
			arguments: {},
		});
		expect(result.isError).toBe(false);
		// Default branch may be 'main' or 'master' depending on git config
		expect(result.output).toMatch(/main|master/);
	});

	it('git_branch creates a new branch', async () => {
		const result = await registry.execute({
			id: 'gb2',
			name: 'git_branch',
			arguments: { name: 'feature-x', create: true },
		});
		expect(result.isError).toBe(false);

		// Verify branch exists
		const list = await registry.execute({
			id: 'gb3',
			name: 'git_branch',
			arguments: {},
		});
		expect(list.output).toContain('feature-x');
	});

	it('git_branch switches to an existing branch', async () => {
		shell('git branch feature-y', tmpDir);

		const result = await registry.execute({
			id: 'gb4',
			name: 'git_branch',
			arguments: { name: 'feature-y' },
		});
		expect(result.isError).toBe(false);

		// Verify we switched
		const status = await registry.execute({
			id: 'gs-verify',
			name: 'git_status',
			arguments: {},
		});
		expect(status.output).toContain('feature-y');
	});
});
