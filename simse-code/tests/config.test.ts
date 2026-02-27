import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import {
	existsSync,
	mkdirSync,
	readFileSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import type {
	ACPFileConfig,
	EmbedFileConfig,
	MCPFileConfig,
	LibraryFileConfig,
	UserConfig,
} from '../config.js';
import { createCLIConfig } from '../config.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function writeJson(dir: string, file: string, data: unknown): void {
	writeFileSync(
		join(dir, file),
		`${JSON.stringify(data, null, '\t')}\n`,
		'utf-8',
	);
}

function makeACPConfig(overrides?: Partial<ACPFileConfig>): ACPFileConfig {
	return {
		servers: overrides?.servers ?? [
			{
				name: 'test-server',
				command: 'echo',
				args: ['test'],
				defaultAgent: 'test-agent',
			},
		],
		defaultServer: overrides?.defaultServer,
		defaultAgent: overrides?.defaultAgent,
	};
}

function makeMCPConfig(overrides?: Partial<MCPFileConfig>): MCPFileConfig {
	return {
		servers: overrides?.servers ?? [],
	};
}

// ---------------------------------------------------------------------------
// createCLIConfig
// ---------------------------------------------------------------------------

describe('createCLIConfig', () => {
	let testDir: string;

	beforeEach(() => {
		testDir = join(
			tmpdir(),
			`simse-test-${Date.now()}-${Math.random().toString(36).slice(2)}`,
		);
		mkdirSync(testDir, { recursive: true });
	});

	afterEach(() => {
		if (existsSync(testDir)) {
			rmSync(testDir, { recursive: true, force: true });
		}
	});

	// -- Basic behavior -------------------------------------------------------

	it('should return a frozen result', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		const result = createCLIConfig({ dataDir: testDir });
		expect(Object.isFrozen(result)).toBe(true);
		expect(result.config).toBeDefined();
		expect(result.logger).toBeDefined();
	});

	it('should throw when acp.json is missing', () => {
		expect(() => createCLIConfig({ dataDir: testDir })).toThrow(
			/No ACP config found/,
		);
	});

	it('should work with minimal acp.json', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		const { config } = createCLIConfig({ dataDir: testDir });
		expect(config.acp.servers).toHaveLength(1);
		expect(config.acp.servers[0].name).toBe('test-server');
	});

	// -- ACP config from file -------------------------------------------------

	it('should read ACP servers from acp.json', () => {
		writeJson(
			testDir,
			'acp.json',
			makeACPConfig({
				servers: [
					{
						name: 'my-server',
						command: 'my-cmd',
						args: ['--flag', 'value'],
						defaultAgent: 'agent-1',
					},
				],
			}),
		);

		const { config } = createCLIConfig({ dataDir: testDir });
		expect(config.acp.servers[0].name).toBe('my-server');
		expect(config.acp.servers[0].command).toBe('my-cmd');
		expect(config.acp.servers[0].args).toContain('--flag');
		expect(config.acp.servers[0].args).toContain('value');
	});

	// -- MCP config from file -------------------------------------------------

	it('should work without mcp.json (optional)', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		const { config } = createCLIConfig({ dataDir: testDir });
		expect(config.mcp.client.servers).toHaveLength(0);
	});

	it('should read MCP servers from mcp.json', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(
			testDir,
			'mcp.json',
			makeMCPConfig({
				servers: [
					{
						name: 'my-tool',
						transport: 'stdio',
						command: 'bunx',
						args: ['my-tool-pkg'],
					},
				],
			}),
		);

		const { config } = createCLIConfig({ dataDir: testDir });
		expect(config.mcp.client.servers).toHaveLength(1);
		expect(config.mcp.client.servers[0].name).toBe('my-tool');
	});

	// -- config.json precedence -----------------------------------------------

	it('should use values from config.json when no CLI flags are given', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(testDir, 'config.json', {
			defaultAgent: 'config-agent',
			logLevel: 'debug',
		} satisfies UserConfig);

		const { config } = createCLIConfig({ dataDir: testDir });
		expect(config.acp.defaultAgent).toBe('config-agent');
	});

	it('should let CLI flags override config.json values', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(testDir, 'config.json', {
			defaultAgent: 'from-config',
			logLevel: 'info',
		} satisfies UserConfig);

		const { config } = createCLIConfig({
			dataDir: testDir,
			defaultAgent: 'from-cli',
			logLevel: 'error',
		});
		expect(config.acp.defaultAgent).toBe('from-cli');
	});

	it('should not overwrite existing config.json on subsequent runs', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(testDir, 'config.json', {
			logLevel: 'error',
		} satisfies UserConfig);

		createCLIConfig({ dataDir: testDir });
		const raw = readFileSync(join(testDir, 'config.json'), 'utf-8');
		const parsed = JSON.parse(raw) as UserConfig;
		expect(parsed.logLevel).toBe('error');
	});

	// -- API-key-gated MCP servers --------------------------------------------

	it('should skip MCP servers with missing requiredEnv', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(
			testDir,
			'mcp.json',
			makeMCPConfig({
				servers: [
					{
						name: 'needs-key',
						transport: 'stdio',
						command: 'bunx',
						args: ['some-pkg'],
						env: { API_KEY: '' },
						requiredEnv: ['API_KEY'],
					},
					{
						name: 'no-key-needed',
						transport: 'stdio',
						command: 'bunx',
						args: ['other-pkg'],
					},
				],
			}),
		);

		const { config, skippedServers } = createCLIConfig({ dataDir: testDir });

		expect(skippedServers).toHaveLength(1);
		expect(skippedServers[0].name).toBe('needs-key');
		expect(skippedServers[0].missingEnv).toContain('API_KEY');

		const names = config.mcp.client.servers.map((s) => s.name);
		expect(names).not.toContain('needs-key');
		expect(names).toContain('no-key-needed');
	});

	it('should include MCP servers when requiredEnv values are provided', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(
			testDir,
			'mcp.json',
			makeMCPConfig({
				servers: [
					{
						name: 'has-key',
						transport: 'stdio',
						command: 'bunx',
						args: ['some-pkg'],
						env: { API_KEY: 'my-key-value' },
						requiredEnv: ['API_KEY'],
					},
				],
			}),
		);

		const { config, skippedServers } = createCLIConfig({ dataDir: testDir });

		expect(skippedServers).toHaveLength(0);
		expect(config.mcp.client.servers[0].name).toBe('has-key');
	});

	it('should report multiple missing requiredEnv vars', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(
			testDir,
			'mcp.json',
			makeMCPConfig({
				servers: [
					{
						name: 'multi-key',
						transport: 'stdio',
						command: 'bunx',
						args: ['pkg'],
						env: { KEY_A: '', KEY_B: '' },
						requiredEnv: ['KEY_A', 'KEY_B'],
					},
				],
			}),
		);

		const { skippedServers } = createCLIConfig({ dataDir: testDir });
		const entry = skippedServers.find((s) => s.name === 'multi-key');
		expect(entry).toBeDefined();
		expect(entry?.missingEnv).toContain('KEY_A');
		expect(entry?.missingEnv).toContain('KEY_B');
	});

	it('should not skip MCP servers without requiredEnv', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(
			testDir,
			'mcp.json',
			makeMCPConfig({
				servers: [
					{
						name: 'plain',
						transport: 'stdio',
						command: 'bunx',
						args: ['plain-pkg'],
						env: { SOME_KEY: 'value' },
					},
				],
			}),
		);

		const { config, skippedServers } = createCLIConfig({ dataDir: testDir });
		expect(skippedServers).toHaveLength(0);
		expect(config.mcp.client.servers[0].name).toBe('plain');
	});

	// -- CLIConfigOptions passthrough -----------------------------------------

	it('should pass clientName and clientVersion to MCP config', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		const { config } = createCLIConfig({
			dataDir: testDir,
			clientName: 'my-app',
			clientVersion: '2.0.0',
		});
		expect(config.mcp.client.clientName).toBe('my-app');
		expect(config.mcp.client.clientVersion).toBe('2.0.0');
	});

	it('should pass serverName and serverVersion to MCP config', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		const { config } = createCLIConfig({
			dataDir: testDir,
			serverName: 'my-server',
			serverVersion: '3.0.0',
		});
		expect(config.mcp.server.name).toBe('my-server');
		expect(config.mcp.server.version).toBe('3.0.0');
	});

	// -- memory.json config ---------------------------------------------------

	it('should work without memory.json (optional)', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		const result = createCLIConfig({ dataDir: testDir });
		expect(result.libraryConfig).toEqual({});
	});

	it('should read memory options from memory.json', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(testDir, 'embed.json', {
			embeddingModel: 'nomic-ai/nomic-embed-text-v1.5',
		} satisfies EmbedFileConfig);
		writeJson(testDir, 'memory.json', {
			enabled: true,
			similarityThreshold: 0.5,
			maxResults: 20,
		} satisfies LibraryFileConfig);

		const { config, libraryConfig, embedConfig } = createCLIConfig({
			dataDir: testDir,
		});
		expect(config.memory.enabled).toBe(true);
		expect(config.memory.embeddingAgent).toBe('nomic-ai/nomic-embed-text-v1.5');
		expect(embedConfig.embeddingModel).toBe('nomic-ai/nomic-embed-text-v1.5');
		expect(config.memory.similarityThreshold).toBe(0.5);
		expect(config.memory.maxResults).toBe(20);
		expect(libraryConfig.enabled).toBe(true);
	});

	it('should read embeddingServer from memory.json', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(testDir, 'memory.json', {
			embeddingServer: 'embed-server',
		} satisfies LibraryFileConfig);

		const result = createCLIConfig({ dataDir: testDir });
		expect(result.libraryConfig.embeddingServer).toBe('embed-server');
	});

	it('should read embeddingModel from memory.json', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(testDir, 'memory.json', {
			embeddingModel: 'nomic-embed-text',
		} satisfies LibraryFileConfig);

		const result = createCLIConfig({ dataDir: testDir });
		expect(result.libraryConfig.embeddingModel).toBe('nomic-embed-text');
	});

	it('should read storageFilename from memory.json', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(testDir, 'memory.json', {
			storageFilename: 'custom.simk',
		} satisfies LibraryFileConfig);

		const result = createCLIConfig({ dataDir: testDir });
		expect(result.libraryConfig.storageFilename).toBe('custom.simk');
	});

	it('should read vector store options from memory.json', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());
		writeJson(testDir, 'memory.json', {
			autoSave: true,
			duplicateThreshold: 0.95,
			duplicateBehavior: 'skip',
			flushIntervalMs: 10000,
			compressionLevel: 9,
			atomicWrite: false,
		} satisfies LibraryFileConfig);

		const { libraryConfig } = createCLIConfig({ dataDir: testDir });
		expect(libraryConfig.autoSave).toBe(true);
		expect(libraryConfig.duplicateThreshold).toBe(0.95);
		expect(libraryConfig.duplicateBehavior).toBe('skip');
		expect(libraryConfig.flushIntervalMs).toBe(10000);
		expect(libraryConfig.compressionLevel).toBe(9);
		expect(libraryConfig.atomicWrite).toBe(false);
	});

	it('should fall back to default model when embeddingModel not in embed.json', () => {
		writeJson(testDir, 'acp.json', makeACPConfig());

		const { config } = createCLIConfig({ dataDir: testDir });
		expect(config.memory.embeddingAgent).toBe('nomic-ai/nomic-embed-text-v1.5');
	});
});
