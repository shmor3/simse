import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type { FieldSchema } from '../features/config/settings-schema.js';
import {
	getAllConfigSchemas,
	getConfigSchema,
	loadConfigFile,
	resolveFieldOptions,
	saveConfigField,
} from '../features/config/settings-schema.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeTempDir(): string {
	const dir = join(
		tmpdir(),
		`simse-schema-test-${Date.now()}-${Math.random().toString(36).slice(2)}`,
	);
	mkdirSync(dir, { recursive: true });
	return dir;
}

// ---------------------------------------------------------------------------
// getConfigSchema
// ---------------------------------------------------------------------------

describe('getConfigSchema', () => {
	it('should return schema for config.json', () => {
		const schema = getConfigSchema('config.json');
		expect(schema).toBeDefined();
		expect(schema?.filename).toBe('config.json');
		expect(schema?.fields.length).toBeGreaterThan(0);
	});

	it('should return schema for embed.json', () => {
		const schema = getConfigSchema('embed.json');
		expect(schema).toBeDefined();
		expect(schema?.filename).toBe('embed.json');
	});

	it('should return schema for memory.json', () => {
		const schema = getConfigSchema('memory.json');
		expect(schema).toBeDefined();
		expect(schema?.filename).toBe('memory.json');
	});

	it('should return schema for acp.json', () => {
		const schema = getConfigSchema('acp.json');
		expect(schema).toBeDefined();
		expect(schema?.filename).toBe('acp.json');
	});

	it('should return undefined for mcp.json (excluded, no editable fields)', () => {
		const schema = getConfigSchema('mcp.json');
		expect(schema).toBeUndefined();
	});

	it('should return schema for summarize.json', () => {
		const schema = getConfigSchema('summarize.json');
		expect(schema).toBeDefined();
		expect(schema?.filename).toBe('summarize.json');
	});

	it('should return schema for settings.json (workspace)', () => {
		const schema = getConfigSchema('settings.json');
		expect(schema).toBeDefined();
		expect(schema?.filename).toBe('settings.json');
	});

	it('should return undefined for unknown files', () => {
		const schema = getConfigSchema('unknown.json');
		expect(schema).toBeUndefined();
	});

	it('should return undefined for empty string', () => {
		const schema = getConfigSchema('');
		expect(schema).toBeUndefined();
	});
});

// ---------------------------------------------------------------------------
// getAllConfigSchemas
// ---------------------------------------------------------------------------

describe('getAllConfigSchemas', () => {
	it('should return all schemas', () => {
		const schemas = getAllConfigSchemas();
		expect(schemas.length).toBeGreaterThanOrEqual(6);
	});

	it('should include config.json schema', () => {
		const schemas = getAllConfigSchemas();
		const found = schemas.find((s) => s.filename === 'config.json');
		expect(found).toBeDefined();
	});

	it('should include memory.json schema', () => {
		const schemas = getAllConfigSchemas();
		const found = schemas.find((s) => s.filename === 'memory.json');
		expect(found).toBeDefined();
	});

	it('should return readonly array', () => {
		const schemas = getAllConfigSchemas();
		expect(Array.isArray(schemas)).toBe(true);
	});
});

// ---------------------------------------------------------------------------
// Field type identification
// ---------------------------------------------------------------------------

describe('field types', () => {
	it('should identify string fields', () => {
		const schema = getConfigSchema('config.json');
		const defaultAgent = schema?.fields.find((f) => f.key === 'defaultAgent');
		expect(defaultAgent).toBeDefined();
		expect(defaultAgent?.type).toBe('string');
	});

	it('should identify enum fields with options', () => {
		const schema = getConfigSchema('config.json');
		const logLevel = schema?.fields.find((f) => f.key === 'logLevel');
		expect(logLevel).toBeDefined();
		expect(logLevel?.type).toBe('enum');
		expect(logLevel?.options).toBeDefined();
		expect(logLevel?.options?.length).toBeGreaterThan(0);
		expect(logLevel?.options).toContain('debug');
		expect(logLevel?.options).toContain('error');
	});

	it('should identify boolean fields', () => {
		const schema = getConfigSchema('memory.json');
		const enabled = schema?.fields.find((f) => f.key === 'enabled');
		expect(enabled).toBeDefined();
		expect(enabled?.type).toBe('boolean');
	});

	it('should identify number fields', () => {
		const schema = getConfigSchema('memory.json');
		const threshold = schema?.fields.find(
			(f) => f.key === 'similarityThreshold',
		);
		expect(threshold).toBeDefined();
		expect(threshold?.type).toBe('number');
	});

	it('should have descriptions for all fields', () => {
		const schemas = getAllConfigSchemas();
		for (const schema of schemas) {
			for (const field of schema.fields) {
				expect(field.description).toBeTruthy();
			}
		}
	});
});

