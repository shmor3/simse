import { afterEach, describe, expect, it } from 'bun:test';
import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { type ACPBackend, createTempConfig } from './config.js';

describe('createTempConfig', () => {
	let cleanup: (() => Promise<void>) | undefined;

	afterEach(async () => {
		await cleanup?.();
	});

	it('creates a temp directory with config files', async () => {
		const result = await createTempConfig('none');
		cleanup = result.cleanup;

		expect(existsSync(result.dataDir)).toBe(true);
		expect(existsSync(join(result.dataDir, 'config.json'))).toBe(true);
		expect(existsSync(join(result.dataDir, 'memory.json'))).toBe(true);
		expect(existsSync(join(result.dataDir, 'embed.json'))).toBe(true);
	});

	it('creates no acp.json for "none" backend', async () => {
		const result = await createTempConfig('none');
		cleanup = result.cleanup;

		expect(existsSync(join(result.dataDir, 'acp.json'))).toBe(false);
	});

	it('creates acp.json for "claude" backend', async () => {
		const result = await createTempConfig('claude');
		cleanup = result.cleanup;

		const acpJson = JSON.parse(
			await readFile(join(result.dataDir, 'acp.json'), 'utf-8'),
		);
		expect(acpJson.servers[0].name).toBe('claude');
		expect(acpJson.servers[0].command).toBe('bunx');
		expect(acpJson.servers[0].args).toContain('claude-code-acp');
	});

	it('creates acp.json for "ollama" backend', async () => {
		const result = await createTempConfig('ollama');
		cleanup = result.cleanup;

		const acpJson = JSON.parse(
			await readFile(join(result.dataDir, 'acp.json'), 'utf-8'),
		);
		expect(acpJson.servers[0].name).toBe('ollama');
	});

	it('cleanup removes the temp directory', async () => {
		const result = await createTempConfig('none');
		const dir = result.dataDir;
		await result.cleanup();
		cleanup = undefined;

		expect(existsSync(dir)).toBe(false);
	});
});
