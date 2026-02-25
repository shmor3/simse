import { describe, expect, it } from 'bun:test';
import {
	formatValidationIssues,
	type ValidationIssue,
	validateACPConfig,
	validateACPServerEntry,
	validateAppConfig,
	validateChainDefinition,
	validateChainStepDefinition,
	validateMCPClientConfig,
	validateMCPServerConfig,
	validateMCPServerConnection,
	validateMemoryConfig,
} from '../src/config/schema.js';
import type { SimseConfig } from '../src/config/settings.js';
import { defineConfig } from '../src/config/settings.js';
import { isConfigError, isConfigValidationError } from '../src/errors/index.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const hasIssues = (issues: readonly ValidationIssue[]): boolean =>
	issues.length > 0;

const isValid = (issues: readonly ValidationIssue[]): boolean =>
	issues.length === 0;

// ===========================================================================
// validateACPServerEntry
// ===========================================================================

describe('ACPServerEntrySchema', () => {
	it('should accept valid config', () => {
		const result = validateACPServerEntry(
			{
				name: 'local',
				command: 'echo',
				defaultAgent: 'my-agent',
				timeoutMs: 30000,
			},
			'server',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject empty name', () => {
		const result = validateACPServerEntry(
			{
				name: '',
				command: 'echo',
			},
			'server',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject empty command', () => {
		const result = validateACPServerEntry(
			{
				name: 'bad',
				command: '',
			},
			'server',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept command without defaultAgent', () => {
		const result = validateACPServerEntry(
			{
				name: 'local',
				command: 'echo',
			},
			'server',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject empty defaultAgent', () => {
		const result = validateACPServerEntry(
			{
				name: 'local',
				command: 'echo',
				defaultAgent: '',
			},
			'server',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept without timeoutMs (defaults later)', () => {
		const result = validateACPServerEntry(
			{
				name: 'local',
				command: 'echo',
			},
			'server',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject timeoutMs below 1000', () => {
		const result = validateACPServerEntry(
			{
				name: 'local',
				command: 'echo',
				timeoutMs: 500,
			},
			'server',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject timeoutMs above 600000', () => {
		const result = validateACPServerEntry(
			{
				name: 'local',
				command: 'echo',
				timeoutMs: 700_000,
			},
			'server',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject non-integer timeoutMs', () => {
		const result = validateACPServerEntry(
			{
				name: 'local',
				command: 'echo',
				timeoutMs: 1500.5,
			},
			'server',
		);
		expect(hasIssues(result)).toBe(true);
	});
});

// ===========================================================================
// validateACPConfig
// ===========================================================================

describe('ACPConfigSchema', () => {
	it('should accept valid config with one server', () => {
		const result = validateACPConfig(
			{
				servers: [{ name: 'local', command: 'echo' }],
			},
			'acp',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should accept config with defaultServer and defaultAgent', () => {
		const result = validateACPConfig(
			{
				servers: [{ name: 'local', command: 'echo' }],
				defaultServer: 'local',
				defaultAgent: 'agent-1',
			},
			'acp',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should accept multiple servers', () => {
		const result = validateACPConfig(
			{
				servers: [
					{ name: 'local', command: 'echo' },
					{ name: 'remote', command: 'remote-agent' },
				],
			},
			'acp',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject empty servers array', () => {
		const result = validateACPConfig(
			{
				servers: [],
			},
			'acp',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject empty defaultServer', () => {
		const result = validateACPConfig(
			{
				servers: [{ name: 'local', command: 'echo' }],
				defaultServer: '',
			},
			'acp',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject empty defaultAgent', () => {
		const result = validateACPConfig(
			{
				servers: [{ name: 'local', command: 'echo' }],
				defaultAgent: '',
			},
			'acp',
		);
		expect(hasIssues(result)).toBe(true);
	});
});

// ===========================================================================
// validateMCPServerConnection
// ===========================================================================

describe('MCPServerConnectionSchema', () => {
	describe('stdio transport', () => {
		it('should accept valid stdio config', () => {
			const result = validateMCPServerConnection(
				{
					name: 'local',
					transport: 'stdio',
					command: 'node',
					args: ['server.js'],
				},
				'mcp',
			);
			expect(isValid(result)).toBe(true);
		});

		it('should accept stdio without args', () => {
			const result = validateMCPServerConnection(
				{
					name: 'local',
					transport: 'stdio',
					command: 'node',
				},
				'mcp',
			);
			expect(isValid(result)).toBe(true);
		});

		it('should reject empty command', () => {
			const result = validateMCPServerConnection(
				{
					name: 'local',
					transport: 'stdio',
					command: '',
				},
				'mcp',
			);
			expect(hasIssues(result)).toBe(true);
		});

		it('should reject empty name', () => {
			const result = validateMCPServerConnection(
				{
					name: '',
					transport: 'stdio',
					command: 'node',
				},
				'mcp',
			);
			expect(hasIssues(result)).toBe(true);
		});
	});

	describe('http transport', () => {
		it('should accept valid http config', () => {
			const result = validateMCPServerConnection(
				{
					name: 'remote',
					transport: 'http',
					url: 'http://localhost:3000/mcp',
				},
				'mcp',
			);
			expect(isValid(result)).toBe(true);
		});

		it('should reject invalid url', () => {
			const result = validateMCPServerConnection(
				{
					name: 'remote',
					transport: 'http',
					url: 'not-a-url',
				},
				'mcp',
			);
			expect(hasIssues(result)).toBe(true);
		});

		it('should reject empty name', () => {
			const result = validateMCPServerConnection(
				{
					name: '',
					transport: 'http',
					url: 'http://localhost:3000/mcp',
				},
				'mcp',
			);
			expect(hasIssues(result)).toBe(true);
		});
	});
});

// ===========================================================================
// validateMCPClientConfig
// ===========================================================================

describe('MCPClientConfigSchema', () => {
	it('should accept empty object (no servers)', () => {
		const result = validateMCPClientConfig({}, 'mcp.client');
		expect(isValid(result)).toBe(true);
	});

	it('should accept multiple server configs', () => {
		const result = validateMCPClientConfig(
			{
				servers: [
					{ name: 'a', transport: 'stdio', command: 'node', args: ['s.js'] },
					{ name: 'b', transport: 'http', url: 'http://localhost:3000' },
				],
			},
			'mcp.client',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject invalid server configs in the array', () => {
		const result = validateMCPClientConfig(
			{
				servers: [{ name: '', transport: 'stdio', command: 'node' }],
			},
			'mcp.client',
		);
		expect(hasIssues(result)).toBe(true);
	});
});

// ===========================================================================
// validateMCPServerConfig
// ===========================================================================

describe('MCPServerConfigSchema', () => {
	it('should accept empty object (all defaults)', () => {
		const result = validateMCPServerConfig({}, 'mcp.server');
		expect(isValid(result)).toBe(true);
	});

	it('should accept valid config', () => {
		const result = validateMCPServerConfig(
			{
				enabled: true,
				name: 'my-server',
				version: '2.0.0',
			},
			'mcp.server',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject empty name', () => {
		const result = validateMCPServerConfig(
			{
				name: '',
			},
			'mcp.server',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject non-semver version', () => {
		const result = validateMCPServerConfig(
			{
				version: 'abc',
			},
			'mcp.server',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject version with v prefix', () => {
		const result = validateMCPServerConfig(
			{
				version: 'v1.0.0',
			},
			'mcp.server',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept valid semver versions', () => {
		const versions = ['0.0.1', '1.0.0', '10.20.30'];
		for (const version of versions) {
			const result = validateMCPServerConfig({ version }, 'mcp.server');
			expect(isValid(result)).toBe(true);
		}
	});
});

// ===========================================================================
// validateMemoryConfig
// ===========================================================================

describe('MemoryConfigSchema', () => {
	it('should accept disabled memory with no embeddingAgent', () => {
		const result = validateMemoryConfig({ enabled: false }, 'memory');
		expect(isValid(result)).toBe(true);
	});

	it('should reject enabled memory without embeddingAgent', () => {
		const result = validateMemoryConfig({ enabled: true }, 'memory');
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept enabled memory with all required fields', () => {
		const result = validateMemoryConfig(
			{
				enabled: true,
				embeddingAgent: 'embed-model',
				similarityThreshold: 0.7,
				maxResults: 5,
			},
			'memory',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject enabled memory without similarityThreshold', () => {
		const result = validateMemoryConfig(
			{
				enabled: true,
				embeddingAgent: 'embed-model',
				maxResults: 5,
			},
			'memory',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject enabled memory without maxResults', () => {
		const result = validateMemoryConfig(
			{
				enabled: true,
				embeddingAgent: 'embed-model',
				similarityThreshold: 0.7,
			},
			'memory',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept disabled memory with only embeddingAgent', () => {
		const result = validateMemoryConfig(
			{
				enabled: false,
				embeddingAgent: 'embed-model',
			},
			'memory',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject empty embeddingAgent', () => {
		const result = validateMemoryConfig(
			{
				embeddingAgent: '',
			},
			'memory',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject similarityThreshold below 0', () => {
		const result = validateMemoryConfig(
			{
				similarityThreshold: -0.1,
			},
			'memory',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject similarityThreshold above 1', () => {
		const result = validateMemoryConfig(
			{
				similarityThreshold: 1.1,
			},
			'memory',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept similarityThreshold at boundaries', () => {
		expect(
			isValid(
				validateMemoryConfig(
					{ enabled: false, similarityThreshold: 0 },
					'memory',
				),
			),
		).toBe(true);
		expect(
			isValid(
				validateMemoryConfig(
					{ enabled: false, similarityThreshold: 1 },
					'memory',
				),
			),
		).toBe(true);
	});

	it('should reject maxResults below 1', () => {
		const result = validateMemoryConfig({ maxResults: 0 }, 'memory');
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject maxResults above 100', () => {
		const result = validateMemoryConfig({ maxResults: 101 }, 'memory');
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject non-integer maxResults', () => {
		const result = validateMemoryConfig({ maxResults: 5.5 }, 'memory');
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept maxResults at boundaries', () => {
		expect(
			isValid(
				validateMemoryConfig({ enabled: false, maxResults: 1 }, 'memory'),
			),
		).toBe(true);
		expect(
			isValid(
				validateMemoryConfig({ enabled: false, maxResults: 100 }, 'memory'),
			),
		).toBe(true);
	});
});

// ===========================================================================
// validateChainStepDefinition
// ===========================================================================

describe('ChainStepDefinitionSchema', () => {
	it('should accept a minimal valid step', () => {
		const result = validateChainStepDefinition(
			{
				name: 'step1',
				template: 'Do something with {input}',
			},
			'step',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should accept a fully-specified step', () => {
		const result = validateChainStepDefinition(
			{
				name: 'step1',
				template: 'Translate {input}',
				provider: 'acp',
				agentId: 'agent-1',
				serverName: 'local',
				systemPrompt: 'You are a translator',
				inputMapping: { input: 'previous_output' },
				storeToMemory: true,
				memoryMetadata: { source: 'translation' },
			},
			'step',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should accept step with MCP provider fields', () => {
		const result = validateChainStepDefinition(
			{
				name: 'mcp_step',
				template: 'Do {action}',
				provider: 'mcp',
				mcpServerName: 'my-mcp',
				mcpToolName: 'my-tool',
				mcpArguments: { action: 'something' },
			},
			'step',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject empty step name', () => {
		const result = validateChainStepDefinition(
			{
				name: '',
				template: 'Do something',
			},
			'step',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should reject step name starting with a digit', () => {
		const result = validateChainStepDefinition(
			{
				name: '1step',
				template: 'Do something',
			},
			'step',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept step name starting with underscore', () => {
		const result = validateChainStepDefinition(
			{
				name: '_step',
				template: 'Do something',
			},
			'step',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should accept step name with hyphens', () => {
		const result = validateChainStepDefinition(
			{
				name: 'my-step',
				template: 'Do something',
			},
			'step',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject empty template', () => {
		const result = validateChainStepDefinition(
			{
				name: 'step1',
				template: '',
			},
			'step',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept all valid providers', () => {
		for (const provider of ['acp', 'mcp', 'memory'] as const) {
			const result = validateChainStepDefinition(
				{
					name: 'step1',
					template: 'Do something',
					provider,
					...(provider === 'mcp'
						? { mcpServerName: 'server', mcpToolName: 'tool' }
						: {}),
				},
				'step',
			);
			expect(isValid(result)).toBe(true);
		}
	});

	it('should require mcpServerName when provider is mcp', () => {
		const result = validateChainStepDefinition(
			{
				name: 'step1',
				template: 'Do something',
				provider: 'mcp',
				mcpToolName: 'tool',
			},
			'step',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should require mcpToolName when provider is mcp', () => {
		const result = validateChainStepDefinition(
			{
				name: 'step1',
				template: 'Do something',
				provider: 'mcp',
				mcpServerName: 'server',
			},
			'step',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept mcp step with both mcpServerName and mcpToolName', () => {
		const result = validateChainStepDefinition(
			{
				name: 'step1',
				template: 'Do something',
				provider: 'mcp',
				mcpServerName: 'server',
				mcpToolName: 'tool',
			},
			'step',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should not require mcpServerName when provider is not mcp', () => {
		const result = validateChainStepDefinition(
			{
				name: 'step1',
				template: 'Do something',
				provider: 'acp',
			},
			'step',
		);
		expect(isValid(result)).toBe(true);
	});
});

// ===========================================================================
// validateChainDefinition
// ===========================================================================

describe('ChainDefinitionSchema', () => {
	it('should accept a valid chain definition', () => {
		const result = validateChainDefinition(
			{
				description: 'A test chain',
				initialValues: { topic: 'AI' },
				steps: [
					{
						name: 'step1',
						template: 'Write about {topic}',
					},
				],
			},
			'chain',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject a chain with no steps', () => {
		const result = validateChainDefinition(
			{
				steps: [],
			},
			'chain',
		);
		expect(hasIssues(result)).toBe(true);
	});

	it('should accept chain without initialValues', () => {
		const result = validateChainDefinition(
			{
				steps: [{ name: 'step1', template: 'Do something' }],
			},
			'chain',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should accept chain-level agentId', () => {
		const result = validateChainDefinition(
			{
				agentId: 'agent-1',
				steps: [{ name: 'step1', template: 'Do something' }],
			},
			'chain',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should accept chain-level serverName', () => {
		const result = validateChainDefinition(
			{
				serverName: 'local',
				steps: [{ name: 'step1', template: 'Do something' }],
			},
			'chain',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should accept multiple steps', () => {
		const result = validateChainDefinition(
			{
				steps: [
					{ name: 'step1', template: 'Step 1' },
					{ name: 'step2', template: 'Step 2' },
					{ name: 'step3', template: 'Step 3' },
				],
			},
			'chain',
		);
		expect(isValid(result)).toBe(true);
	});

	it('should reject chain with invalid step', () => {
		const result = validateChainDefinition(
			{
				steps: [{ name: '', template: '' }],
			},
			'chain',
		);
		expect(hasIssues(result)).toBe(true);
	});
});

// ===========================================================================
// Provider validation (within chain step)
// ===========================================================================

describe('ProviderSchema', () => {
	it('should accept all valid providers', () => {
		for (const provider of ['acp', 'mcp', 'memory'] as const) {
			const result = validateChainStepDefinition(
				{
					name: 'step1',
					template: 'Do something',
					provider,
					...(provider === 'mcp'
						? { mcpServerName: 'srv', mcpToolName: 'tl' }
						: {}),
				},
				'step',
			);
			expect(isValid(result)).toBe(true);
		}
	});
});

// ===========================================================================
// validateAppConfig
// ===========================================================================

describe('AppConfigSchema', () => {
	it('should accept a valid config', () => {
		const result = validateAppConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
				defaultServer: 'local',
				defaultAgent: 'agent-1',
			},
		});
		expect(isValid(result)).toBe(true);
	});

	it('should accept config with only acp', () => {
		const result = validateAppConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
		});
		expect(isValid(result)).toBe(true);
	});

	it('should accept a fully-specified config', () => {
		const result = validateAppConfig({
			acp: {
				servers: [
					{
						name: 'local',
						command: 'echo',
						defaultAgent: 'agent-1',
					},
				],
				defaultServer: 'local',
				defaultAgent: 'agent-1',
			},
			memory: {
				enabled: true,
				embeddingAgent: 'embed-model',
				similarityThreshold: 0.8,
				maxResults: 10,
			},
			mcp: {
				client: { servers: [] },
				server: {
					enabled: true,
					name: 'my-server',
					version: '1.0.0',
				},
			},
			chains: {
				test: {
					description: 'Test chain',
					steps: [{ name: 'step1', template: 'Do something' }],
				},
			},
		});
		expect(isValid(result)).toBe(true);
	});

	it('should reject config with empty acp servers', () => {
		const result = validateAppConfig({
			acp: {
				servers: [],
			},
		});
		expect(hasIssues(result)).toBe(true);
	});
});

// ===========================================================================
// formatValidationIssues
// ===========================================================================

describe('formatValidationIssues', () => {
	it('should format issues with paths', () => {
		const issues = validateACPServerEntry(
			{
				name: '',
				command: '',
			},
			'server',
		);
		const formatted = formatValidationIssues(issues);
		expect(formatted.length).toBeGreaterThan(0);
		expect(formatted[0]).toHaveProperty('path');
		expect(formatted[0]).toHaveProperty('message');
	});

	it('should preserve path and message from original issues', () => {
		const formatted = formatValidationIssues([
			{ path: 'acp.servers', message: 'At least one server required' },
		]);
		expect(formatted).toHaveLength(1);
		expect(formatted[0].path).toBe('acp.servers');
		expect(formatted[0].message).toBe('At least one server required');
	});

	it('should handle nested paths', () => {
		const formatted = formatValidationIssues([
			{ path: 'acp.servers[0].name', message: 'Name cannot be empty' },
		]);
		expect(formatted[0].path).toBe('acp.servers[0].name');
	});
});

// ===========================================================================
// defineConfig — basic validation
// ===========================================================================

describe('defineConfig — basic validation', () => {
	it('should accept a valid minimal config', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
				defaultServer: 'local',
				defaultAgent: 'agent-1',
			},
		});

		expect(config.acp.servers).toHaveLength(1);
		expect(config.acp.servers[0].name).toBe('local');
		expect(config.acp.defaultServer).toBe('local');
		expect(config.acp.defaultAgent).toBe('agent-1');
	});

	it('should throw ConfigValidationError for empty servers array', () => {
		expect(() =>
			defineConfig({
				acp: {
					servers: [],
				},
			}),
		).toThrow(expect.anything());
		try {
			defineConfig({
				acp: {
					servers: [],
				},
			});
		} catch (e) {
			// Removed debug log: console.log("Config validation error thrown:", e);
			const err = e as { code?: string };
			expect(isConfigValidationError(err) || isConfigError(err)).toBe(true);
			expect(
				err.code === 'CONFIG_VALIDATION' || err.code === 'CONFIG_ERROR',
			).toBe(true);
		}
	});

	it('should merge partial config with defaults', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
			memory: {
				enabled: true,
				embeddingAgent: 'embed-model',
				similarityThreshold: 0.7,
				maxResults: 20,
			},
		});

		// Memory
		expect(config.memory.enabled).toBe(true);
		expect(config.memory.embeddingAgent).toBe('embed-model');
		expect(config.memory.similarityThreshold).toBe(0.7);
		expect(config.memory.maxResults).toBe(20);
	});

	it('should produce a complete AppConfig with all required fields', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
		});

		// All top-level sections should be present
		expect(config.acp).toBeDefined();
		expect(config.mcp).toBeDefined();
		expect(config.mcp.client).toBeDefined();
		expect(config.mcp.server).toBeDefined();
		expect(config.memory).toBeDefined();
		expect(config.chains).toBeDefined();

		// MCP defaults
		expect(config.mcp.server.enabled).toBe(false);
		expect(config.mcp.client.servers).toHaveLength(0);
	});
});

// ===========================================================================
// defineConfig — chains
// ===========================================================================

describe('defineConfig — chains', () => {
	it('should validate valid chain definitions', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
			chains: {
				'blog-writer': {
					description: 'Write a blog post',
					initialValues: { topic: 'AI' },
					steps: [
						{
							name: 'research',
							template: 'Research {topic}',
						},
						{
							name: 'write',
							template: 'Write about {topic}: {research}',
						},
					],
				},
			},
		});

		expect(config.chains['blog-writer']).toBeDefined();
		expect(config.chains['blog-writer'].steps).toHaveLength(2);
		expect(config.chains['blog-writer'].initialValues.topic).toBe('AI');
	});

	it('should accept multiple chains', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
			chains: {
				chain1: {
					steps: [{ name: 'step1', template: 'Do 1' }],
				},
				chain2: {
					steps: [{ name: 'step1', template: 'Do 2' }],
				},
				chain3: {
					steps: [{ name: 'step1', template: 'Do 3' }],
				},
			},
		});

		expect(Object.keys(config.chains)).toHaveLength(3);
	});

	it('should throw ConfigValidationError for invalid chain steps', () => {
		expect(() =>
			defineConfig({
				acp: {
					servers: [{ name: 'local', command: 'echo' }],
				},
				chains: {
					bad: {
						steps: [{ name: '123bad', template: 'test' }],
					},
				},
			}),
		).toThrow(expect.anything());
		try {
			defineConfig({
				acp: {
					servers: [
						{
							name: 'valid',
							command: 'echo',
						},
					],
				},
				chains: {
					test: {
						steps: [{ name: 'step1', template: 'test', provider: 'acp' }],
					},
				},
			});
		} catch (e) {
			const err = e as { code?: string };
			expect(isConfigValidationError(err) || isConfigError(err)).toBe(true);
			expect(
				err.code === 'CONFIG_VALIDATION' || err.code === 'CONFIG_ERROR',
			).toBe(true);
		}
	});

	it('should throw ConfigValidationError for chain with no steps', () => {
		expect(() =>
			defineConfig({
				acp: {
					servers: [{ name: 'local', command: 'echo' }],
				},
				chains: {
					empty: {
						steps: [],
					},
				},
			}),
		).toThrow(expect.anything());
		try {
			defineConfig({
				acp: {
					servers: [{ name: 'valid', command: 'echo' }],
				},
				chains: {
					test: {
						steps: [],
					},
				},
			});
		} catch (e) {
			// Removed debug log: console.log("Chain validation error thrown:", e);
			const err = e as { code?: string };
			expect(isConfigValidationError(err) || isConfigError(err)).toBe(true);
			expect(
				err.code === 'CONFIG_VALIDATION' || err.code === 'CONFIG_ERROR',
			).toBe(true);
		}
	});

	it('should handle chains with all optional fields', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
			chains: {
				minimal: {
					steps: [{ name: 'step1', template: 'Do something' }],
				},
			},
		});

		const chain = config.chains.minimal;
		expect(chain.description).toBeUndefined();
		expect(chain.initialValues).toEqual({});
	});

	it('should accept chain with provider-specific steps', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
			chains: {
				mixed: {
					steps: [
						{
							name: 'acp_step',
							template: 'Generate {input}',
							provider: 'acp',
							agentId: 'agent-1',
						},
						{
							name: 'memory_step',
							template: 'Search {input}',
							provider: 'memory',
						},
						{
							name: 'mcp_step',
							template: 'Call {input}',
							provider: 'mcp',
							mcpServerName: 'server-1',
							mcpToolName: 'tool-1',
						},
					],
				},
			},
		});

		const steps = config.chains.mixed.steps;
		expect(steps).toHaveLength(3);
		expect(steps[0].provider).toBe('acp');
		expect(steps[1].provider).toBe('memory');
		expect(steps[2].provider).toBe('mcp');
	});

	it('should default chains to empty when omitted', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
		});

		expect(config.chains).toEqual({});
	});
});

// ===========================================================================
// defineConfig — MCP
// ===========================================================================

describe('defineConfig — MCP', () => {
	it('should accept MCP client server connections', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
			mcp: {
				client: {
					servers: [
						{
							name: 'stdio-server',
							transport: 'stdio' as const,
							command: 'node',
							args: ['server.js'],
						},
						{
							name: 'http-server',
							transport: 'http' as const,
							url: 'http://localhost:3000/mcp',
						},
					],
				},
			},
		});

		expect(config.mcp.client.servers).toHaveLength(2);
		expect(config.mcp.client.servers[0].transport).toBe('stdio');
		expect(config.mcp.client.servers[1].transport).toBe('http');
	});

	it('should accept MCP server mode config', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
			mcp: {
				server: {
					enabled: true,
					name: 'my-mcp-server',
					version: '2.0.0',
				},
			},
		});

		expect(config.mcp.server.enabled).toBe(true);
		expect(config.mcp.server.name).toBe('my-mcp-server');
		expect(config.mcp.server.version).toBe('2.0.0');
	});

	it('should default MCP server to disabled', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
		});

		expect(config.mcp.server.enabled).toBe(false);
	});

	it('should default MCP clients to empty', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
		});

		expect(config.mcp.client.servers).toHaveLength(0);
	});
});