// ---------------------------------------------------------------------------
// config.json schema specifics
// ---------------------------------------------------------------------------

describe('config.json schema', () => {
	it('should have logLevel with correct options', () => {
		const schema = getConfigSchema('config.json');
		const logLevel = schema?.fields.find((f) => f.key === 'logLevel');
		expect(logLevel?.type).toBe('enum');
		expect(logLevel?.options).toEqual([
			'debug',
			'info',
			'warn',
			'error',
			'none',
		]);
		expect(logLevel?.default).toBe('warn');
	});

	it('should have defaultAgent as string', () => {
		const schema = getConfigSchema('config.json');
		const field = schema?.fields.find((f) => f.key === 'defaultAgent');
		expect(field?.type).toBe('string');
	});

	it('should have perplexityApiKey as string', () => {
		const schema = getConfigSchema('config.json');
		const field = schema?.fields.find((f) => f.key === 'perplexityApiKey');
		expect(field?.type).toBe('string');
	});

	it('should have githubToken as string', () => {
		const schema = getConfigSchema('config.json');
		const field = schema?.fields.find((f) => f.key === 'githubToken');
		expect(field?.type).toBe('string');
	});
});

// ---------------------------------------------------------------------------
// memory.json schema specifics
// ---------------------------------------------------------------------------

describe('memory.json schema', () => {
	it('should have enabled with default true', () => {
		const schema = getConfigSchema('memory.json');
		const field = schema?.fields.find((f) => f.key === 'enabled');
		expect(field?.type).toBe('boolean');
		expect(field?.default).toBe(true);
	});

	it('should have similarityThreshold with default 0.7', () => {
		const schema = getConfigSchema('memory.json');
		const field = schema?.fields.find((f) => f.key === 'similarityThreshold');
		expect(field?.type).toBe('number');
		expect(field?.default).toBe(0.7);
	});

	it('should have maxResults with default 10', () => {
		const schema = getConfigSchema('memory.json');
		const field = schema?.fields.find((f) => f.key === 'maxResults');
		expect(field?.type).toBe('number');
		expect(field?.default).toBe(10);
	});

	it('should have autoSummarizeThreshold with default 20', () => {
		const schema = getConfigSchema('memory.json');
		const field = schema?.fields.find(
			(f) => f.key === 'autoSummarizeThreshold',
		);
		expect(field?.type).toBe('number');
		expect(field?.default).toBe(20);
	});

	it('should have duplicateThreshold with default 0', () => {
		const schema = getConfigSchema('memory.json');
		const field = schema?.fields.find((f) => f.key === 'duplicateThreshold');
		expect(field?.type).toBe('number');
		expect(field?.default).toBe(0);
	});

	it('should have duplicateBehavior as enum with default skip', () => {
		const schema = getConfigSchema('memory.json');
		const field = schema?.fields.find((f) => f.key === 'duplicateBehavior');
		expect(field?.type).toBe('enum');
		expect(field?.options).toEqual(['skip', 'warn', 'error']);
		expect(field?.default).toBe('skip');
	});
});

// ---------------------------------------------------------------------------
// embed.json schema specifics
// ---------------------------------------------------------------------------

describe('embed.json schema', () => {
	it('should have embeddingModel with default', () => {
		const schema = getConfigSchema('embed.json');
		const field = schema?.fields.find((f) => f.key === 'embeddingModel');
		expect(field?.type).toBe('string');
		expect(field?.default).toBe('nomic-ai/nomic-embed-text-v1.5');
	});

	it('should have dtype as enum', () => {
		const schema = getConfigSchema('embed.json');
		const field = schema?.fields.find((f) => f.key === 'dtype');
		expect(field?.type).toBe('enum');
		expect(field?.options).toEqual(['fp32', 'fp16', 'q8', 'q4']);
	});

	it('should have teiUrl as string', () => {
		const schema = getConfigSchema('embed.json');
		const field = schema?.fields.find((f) => f.key === 'teiUrl');
		expect(field?.type).toBe('string');
	});
});

// ---------------------------------------------------------------------------
// loadConfigFile
// ---------------------------------------------------------------------------

