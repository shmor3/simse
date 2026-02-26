import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { registerGitTools } from '../src/ai/tools/host/git.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';
import type { ToolRegistry } from '../src/ai/tools/types.js';
import { createSilentLogger } from './utils/mocks.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function shell(cmd: string, cwd: string): string {
	const result = Bun.spawnSync(['bash', '-c', cmd], {
		cwd,
		stdout: 'pipe',
		stderr: 'pipe',
	});
	if (result.exitCode !== 0) {
		const stderr = new TextDecoder().decode(result.stderr).trim();
		throw new Error(`Shell command failed: ${cmd}\n${stderr}`);
	}
	return new TextDecoder().decode(result.stdout).trim();
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

describe('git_add', () => {
	let registry: ToolRegistry;
	let tmpDir: string;

	beforeEach(() => {
		tmpDir = mkdtempSync(join(tmpdir(), 'simse-git-extra-'));
		initGitRepo(tmpDir);
		registry = createToolRegistry({ logger: createSilentLogger() });
		registerGitTools(registry, { workingDirectory: tmpDir });
	});

	afterEach(() => {
		rmSync(tmpDir, { recursive: true, force: true });
	});

	it('stages a specific file', async () => {
		writeFileSync(join(tmpDir, 'new-file.txt'), 'content');

		const result = await registry.execute({
			id: 'a1',
			name: 'git_add',
			arguments: { paths: 'new-file.txt' },
		});

		expect(result.isError).toBe(false);

		// Verify staged
		const status = shell('git status --porcelain', tmpDir);
		expect(status).toContain('A  new-file.txt');
	});

	it('stages all changes with all=true', async () => {
		writeFileSync(join(tmpDir, 'a.txt'), 'aaa');
		writeFileSync(join(tmpDir, 'b.txt'), 'bbb');

		const result = await registry.execute({
			id: 'a2',
			name: 'git_add',
			arguments: { all: true },
		});

		expect(result.isError).toBe(false);

		const status = shell('git status --porcelain', tmpDir);
		expect(status).toContain('a.txt');
		expect(status).toContain('b.txt');
	});

	it('errors when no paths and all is not true', async () => {
		const result = await registry.execute({
			id: 'a3',
			name: 'git_add',
			arguments: {},
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('provide paths');
	});
});

describe('git_stash', () => {
	let registry: ToolRegistry;
	let tmpDir: string;

	beforeEach(() => {
		tmpDir = mkdtempSync(join(tmpdir(), 'simse-git-stash-'));
		initGitRepo(tmpDir);
		registry = createToolRegistry({ logger: createSilentLogger() });
		registerGitTools(registry, { workingDirectory: tmpDir });
	});

	afterEach(() => {
		rmSync(tmpDir, { recursive: true, force: true });
	});

	it('saves and pops changes', async () => {
		writeFileSync(join(tmpDir, 'README.md'), 'modified');
		shell('git add README.md', tmpDir);

		// Stash save
		const saveResult = await registry.execute({
			id: 'st1',
			name: 'git_stash',
			arguments: { action: 'save', message: 'test stash' },
		});
		expect(saveResult.isError).toBe(false);

		// Working tree should be clean now
		const status = shell('git status --porcelain', tmpDir);
		expect(status).toBe('');

		// Stash pop
		const popResult = await registry.execute({
			id: 'st2',
			name: 'git_stash',
			arguments: { action: 'pop' },
		});
		expect(popResult.isError).toBe(false);
	});

	it('lists stashes', async () => {
		writeFileSync(join(tmpDir, 'README.md'), 'modified');
		shell('git add README.md && git stash -m "first stash"', tmpDir);

		const result = await registry.execute({
			id: 'st3',
			name: 'git_stash',
			arguments: { action: 'list' },
		});

		expect(result.isError).toBe(false);
		expect(result.output).toContain('first stash');
	});

	it('errors on unknown action', async () => {
		const result = await registry.execute({
			id: 'st4',
			name: 'git_stash',
			arguments: { action: 'invalid' },
		});

		expect(result.isError).toBe(true);
		expect(result.output).toContain('Unknown stash action');
	});
});

describe('git_push', () => {
	let registry: ToolRegistry;
	let tmpDir: string;

	beforeEach(() => {
		tmpDir = mkdtempSync(join(tmpdir(), 'simse-git-push-'));
		initGitRepo(tmpDir);
		registry = createToolRegistry({ logger: createSilentLogger() });
		registerGitTools(registry, { workingDirectory: tmpDir });
	});

	afterEach(() => {
		rmSync(tmpDir, { recursive: true, force: true });
	});

	it('errors when no remote exists', async () => {
		const result = await registry.execute({
			id: 'p1',
			name: 'git_push',
			arguments: {},
		});

		// No remote configured, should fail
		expect(result.isError).toBe(true);
	});
});

describe('git_pull', () => {
	let registry: ToolRegistry;
	let tmpDir: string;

	beforeEach(() => {
		tmpDir = mkdtempSync(join(tmpdir(), 'simse-git-pull-'));
		initGitRepo(tmpDir);
		registry = createToolRegistry({ logger: createSilentLogger() });
		registerGitTools(registry, { workingDirectory: tmpDir });
	});

	afterEach(() => {
		rmSync(tmpDir, { recursive: true, force: true });
	});

	it('errors when no remote exists', async () => {
		const result = await registry.execute({
			id: 'pl1',
			name: 'git_pull',
			arguments: {},
		});

		// No remote configured, should fail
		expect(result.isError).toBe(true);
	});
});

describe('git tool registration', () => {
	let registry: ToolRegistry;
	let tmpDir: string;

	beforeEach(() => {
		tmpDir = mkdtempSync(join(tmpdir(), 'simse-git-reg-'));
		initGitRepo(tmpDir);
		registry = createToolRegistry({ logger: createSilentLogger() });
		registerGitTools(registry, { workingDirectory: tmpDir });
	});

	afterEach(() => {
		rmSync(tmpDir, { recursive: true, force: true });
	});

	it('registers all git tools', () => {
		const defs = registry.getToolDefinitions();
		const names = defs.map((d) => d.name);

		expect(names).toContain('git_status');
		expect(names).toContain('git_diff');
		expect(names).toContain('git_log');
		expect(names).toContain('git_commit');
		expect(names).toContain('git_branch');
		expect(names).toContain('git_add');
		expect(names).toContain('git_stash');
		expect(names).toContain('git_push');
		expect(names).toContain('git_pull');
	});

	it('marks git_push as destructive', () => {
		const defs = registry.getToolDefinitions();
		const pushDef = defs.find((d) => d.name === 'git_push');
		expect(pushDef?.annotations?.destructive).toBe(true);
	});
});