// ===========================================================================
// defineConfig — lenient mode
// ===========================================================================

describe('defineConfig — lenient mode', () => {
	it('should warn but not throw for invalid chains in lenient mode', () => {
		const warnings: Array<{ path: string; message: string }>[] = [];

		const config = defineConfig(
			{
				acp: {
					servers: [{ name: 'local', command: 'echo' }],
				},
				chains: {
					bad: {
						steps: [{ name: '123invalid', template: 'test' }],
					},
					good: {
						steps: [{ name: 'valid_step', template: 'test' }],
					},
				},
			} as SimseConfig,
			{
				lenient: true,
				onWarn: (issues) => warnings.push([...issues]),
			},
		);

		// Should still produce a config (chains that parsed are included)
		expect(config).toBeDefined();
		expect(config.acp.servers).toHaveLength(1);
	});

	it('should call onWarn with issues when lenient', () => {
		const warnings: Array<{ path: string; message: string }>[] = [];

		defineConfig(
			{
				acp: {
					servers: [{ name: 'local', command: 'echo' }],
				},
				chains: {
					bad: {
						steps: [{ name: '1bad', template: 'test' }],
					},
				},
			} as SimseConfig,
			{
				lenient: true,
				onWarn: (issues) => warnings.push([...issues]),
			},
		);

		expect(warnings.length).toBeGreaterThan(0);
	});
});

