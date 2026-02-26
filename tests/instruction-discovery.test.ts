import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { discoverInstructions } from '../src/ai/prompts/instruction-discovery.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let tempDir: string;

beforeEach(() => {
	tempDir = join(
		tmpdir(),
		`simse-test-${Date.now()}-${Math.random().toString(36).slice(2)}`,
	);
	mkdirSync(tempDir, { recursive: true });
});

afterEach(() => {
	rmSync(tempDir, { recursive: true, force: true });
});

// ---------------------------------------------------------------------------
// discoverInstructions
// ---------------------------------------------------------------------------

describe('discoverInstructions', () => {
	it('finds CLAUDE.md in the root directory', async () => {
		writeFileSync(join(tempDir, 'CLAUDE.md'), '# Claude instructions');

		const results = await discoverInstructions({ rootDir: tempDir });

		expect(results).toHaveLength(1);
		expect(results[0].content).toBe('# Claude instructions');
		expect(results[0].path).toContain('CLAUDE.md');
	});

	it('finds AGENTS.md', async () => {
		writeFileSync(join(tempDir, 'AGENTS.md'), '# Agent config');

		const results = await discoverInstructions({ rootDir: tempDir });

		expect(results).toHaveLength(1);
		expect(results[0].content).toBe('# Agent config');
		expect(results[0].path).toContain('AGENTS.md');
	});

	it('finds .simse/instructions.md', async () => {
		mkdirSync(join(tempDir, '.simse'), { recursive: true });
		writeFileSync(
			join(tempDir, '.simse', 'instructions.md'),
			'# Simse instructions',
		);

		const results = await discoverInstructions({ rootDir: tempDir });

		expect(results).toHaveLength(1);
		expect(results[0].content).toBe('# Simse instructions');
		expect(results[0].path).toContain('instructions.md');
	});

	it('returns empty array when no files are found', async () => {
		const results = await discoverInstructions({ rootDir: tempDir });

		expect(results).toHaveLength(0);
		expect(results).toEqual([]);
	});

	it('supports custom patterns', async () => {
		writeFileSync(join(tempDir, 'CUSTOM.md'), '# Custom instructions');
		writeFileSync(join(tempDir, 'AGENTS.md'), '# Should be ignored');

		const results = await discoverInstructions({
			rootDir: tempDir,
			patterns: ['CUSTOM.md'],
		});

		expect(results).toHaveLength(1);
		expect(results[0].content).toBe('# Custom instructions');
	});

	it('discovers multiple files in pattern order', async () => {
		writeFileSync(join(tempDir, 'CLAUDE.md'), '# Claude');
		writeFileSync(join(tempDir, 'AGENTS.md'), '# Agents');
		mkdirSync(join(tempDir, '.simse'), { recursive: true });
		writeFileSync(join(tempDir, '.simse', 'instructions.md'), '# Instructions');

		const results = await discoverInstructions({ rootDir: tempDir });

		expect(results).toHaveLength(3);
		expect(results[0].content).toBe('# Claude');
		expect(results[1].content).toBe('# Agents');
		expect(results[2].content).toBe('# Instructions');
	});

	it('skips missing files silently', async () => {
		writeFileSync(join(tempDir, 'CLAUDE.md'), '# Claude');
		// AGENTS.md intentionally not created

		const results = await discoverInstructions({ rootDir: tempDir });

		expect(results).toHaveLength(1);
		expect(results[0].content).toBe('# Claude');
	});

	it('returns frozen results', async () => {
		writeFileSync(join(tempDir, 'CLAUDE.md'), '# Claude');

		const results = await discoverInstructions({ rootDir: tempDir });

		expect(Object.isFrozen(results)).toBe(true);
		expect(Object.isFrozen(results[0])).toBe(true);
	});
});
