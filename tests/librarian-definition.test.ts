import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type {
	ArbitrationResult,
	LibrarianBid,
	LibrarianDefinition,
} from 'simse-vector';
import {
	loadAllDefinitions,
	loadDefinition,
	matchesTopic,
	saveDefinition,
	validateDefinition,
} from 'simse-vector';

describe('LibrarianDefinition types', () => {
	it('allows constructing a valid LibrarianDefinition', () => {
		const def: LibrarianDefinition = {
			name: 'code-patterns',
			description: 'Manages code pattern memories',
			purpose: 'I specialize in code patterns and architecture',
			topics: ['code/*', 'architecture/*'],
			permissions: { add: true, delete: true, reorganize: true },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
		};
		expect(def.name).toBe('code-patterns');
		expect(def.topics).toHaveLength(2);
	});

	it('allows LibrarianDefinition with ACP config', () => {
		const def: LibrarianDefinition = {
			name: 'test',
			description: 'Test librarian',
			purpose: 'Testing',
			topics: ['*'],
			permissions: { add: true, delete: false, reorganize: false },
			thresholds: { topicComplexity: 50, escalateAt: 100 },
			acp: {
				command: 'simse-engine',
				args: ['--mode', 'librarian'],
				agentId: 'test-agent',
			},
		};
		expect(def.acp?.command).toBe('simse-engine');
	});

	it('allows constructing a LibrarianBid', () => {
		const bid: LibrarianBid = {
			librarianName: 'code-patterns',
			argument: 'I already manage 15 volumes about React patterns',
			confidence: 0.85,
		};
		expect(bid.confidence).toBeGreaterThan(0);
	});

	it('allows constructing an ArbitrationResult', () => {
		const result: ArbitrationResult = {
			winner: 'code-patterns',
			reason: 'Best expertise match',
			bids: [
				{
					librarianName: 'code-patterns',
					argument: 'I manage code patterns',
					confidence: 0.9,
				},
			],
		};
		expect(result.bids).toHaveLength(1);
	});
});

// ---------------------------------------------------------------------------
// validateDefinition
// ---------------------------------------------------------------------------

const VALID_DEF: LibrarianDefinition = {
	name: 'code-patterns',
	description: 'Manages code pattern memories',
	purpose: 'I specialize in code patterns and architecture',
	topics: ['code/*', 'architecture/*'],
	permissions: { add: true, delete: true, reorganize: true },
	thresholds: { topicComplexity: 50, escalateAt: 100 },
};

describe('validateDefinition', () => {
	it('accepts a valid definition', () => {
		const result = validateDefinition(VALID_DEF);
		expect(result.valid).toBe(true);
		expect(result.errors).toHaveLength(0);
	});

	it('accepts a valid definition with acp config', () => {
		const result = validateDefinition({
			...VALID_DEF,
			acp: { command: 'simse-engine', args: ['--mode', 'librarian'] },
		});
		expect(result.valid).toBe(true);
		expect(result.errors).toHaveLength(0);
	});

	it('rejects non-object input', () => {
		const result = validateDefinition('not an object');
		expect(result.valid).toBe(false);
		expect(result.errors).toContain('input must be an object');
	});

	it('rejects missing fields', () => {
		const result = validateDefinition({});
		expect(result.valid).toBe(false);
		expect(result.errors.length).toBeGreaterThan(0);
		expect(result.errors.some((e) => e.includes('name'))).toBe(true);
		expect(result.errors.some((e) => e.includes('description'))).toBe(true);
		expect(result.errors.some((e) => e.includes('purpose'))).toBe(true);
		expect(result.errors.some((e) => e.includes('topics'))).toBe(true);
		expect(result.errors.some((e) => e.includes('permissions'))).toBe(true);
		expect(result.errors.some((e) => e.includes('thresholds'))).toBe(true);
	});

	it('rejects empty topics array', () => {
		const result = validateDefinition({ ...VALID_DEF, topics: [] });
		expect(result.valid).toBe(false);
		expect(result.errors.some((e) => e.includes('topics'))).toBe(true);
	});

	it('rejects invalid name format', () => {
		const result = validateDefinition({ ...VALID_DEF, name: 'Invalid Name!' });
		expect(result.valid).toBe(false);
		expect(result.errors.some((e) => e.includes('kebab-case'))).toBe(true);
	});

	it('rejects name starting with hyphen', () => {
		const result = validateDefinition({ ...VALID_DEF, name: '-bad' });
		expect(result.valid).toBe(false);
	});

	it('rejects non-boolean permissions', () => {
		const result = validateDefinition({
			...VALID_DEF,
			permissions: { add: 'yes', delete: true, reorganize: true },
		});
		expect(result.valid).toBe(false);
		expect(result.errors.some((e) => e.includes('permissions.add'))).toBe(true);
	});

	it('rejects non-positive thresholds', () => {
		const result = validateDefinition({
			...VALID_DEF,
			thresholds: { topicComplexity: 0, escalateAt: -1 },
		});
		expect(result.valid).toBe(false);
		expect(result.errors.some((e) => e.includes('topicComplexity'))).toBe(true);
		expect(result.errors.some((e) => e.includes('escalateAt'))).toBe(true);
	});

	it('rejects acp with empty command', () => {
		const result = validateDefinition({
			...VALID_DEF,
			acp: { command: '' },
		});
		expect(result.valid).toBe(false);
		expect(result.errors.some((e) => e.includes('acp.command'))).toBe(true);
	});
});