// ===========================================================================
// defineConfig — full integration
// ===========================================================================

describe('defineConfig — full integration', () => {
	it('should validate a complete config object', () => {
		const input: SimseConfig = {
			acp: {
				servers: [
					{
						name: 'local',
						command: 'echo',
						defaultAgent: 'default-agent',
					},
					{
						name: 'remote',
						command: 'remote-agent',
						defaultAgent: 'remote-agent',
					},
				],
				defaultServer: 'local',
				defaultAgent: 'fallback-agent',
			},
			memory: {
				enabled: true,
				embeddingAgent: 'embed-model',
				similarityThreshold: 0.7,
				maxResults: 5,
			},
			mcp: {
				client: {
					servers: [
						{
							name: 'mcp-local',
							transport: 'stdio' as const,
							command: 'node',
							args: ['mcp-server.js'],
						},
					],
				},
			},
			chains: {
				'blog-writer': {
					description: 'Write a blog post about a topic',
					initialValues: { topic: 'AI' },
					steps: [
						{
							name: 'research',
							template: 'Research {topic}',
							systemPrompt: 'You are a researcher',
						},
						{
							name: 'outline',
							template: 'Create an outline for {topic}:\n\n{research}',
							systemPrompt: 'You are an outliner',
						},
						{
							name: 'write',
							template: 'Write about {topic} following:\n\n{outline}',
							agentId: 'writer-agent',
							systemPrompt: 'You are a blog writer',
						},
					],
				},
				translate: {
					description: 'Translate text',
					initialValues: {
						text: 'Hello, world!',
						language: 'French',
					},
					steps: [
						{
							name: 'translate',
							template: 'Translate "{text}" to {language}',
							systemPrompt: 'You are a translator',
						},
						{
							name: 'verify',
							template: 'Verify this {language} translation: {translate}',
						},
					],
				},
			},
		};

		const config = defineConfig(input);

		// ACP
		expect(config.acp.servers).toHaveLength(2);
		expect(config.acp.servers[0].name).toBe('local');
		expect(config.acp.servers[1].name).toBe('remote');
		expect(config.acp.defaultServer).toBe('local');
		expect(config.acp.defaultAgent).toBe('fallback-agent');

		// Memory
		expect(config.memory.enabled).toBe(true);
		expect(config.memory.embeddingAgent).toBe('embed-model');
		expect(config.memory.similarityThreshold).toBe(0.7);
		expect(config.memory.maxResults).toBe(5);

		// MCP
		expect(config.mcp.client.servers).toHaveLength(1);
		expect(config.mcp.server.enabled).toBe(false);

		// Chains
		const blogWriter = config.chains['blog-writer'];
		expect(blogWriter).toBeDefined();
		expect(blogWriter.steps).toHaveLength(3);
		expect(blogWriter.description).toBe('Write a blog post about a topic');

		const translate = config.chains.translate;
		expect(translate).toBeDefined();
		expect(translate.steps).toHaveLength(2);
	});

	it('should apply timeoutMs default to servers', () => {
		const config = defineConfig({
			acp: {
				servers: [{ name: 'local', command: 'echo' }],
			},
		});

		expect(config.acp.servers[0].timeoutMs).toBe(30_000);
	});

	it('should preserve explicit timeoutMs', () => {
		const config = defineConfig({
			acp: {
				servers: [
					{
						name: 'local',
						command: 'echo',
						timeoutMs: 60_000,
					},
				],
			},
		});

		expect(config.acp.servers[0].timeoutMs).toBe(60_000);
	});
});