describe('loadConfigFile', () => {
	let tempDir: string;

	beforeEach(() => {
		tempDir = makeTempDir();
	});

	afterEach(() => {
		rmSync(tempDir, { recursive: true, force: true });
	});

	it('should read a JSON config file correctly', () => {
		const data = { logLevel: 'debug', defaultAgent: 'test-agent' };
		writeFileSync(join(tempDir, 'config.json'), JSON.stringify(data));

		const result = loadConfigFile(tempDir, 'config.json');
		expect(result).toEqual(data);
	});

	it('should return empty object for missing file', () => {
		const result = loadConfigFile(tempDir, 'nonexistent.json');
		expect(result).toEqual({});
	});

	it('should return empty object for invalid JSON', () => {
		writeFileSync(join(tempDir, 'bad.json'), 'not valid json {{{');

		const result = loadConfigFile(tempDir, 'bad.json');
		expect(result).toEqual({});
	});

	it('should handle nested values', () => {
		const data = { servers: [{ name: 'test' }], defaultServer: 'test' };
		writeFileSync(join(tempDir, 'acp.json'), JSON.stringify(data));

		const result = loadConfigFile(tempDir, 'acp.json');
		expect(result).toEqual(data);
	});
});

// ---------------------------------------------------------------------------
// saveConfigField
// ---------------------------------------------------------------------------

describe('saveConfigField', () => {
	let tempDir: string;

	beforeEach(() => {
		tempDir = makeTempDir();
	});

	afterEach(() => {
		rmSync(tempDir, { recursive: true, force: true });
	});

	it('should create file and set field if file does not exist', () => {
		saveConfigField(tempDir, 'config.json', 'logLevel', 'debug');

		const result = loadConfigFile(tempDir, 'config.json');
		expect(result.logLevel).toBe('debug');
	});

	it('should update existing field in existing file', () => {
		const initial = { logLevel: 'warn', defaultAgent: 'agent1' };
		writeFileSync(join(tempDir, 'config.json'), JSON.stringify(initial));

		saveConfigField(tempDir, 'config.json', 'logLevel', 'error');

		const result = loadConfigFile(tempDir, 'config.json');
		expect(result.logLevel).toBe('error');
		expect(result.defaultAgent).toBe('agent1');
	});

	it('should add new field to existing file', () => {
		const initial = { logLevel: 'warn' };
		writeFileSync(join(tempDir, 'config.json'), JSON.stringify(initial));

		saveConfigField(tempDir, 'config.json', 'defaultAgent', 'new-agent');

		const result = loadConfigFile(tempDir, 'config.json');
		expect(result.logLevel).toBe('warn');
		expect(result.defaultAgent).toBe('new-agent');
	});

	it('should handle boolean values', () => {
		saveConfigField(tempDir, 'memory.json', 'enabled', false);

		const result = loadConfigFile(tempDir, 'memory.json');
		expect(result.enabled).toBe(false);
	});

	it('should handle number values', () => {
		saveConfigField(tempDir, 'memory.json', 'similarityThreshold', 0.85);

		const result = loadConfigFile(tempDir, 'memory.json');
		expect(result.similarityThreshold).toBe(0.85);
	});

	it('should create parent directories if needed', () => {
		const nestedDir = join(tempDir, 'nested', 'dir');

		saveConfigField(nestedDir, 'config.json', 'logLevel', 'info');

		expect(existsSync(join(nestedDir, 'config.json'))).toBe(true);
		const result = loadConfigFile(nestedDir, 'config.json');
		expect(result.logLevel).toBe('info');
	});
});

// ---------------------------------------------------------------------------
// field presets and resolve
// ---------------------------------------------------------------------------