// ---------------------------------------------------------------------------
// matchesTopic
// ---------------------------------------------------------------------------

describe('matchesTopic', () => {
	it('wildcard * matches any single-level topic', () => {
		expect(matchesTopic(['*'], 'anything')).toBe(true);
	});

	it('glob pattern matches at one level', () => {
		expect(matchesTopic(['code/*'], 'code/react')).toBe(true);
	});

	it('glob pattern does not match deeper levels without **', () => {
		expect(matchesTopic(['code/*'], 'code/react/hooks')).toBe(false);
	});

	it('rejects non-matching topic', () => {
		expect(matchesTopic(['code/*'], 'design/figma')).toBe(false);
	});

	it('multi-pattern matches if any pattern matches', () => {
		expect(matchesTopic(['code/*', 'design/*'], 'design/figma')).toBe(true);
	});

	it('deep glob ** matches nested levels', () => {
		expect(matchesTopic(['code/**'], 'code/react/hooks')).toBe(true);
		expect(matchesTopic(['code/**'], 'code/react')).toBe(true);
	});

	it('exact pattern matches exactly', () => {
		expect(matchesTopic(['code/react'], 'code/react')).toBe(true);
		expect(matchesTopic(['code/react'], 'code/vue')).toBe(false);
	});
});

// ---------------------------------------------------------------------------
// saveDefinition / loadDefinition / loadAllDefinitions
// ---------------------------------------------------------------------------

describe('saveDefinition / loadDefinition', () => {
	let tmpDir: string;

	beforeEach(async () => {
		tmpDir = await mkdtemp(join(tmpdir(), 'simse-librarian-test-'));
	});

	afterEach(async () => {
		await rm(tmpDir, { recursive: true, force: true });
	});

	it('round-trips a definition through save and load', async () => {
		await saveDefinition(tmpDir, VALID_DEF);
		const loaded = await loadDefinition(tmpDir, VALID_DEF.name);
		expect(loaded).toBeDefined();
		expect(loaded!.name).toBe(VALID_DEF.name);
		expect(loaded!.description).toBe(VALID_DEF.description);
		expect(loaded!.purpose).toBe(VALID_DEF.purpose);
		expect(loaded!.topics).toEqual(VALID_DEF.topics);
		expect(loaded!.permissions).toEqual(VALID_DEF.permissions);
		expect(loaded!.thresholds).toEqual(VALID_DEF.thresholds);
	});

	it('returns undefined for non-existent definition', async () => {
		const loaded = await loadDefinition(tmpDir, 'does-not-exist');
		expect(loaded).toBeUndefined();
	});

	it('returns undefined for invalid JSON content', async () => {
		const { writeFile: wf } = await import('node:fs/promises');
		await wf(join(tmpDir, 'bad.json'), '{ not valid json !!!', 'utf-8');
		const loaded = await loadDefinition(tmpDir, 'bad');
		expect(loaded).toBeUndefined();
	});

	it('returns undefined for valid JSON that fails validation', async () => {
		const { writeFile: wf } = await import('node:fs/promises');
		await wf(
			join(tmpDir, 'invalid.json'),
			JSON.stringify({ name: 'INVALID' }),
			'utf-8',
		);
		const loaded = await loadDefinition(tmpDir, 'invalid');
		expect(loaded).toBeUndefined();
	});

	it('creates the directory if it does not exist', async () => {
		const nested = join(tmpDir, 'nested', 'dir');
		await saveDefinition(nested, VALID_DEF);
		const loaded = await loadDefinition(nested, VALID_DEF.name);
		expect(loaded).toBeDefined();
		expect(loaded!.name).toBe(VALID_DEF.name);
	});
});

describe('loadAllDefinitions', () => {
	let tmpDir: string;

	beforeEach(async () => {
		tmpDir = await mkdtemp(join(tmpdir(), 'simse-librarian-test-'));
	});

	afterEach(async () => {
		await rm(tmpDir, { recursive: true, force: true });
	});

	it('returns empty array for non-existent directory', async () => {
		const defs = await loadAllDefinitions(join(tmpDir, 'nope'));
		expect(defs).toEqual([]);
	});

	it('loads all valid definitions from directory', async () => {
		const def1: LibrarianDefinition = {
			...VALID_DEF,
			name: 'alpha',
		};
		const def2: LibrarianDefinition = {
			...VALID_DEF,
			name: 'beta',
		};
		await saveDefinition(tmpDir, def1);
		await saveDefinition(tmpDir, def2);

		const defs = await loadAllDefinitions(tmpDir);
		expect(defs).toHaveLength(2);
		const names = defs.map((d) => d.name).sort();
		expect(names).toEqual(['alpha', 'beta']);
	});

	it('skips invalid files in the directory', async () => {
		await saveDefinition(tmpDir, VALID_DEF);
		const { writeFile: wf } = await import('node:fs/promises');
		await wf(join(tmpDir, 'bad.json'), '{}', 'utf-8');

		const defs = await loadAllDefinitions(tmpDir);
		expect(defs).toHaveLength(1);
		expect(defs[0].name).toBe(VALID_DEF.name);
	});
});