describe('field presets and resolve', () => {
	// Helper to get a field from a schema by filename and key
	function getField(filename: string, key: string): FieldSchema {
		const schema = getConfigSchema(filename);
		expect(schema).toBeDefined();
		const field = schema!.fields.find((f) => f.key === key);
		expect(field).toBeDefined();
		return field!;
	}

	// -- presets on number fields in memory.json --

	it('similarityThreshold in memory.json should have presets with common thresholds', () => {
		const field = getField('memory.json', 'similarityThreshold');
		expect(field.presets).toBeDefined();
		expect(field.presets).toContain('0.5');
		expect(field.presets).toContain('0.7');
	});

	it('maxResults in memory.json should have presets including 10', () => {
		const field = getField('memory.json', 'maxResults');
		expect(field.presets).toBeDefined();
		expect(field.presets).toContain('10');
	});

	it('autoSummarizeThreshold in memory.json should have presets including 0', () => {
		const field = getField('memory.json', 'autoSummarizeThreshold');
		expect(field.presets).toBeDefined();
		expect(field.presets).toContain('0');
	});

	it('duplicateThreshold in memory.json should have presets including 0', () => {
		const field = getField('memory.json', 'duplicateThreshold');
		expect(field.presets).toBeDefined();
		expect(field.presets).toContain('0');
	});

	// -- resolve on string fields --

	it('defaultServer in acp.json should have resolve acp-servers', () => {
		const field = getField('acp.json', 'defaultServer');
		expect(field.resolve).toBe('acp-servers');
	});

	it('defaultAgent in config.json should have resolve agents', () => {
		const field = getField('config.json', 'defaultAgent');
		expect(field.resolve).toBe('agents');
	});

	it('embeddingModel in embed.json should have resolve embedding-models', () => {
		const field = getField('embed.json', 'embeddingModel');
		expect(field.resolve).toBe('embedding-models');
	});

	it('server in summarize.json should have resolve acp-servers', () => {
		const field = getField('summarize.json', 'server');
		expect(field.resolve).toBe('acp-servers');
	});

	it('agent in summarize.json should have resolve agents', () => {
		const field = getField('summarize.json', 'agent');
		expect(field.resolve).toBe('agents');
	});

	it('defaultAgent in settings.json should have resolve agents', () => {
		const field = getField('settings.json', 'defaultAgent');
		expect(field.resolve).toBe('agents');
	});

	it('defaultServer in settings.json should have resolve acp-servers', () => {
		const field = getField('settings.json', 'defaultServer');
		expect(field.resolve).toBe('acp-servers');
	});
});

// ---------------------------------------------------------------------------
// resolveFieldOptions
// ---------------------------------------------------------------------------

describe('resolveFieldOptions', () => {
	let tempDir: string;

	beforeEach(() => {
		tempDir = makeTempDir();
	});

	afterEach(() => {
		rmSync(tempDir, { recursive: true, force: true });
	});

	it('should return acp server names from acp.json', () => {
		const acpData = {
			servers: [
				{ name: 'claude', command: 'bunx' },
				{ name: 'ollama', command: 'bun' },
			],
		};
		writeFileSync(join(tempDir, 'acp.json'), JSON.stringify(acpData));

		const options = resolveFieldOptions('acp-servers', tempDir, tempDir);
		expect(options).toContain('claude');
		expect(options).toContain('ollama');
		expect(options).toContain('(unset)');
		expect(options).toContain('Add new server...');
		expect(options[options.length - 1]).toBe('Add new server...');
	});

	it('should return empty + Add new server when acp.json missing', () => {
		const options = resolveFieldOptions('acp-servers', tempDir, tempDir);
		expect(options).toContain('(unset)');
		expect(options).toContain('Add new server...');
	});

	it('should return agent IDs from acp.json servers', () => {
		const acpData = {
			servers: [
				{ name: 'claude-server', command: 'bunx', defaultAgent: 'claude-agent' },
				{ name: 'ollama', command: 'bun' },
			],
		};
		writeFileSync(join(tempDir, 'acp.json'), JSON.stringify(acpData));

		const options = resolveFieldOptions('agents', tempDir, tempDir);
		expect(options).toContain('claude-agent');
		expect(options).toContain('ollama');
		expect(options).toContain('(unset)');
		expect(options).toContain('Custom value...');
	});

	it('should return agent IDs from .simse/agents/*.md files', () => {
		const agentsDir = join(tempDir, '.simse', 'agents');
		mkdirSync(agentsDir, { recursive: true });
		writeFileSync(join(agentsDir, 'researcher.md'), '# Researcher');
		writeFileSync(join(agentsDir, 'coder.md'), '# Coder');

		const options = resolveFieldOptions('agents', tempDir, tempDir);
		expect(options).toContain('researcher');
		expect(options).toContain('coder');
	});

	it('should return embedding model presets', () => {
		const options = resolveFieldOptions('embedding-models', tempDir, tempDir);
		expect(options).toContain('Snowflake/snowflake-arctic-embed-xs');
		expect(options).toContain('nomic-ai/nomic-embed-text-v1.5');
		expect(options).toContain('Snowflake/snowflake-arctic-embed-l');
		expect(options).toContain('(unset)');
		expect(options).toContain('Custom model...');
	});

	it('should deduplicate agent names from servers and agent files', () => {
		const acpData = {
			servers: [{ name: 'researcher', command: 'bunx' }],
		};
		writeFileSync(join(tempDir, 'acp.json'), JSON.stringify(acpData));

		const agentsDir = join(tempDir, '.simse', 'agents');
		mkdirSync(agentsDir, { recursive: true });
		writeFileSync(join(agentsDir, 'researcher.md'), '# Researcher');

		const options = resolveFieldOptions('agents', tempDir, tempDir);
		const count = options.filter((o) => o === 'researcher').length;
		expect(count).toBe(1);
	});
});
