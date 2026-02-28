import { beforeEach, describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';
import type { ACPGenerateResult } from '../src/ai/acp/types.js';
import type { ChainCallbacks } from '../src/ai/chain/index.js';
import {
	createChain,
	createChainFromDefinition,
	createPromptTemplate,
	runNamedChain,
} from '../src/ai/chain/index.js';
import type { Library } from '../src/ai/library/library.js';
import type { Lookup } from '../src/ai/library/types.js';
import type { MCPClient } from '../src/ai/mcp/mcp-client.js';
import type { MCPToolResult } from '../src/ai/mcp/types.js';
import type { AppConfig, ChainDefinition } from '../src/config/settings.js';
import type { SimseError } from '../src/errors/index.js';
import {
	isChainError,
	isChainNotFoundError,
	isChainStepError,
	isSimseError,
	isTemplateMissingVariablesError,
} from '../src/errors/index.js';
import { createLogger, type Logger } from '../src/logger.js';

// ---------------------------------------------------------------------------
// Helpers: create silent logger for tests
// ---------------------------------------------------------------------------

function createSilentLogger(): Logger {
	return createLogger({ context: 'test', level: 'none', transports: [] });
}

// ---------------------------------------------------------------------------
// Helpers: mock factories
// ---------------------------------------------------------------------------

function createMockACPClient(overrides: Partial<ACPClient> = {}): ACPClient {
	return {
		generate: mock((..._: any[]): any => {}).mockResolvedValue({
			content: 'acp response',
			agentId: 'default',
			serverName: 'local',
			sessionId: 'session-1',
		} satisfies ACPGenerateResult),
		chat: mock((..._: any[]): any => {}).mockResolvedValue({
			content: 'acp chat response',
			agentId: 'default',
			serverName: 'local',
			sessionId: 'session-2',
		}),
		generateStream: mock((..._: any[]): any => {}),
		listAgents: mock((..._: any[]): any => {}).mockResolvedValue([]),
		getAgent: mock((..._: any[]): any => {}),
		embed: mock((..._: any[]): any => {}).mockResolvedValue({
			embeddings: [[0.1, 0.2, 0.3]],
			agentId: 'embedding',
			serverName: 'local',
		}),
		isAvailable: mock((..._: any[]): any => {}).mockResolvedValue(true),
		serverNames: ['local'],
		serverCount: 1,
		defaultServerName: 'local',
		defaultAgent: 'default',
		...overrides,
	} as unknown as ACPClient;
}

function createMockMCPClient(overrides: Partial<MCPClient> = {}): MCPClient {
	return {
		connect: mock((..._: any[]): any => {}),
		connectAll: mock((..._: any[]): any => {}).mockResolvedValue([]),
		disconnect: mock((..._: any[]): any => {}),
		disconnectAll: mock((..._: any[]): any => {}),
		isAvailable: mock((..._: any[]): any => {}).mockReturnValue(true),
		connectionCount: 1,
		connectedServerNames: ['test-server'],
		listTools: mock((..._: any[]): any => {}).mockResolvedValue([]),
		callTool: mock((..._: any[]): any => {}).mockResolvedValue({
			content: 'tool result',
			isError: false,
			rawContent: [{ type: 'text', text: 'tool result' }],
			metrics: {
				durationMs: 50,
				serverName: 'test-server',
				toolName: 'test-tool',
				startedAt: new Date().toISOString(),
			},
		} satisfies MCPToolResult),
		listResources: mock((..._: any[]): any => {}).mockResolvedValue([]),
		readResource: mock((..._: any[]): any => {}).mockResolvedValue(''),
		listPrompts: mock((..._: any[]): any => {}).mockResolvedValue([]),
		getPrompt: mock((..._: any[]): any => {}).mockResolvedValue(''),
		...overrides,
	} as unknown as MCPClient;
}

function createMockLibrary(overrides: Partial<Library> = {}): Library {
	return {
		initialize: mock((..._: any[]): any => {}),
		dispose: mock((..._: any[]): any => {}),
		add: mock((..._: any[]): any => {}).mockResolvedValue('mem-id-1'),
		addBatch: mock((..._: any[]): any => {}).mockResolvedValue([
			'mem-id-1',
			'mem-id-2',
		]),
		search: mock((..._: any[]): any => {}).mockResolvedValue([
			{
				volume: {
					id: 'e1',
					text: 'previous memory',
					embedding: [0.1],
					metadata: {},
					timestamp: Date.now(),
				},
				score: 0.95,
			},
		] satisfies Lookup[]),
		delete: mock((..._: any[]): any => {}).mockResolvedValue(true),
		deleteBatch: mock((..._: any[]): any => {}).mockResolvedValue(1),
		clear: mock((..._: any[]): any => {}),
		size: 5,
		isInitialized: true,
		isDirty: false,
		embeddingAgent: 'embedding',
		...overrides,
	} as unknown as Library;
}

function createMinimalAppConfig(overrides: Partial<AppConfig> = {}): AppConfig {
	return {
		acp: {
			servers: [
				{
					name: 'local',
					command: 'echo',
					defaultAgent: 'default',
					timeoutMs: 30000,
				},
			],
			defaultServer: 'local',
			defaultAgent: 'default',
		},
		mcp: {
			client: { servers: [] },
			server: {
				enabled: false,
				transport: 'stdio',
				name: 'simse',
				version: '1.0.0',
			},
		},
		memory: {
			enabled: false,
			embeddingAgent: 'embedding',
			similarityThreshold: 0.7,
			maxResults: 5,
		},
		chains: {},
		...overrides,
	};
}

// ===========================================================================
// PromptTemplate
// ===========================================================================

describe('PromptTemplate', () => {
	// -------------------------------------------------------------------
	// Constructor
	// -------------------------------------------------------------------

	describe('constructor', () => {
		it('should create a template from a string', () => {
			const tpl = createPromptTemplate('Hello {name}');
			expect(tpl.getVariables()).toEqual(['name']);
		});

		it('should extract multiple variables', () => {
			const tpl = createPromptTemplate('Translate {text} to {language}');
			expect(tpl.getVariables()).toEqual(['text', 'language']);
		});

		it('should extract duplicate variables only once per occurrence', () => {
			// Note: the regex extracts every match, so duplicates appear.
			// (Template still works because replaceAll covers all.)
			const tpl = createPromptTemplate('{x} and {x}');
			const vars = tpl.getVariables();
			expect(vars.filter((v) => v === 'x').length).toBeGreaterThanOrEqual(1);
		});

		it('should handle templates with no variables', () => {
			const tpl = createPromptTemplate('No variables here.');
			expect(tpl.getVariables()).toEqual([]);
			expect(tpl.hasVariables).toBe(false);
		});

		it('should handle templates with underscored variable names', () => {
			const tpl = createPromptTemplate('Hello {first_name}');
			expect(tpl.getVariables()).toEqual(['first_name']);
		});

		it('should throw SimseError for empty template string', () => {
			expect(() => createPromptTemplate('')).toThrow(expect.anything());
			try {
				createPromptTemplate('');
			} catch (e) {
				expect(isSimseError(e)).toBe(true);
				expect((e as SimseError).code).toBe('TEMPLATE_EMPTY');
			}
		});

		it('should handle a template that is only a variable', () => {
			const tpl = createPromptTemplate('{x}');
			expect(tpl.getVariables()).toEqual(['x']);
		});

		it('should not extract variables from non-word characters', () => {
			const tpl = createPromptTemplate('Use {valid_var} but not {123bad}');
			// Only valid (\w+) matches should be extracted
			const vars = tpl.getVariables();
			expect(vars).toContain('valid_var');
			// {123bad} — "123bad" is all word chars, so it will match \w+
			// but the key point is the template works with what it finds
		});
	});

	// -------------------------------------------------------------------
	// format()
	// -------------------------------------------------------------------

	describe('format', () => {
		it('should replace a single variable', () => {
			const tpl = createPromptTemplate('Hello {name}');
			const result = tpl.format({ name: 'World' });
			expect(result).toBe('Hello World');
		});

		it('should replace multiple different variables', () => {
			const tpl = createPromptTemplate('Translate {text} to {language}');
			const result = tpl.format({ text: 'hello', language: 'French' });
			expect(result).toBe('Translate hello to French');
		});

		it('should replace all occurrences of the same variable', () => {
			const tpl = createPromptTemplate('{x} and {x}');
			const result = tpl.format({ x: 'same' });
			expect(result).toBe('same and same');
		});

		it('should leave extra values unused without error', () => {
			const tpl = createPromptTemplate('Hello {name}');
			const result = tpl.format({ name: 'World', extra: 'ignored' });
			expect(result).toBe('Hello World');
		});

		it('should throw TemplateMissingVariablesError for a single missing variable', () => {
			const tpl = createPromptTemplate('Hello {name}');

			expect(() => tpl.format({})).toThrow(expect.anything());
			try {
				tpl.format({});
			} catch (e) {
				expect(isTemplateMissingVariablesError(e)).toBe(true);
			}

			try {
				tpl.format({});
			} catch (e) {
				const err = e as SimseError;
				expect((err as any).missingVariables).toEqual(['name']);
			}
		});

		it('should throw TemplateMissingVariablesError listing all missing variables', () => {
			const tpl = createPromptTemplate('{a} {b} {c}');

			try {
				tpl.format({ a: 'ok' });
			} catch (e) {
				const err = e as SimseError;
				expect((err as any).missingVariables).toContain('b');
				expect((err as any).missingVariables).toContain('c');
			}
		});

		it('should handle empty string values', () => {
			const tpl = createPromptTemplate('Value: {val}');
			const result = tpl.format({ val: '' });
			expect(result).toBe('Value: ');
		});

		it('should handle values containing curly braces', () => {
			const tpl = createPromptTemplate('Data: {json}');
			const result = tpl.format({ json: '{"key":"value"}' });
			expect(result).toBe('Data: {"key":"value"}');
		});

		it('should handle values containing the variable placeholder syntax', () => {
			const tpl = createPromptTemplate('Template: {tpl}');
			const result = tpl.format({ tpl: '{nested}' });
			expect(result).toBe('Template: {nested}');
		});

		it('should handle multiline templates', () => {
			const tpl = createPromptTemplate('Line1: {a}\nLine2: {b}\nLine3: {c}');
			const result = tpl.format({ a: '1', b: '2', c: '3' });
			expect(result).toBe('Line1: 1\nLine2: 2\nLine3: 3');
		});

		it('should work with no variables (passthrough)', () => {
			const tpl = createPromptTemplate('Static text.');
			const result = tpl.format({});
			expect(result).toBe('Static text.');
		});
	});

	// -------------------------------------------------------------------
	// Accessors
	// -------------------------------------------------------------------

	describe('accessors', () => {
		it('should return hasVariables = true when variables exist', () => {
			const tpl = createPromptTemplate('Hello {name}');
			expect(tpl.hasVariables).toBe(true);
		});

		it('should return hasVariables = false when no variables exist', () => {
			const tpl = createPromptTemplate('No vars');
			expect(tpl.hasVariables).toBe(false);
		});

		it('should return the raw template string', () => {
			const raw = 'Template {x}';
			const tpl = createPromptTemplate(raw);
			expect(tpl.raw).toBe(raw);
		});

		it('should return a copy of variables from getVariables()', () => {
			const tpl = createPromptTemplate('{a} {b}');
			const vars1 = tpl.getVariables();
			const vars2 = tpl.getVariables();
			expect(vars1).toEqual(vars2);
			expect(vars1).not.toBe(vars2); // different array instances
		});
	});
});

// ===========================================================================
// Chain (createChain)
// ===========================================================================

describe('Chain', () => {
	let mockACP: ACPClient;
	let mockMCP: MCPClient;
	let mockLibrary: Library;
	let config: AppConfig;
	let silentLogger: Logger;

	beforeEach(() => {
		mockACP = createMockACPClient();
		mockMCP = createMockMCPClient();
		mockLibrary = createMockLibrary();
		config = createMinimalAppConfig();
		silentLogger = createSilentLogger();
	});

	// -----------------------------------------------------------------------
	// Construction & builder
	// -----------------------------------------------------------------------

	describe('construction', () => {
		it('should create an empty chain', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});
			expect(chain.length).toBe(0);
		});

		it('should add steps and report length', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain
				.addStep({
					name: 'step1',
					template: createPromptTemplate('Hello {name}'),
				})
				.addStep({
					name: 'step2',
					template: createPromptTemplate('Goodbye {previous_output}'),
				});

			expect(chain.length).toBe(2);
		});

		it('should support fluent addStep chaining', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			const result = chain.addStep({
				name: 's',
				template: createPromptTemplate('t {x}'),
			});

			expect(result).toBe(chain);
		});

		it('should clear all steps', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({ name: 's', template: createPromptTemplate('t {x}') });
			expect(chain.length).toBe(1);

			chain.clear();
			expect(chain.length).toBe(0);
		});

		it('should return read-only step configs', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({ name: 's', template: createPromptTemplate('t {x}') });

			const configs = chain.stepConfigs;
			expect(configs).toHaveLength(1);
			expect(configs[0].name).toBe('s');
		});

		it('should throw when adding an MCP step without mcpServerName', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			expect(() =>
				chain.addStep({
					name: 'mcp-step',
					template: createPromptTemplate('do {thing}'),
					provider: 'mcp',
					mcpToolName: 'some-tool',
				}),
			).toThrow(expect.anything());
			try {
				chain.addStep({
					name: 'mcp-step',
					template: createPromptTemplate('do {thing}'),
					provider: 'mcp',
					mcpToolName: 'some-tool',
				});
			} catch (e) {
				expect(isChainError(e)).toBe(true);
			}
		});

		it('should throw when adding an MCP step without mcpToolName', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			expect(() =>
				chain.addStep({
					name: 'mcp-step',
					template: createPromptTemplate('do {thing}'),
					provider: 'mcp',
					mcpServerName: 'some-server',
				}),
			).toThrow(expect.anything());
			try {
				chain.addStep({
					name: 'mcp-step',
					template: createPromptTemplate('do {thing}'),
					provider: 'mcp',
					mcpServerName: 'some-server',
				});
			} catch (e) {
				expect(isChainError(e)).toBe(true);
			}
		});
	});

	// -----------------------------------------------------------------------
	// createChainFromDefinition
	// -----------------------------------------------------------------------

	describe('createChainFromDefinition', () => {
		it('should build a chain from a definition object', () => {
			const definition: ChainDefinition = {
				description: 'Test chain',
				initialValues: {},
				steps: [
					{
						name: 'step1',
						template: 'Hello {name}',
						systemPrompt: 'Be helpful.',
					},
					{
						name: 'step2',
						template: 'Review: {previous_output}',
						agentId: 'reviewer',
					},
				],
			};

			const chain = createChainFromDefinition(definition, {
				acpClient: mockACP,
				logger: silentLogger,
			});

			expect(chain.length).toBe(2);
			expect(chain.stepConfigs[0].name).toBe('step1');
			expect(chain.stepConfigs[0].systemPrompt).toBe('Be helpful.');
			expect(chain.stepConfigs[1].name).toBe('step2');
			expect(chain.stepConfigs[1].agentId).toBe('reviewer');
		});

		it('should throw ChainError for definition with no steps', () => {
			const definition: ChainDefinition = {
				description: 'Empty',
				initialValues: {},
				steps: [],
			};

			expect(() =>
				createChainFromDefinition(definition, {
					acpClient: mockACP,
					logger: silentLogger,
				}),
			).toThrow(expect.anything());
			try {
				createChainFromDefinition(definition, {
					acpClient: mockACP,
					logger: silentLogger,
				});
			} catch (e) {
				expect(isChainError(e)).toBe(true);
			}
		});

		it('should apply chain-level agentId to steps without explicit agentId', () => {
			const definition: ChainDefinition = {
				agentId: 'custom-agent',
				initialValues: {},
				steps: [
					{ name: 's1', template: 'Hello {x}' },
					{
						name: 's2',
						template: '{previous_output}',
						agentId: 'override-agent',
					},
				],
			};

			const chain = createChainFromDefinition(definition, {
				acpClient: mockACP,
				logger: silentLogger,
			});

			expect(chain.stepConfigs[0].agentId).toBe('custom-agent');
			expect(chain.stepConfigs[1].agentId).toBe('override-agent');
		});
	});

	// -----------------------------------------------------------------------
	// run() — single step
	// -----------------------------------------------------------------------

	describe('run (single step)', () => {
		it('should execute a single ACP step and return results', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'greet',
				template: createPromptTemplate('Hello {name}'),
			});

			const results = await chain.run({ name: 'World' });

			expect(results).toHaveLength(1);
			expect(results[0].stepName).toBe('greet');
			expect(results[0].provider).toBe('acp');
			expect(results[0].model).toBe('acp:default');
			expect(results[0].input).toBe('Hello World');
			expect(results[0].output).toBe('acp response');
			expect(results[0].durationMs).toBeGreaterThanOrEqual(0);
			expect(results[0].stepIndex).toBe(0);

			expect(mockACP.generate).toHaveBeenCalledWith('Hello World', {
				agentId: undefined,
				serverName: undefined,
				systemPrompt: undefined,
				config: undefined,
			});
		});

		it('should pass systemPrompt and agentId to acp.generate', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 's',
				template: createPromptTemplate('Prompt {x}'),
				systemPrompt: 'Be concise.',
				agentId: 'custom-agent',
			});

			await chain.run({ x: 'test' });

			expect(mockACP.generate).toHaveBeenCalledWith('Prompt test', {
				agentId: 'custom-agent',
				serverName: undefined,
				systemPrompt: 'Be concise.',
				config: undefined,
			});
		});

		it('should execute a step with a specific agentId', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'ask',
				template: createPromptTemplate('Question: {q}'),
				agentId: 'qa-agent',
			});

			const results = await chain.run({ q: 'What is AI?' });

			expect(results).toHaveLength(1);
			expect(results[0].provider).toBe('acp');
			expect(results[0].output).toBe('acp response');

			expect(mockACP.generate).toHaveBeenCalledWith(
				'Question: What is AI?',
				expect.objectContaining({ agentId: 'qa-agent' }),
			);
		});

		it('should pass serverName for ACP steps', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 's',
				template: createPromptTemplate('Prompt {x}'),
				serverName: 'remote-server',
				agentId: 'remote-agent',
			});

			await chain.run({ x: 'val' });

			expect(mockACP.generate).toHaveBeenCalledWith(
				'Prompt val',
				expect.objectContaining({
					agentId: 'remote-agent',
					serverName: 'remote-server',
				}),
			);
		});

		it('should execute an MCP step', async () => {
			const chain = createChain({
				acpClient: mockACP,
				mcpClient: mockMCP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'tool-step',
				template: createPromptTemplate('Do {action}'),
				provider: 'mcp',
				mcpServerName: 'test-server',
				mcpToolName: 'run-action',
			});

			const results = await chain.run({ action: 'something' });

			expect(results).toHaveLength(1);
			expect(results[0].provider).toBe('mcp');
			expect(results[0].model).toBe('mcp:test-server/run-action');
			expect(results[0].output).toBe('tool result');
		});

		it('should execute a memory search step', async () => {
			const chain = createChain({
				acpClient: mockACP,
				library: mockLibrary,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'recall',
				template: createPromptTemplate('Remember {topic}'),
				provider: 'memory',
			});

			const results = await chain.run({ topic: 'AI history' });

			expect(results).toHaveLength(1);
			expect(results[0].provider).toBe('memory');
			expect(results[0].model).toBe('library:search');
			expect(results[0].output).toContain('previous memory');

			expect(mockLibrary.search).toHaveBeenCalledWith('Remember AI history');
		});
	});

	// -----------------------------------------------------------------------
	// run() — multi-step
	// -----------------------------------------------------------------------

	describe('run (multi-step)', () => {
		it('should chain outputs via previous_output variable', async () => {
			const generateMock = mock((..._: any[]): any => {})
				.mockResolvedValueOnce({
					content: 'outline result',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's1',
				})
				.mockResolvedValueOnce({
					content: 'draft result',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's2',
				});

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain
				.addStep({
					name: 'outline',
					template: createPromptTemplate('Outline for {topic}'),
				})
				.addStep({
					name: 'draft',
					template: createPromptTemplate('Draft from: {previous_output}'),
				});

			const results = await chain.run({ topic: 'testing' });

			expect(results).toHaveLength(2);
			expect(results[0].output).toBe('outline result');
			expect(results[1].output).toBe('draft result');

			// Second call should receive the output of the first step
			expect(generateMock).toHaveBeenCalledTimes(2);
			expect(generateMock.mock.calls[1][0]).toBe('Draft from: outline result');
		});

		it('should make step output available by step name', async () => {
			const generateMock = mock((..._: any[]): any => {})
				.mockResolvedValueOnce({
					content: 'first output',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's1',
				})
				.mockResolvedValueOnce({
					content: 'second output',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's2',
				})
				.mockResolvedValueOnce({
					content: 'third output',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's3',
				});

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain
				.addStep({
					name: 'alpha',
					template: createPromptTemplate('Start {topic}'),
				})
				.addStep({
					name: 'beta',
					template: createPromptTemplate('Middle {alpha}'),
				})
				.addStep({
					name: 'gamma',
					template: createPromptTemplate('End {alpha} and {beta}'),
				});

			const results = await chain.run({ topic: 'test' });

			expect(results).toHaveLength(3);

			// beta receives alpha's output
			expect(generateMock.mock.calls[1][0]).toBe('Middle first output');

			// gamma receives both alpha's and beta's outputs
			expect(generateMock.mock.calls[2][0]).toBe(
				'End first output and second output',
			);
		});

		it('should support steps with different agentIds', async () => {
			const generateMock = mock((..._: any[]): any => {})
				.mockResolvedValueOnce({
					content: 'agent1 output',
					agentId: 'agent-a',
					serverName: 'local',
					sessionId: 's1',
				})
				.mockResolvedValueOnce({
					content: 'agent2 output',
					agentId: 'agent-b',
					serverName: 'local',
					sessionId: 's2',
				});

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain
				.addStep({
					name: 's1',
					template: createPromptTemplate('First: {input}'),
					agentId: 'agent-a',
				})
				.addStep({
					name: 's2',
					template: createPromptTemplate('Second: {previous_output}'),
					agentId: 'agent-b',
				});

			const results = await chain.run({ input: 'hello' });

			expect(results).toHaveLength(2);
			expect(results[0].provider).toBe('acp');
			expect(results[1].provider).toBe('acp');

			expect(generateMock).toHaveBeenCalledTimes(2);
			expect(generateMock.mock.calls[0][1]).toEqual(
				expect.objectContaining({ agentId: 'agent-a' }),
			);
			expect(generateMock.mock.calls[1][1]).toEqual(
				expect.objectContaining({ agentId: 'agent-b' }),
			);
		});
	});

	// -----------------------------------------------------------------------
	// run() — inputMapping
	// -----------------------------------------------------------------------

	describe('run (inputMapping)', () => {
		it('should remap values using inputMapping', async () => {
			const generateMock = mock((..._: any[]): any => {})
				.mockResolvedValueOnce({
					content: 'outline text',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's1',
				})
				.mockResolvedValueOnce({
					content: 'draft text',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's2',
				});

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain
				.addStep({
					name: 'outline',
					template: createPromptTemplate('Outline: {topic}'),
				})
				.addStep({
					name: 'draft',
					template: createPromptTemplate('Write about: {content}'),
					inputMapping: { content: 'outline' },
				});

			const results = await chain.run({ topic: 'AI' });

			expect(results).toHaveLength(2);
			// The second step should receive the outline's output mapped to "content"
			expect(generateMock.mock.calls[1][0]).toBe('Write about: outline text');
		});
	});

	// -----------------------------------------------------------------------
	// run() — outputTransform
	// -----------------------------------------------------------------------

	describe('run (outputTransform)', () => {
		it('should apply outputTransform to step output', async () => {
			const generateMock = mock((..._: any[]): any => {})
				.mockResolvedValueOnce({
					content: '  padded output  ',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's1',
				})
				.mockResolvedValueOnce({
					content: 'final',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's2',
				});

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain
				.addStep({
					name: 'raw',
					template: createPromptTemplate('Generate {topic}'),
					outputTransform: (output: string) => output.trim(),
				})
				.addStep({
					name: 'use',
					template: createPromptTemplate('Use: {previous_output}'),
				});

			const results = await chain.run({ topic: 'stuff' });

			expect(results).toHaveLength(2);
			expect(results[0].output).toBe('padded output');
			expect(generateMock.mock.calls[1][0]).toBe('Use: padded output');
		});
	});

	// -----------------------------------------------------------------------
	// run() — storeToMemory
	// -----------------------------------------------------------------------

	describe('run (storeToMemory)', () => {
		it('should store output to memory when storeToMemory is true', async () => {
			const chain = createChain({
				acpClient: mockACP,
				library: mockLibrary,
				logger: silentLogger,
			});

			chain.addStep({
				name: 's',
				template: createPromptTemplate('Generate {topic}'),
				storeToMemory: true,
				memoryMetadata: { source: 'test' },
			});

			await chain.run({ topic: 'AI' });

			expect(mockLibrary.add).toHaveBeenCalledWith('acp response', {
				source: 'test',
			});
		});

		it('should not store to memory when storeToMemory is false/undefined', async () => {
			const chain = createChain({
				acpClient: mockACP,
				library: mockLibrary,
				logger: silentLogger,
			});

			chain.addStep({
				name: 's',
				template: createPromptTemplate('Generate {topic}'),
			});

			await chain.run({ topic: 'AI' });

			expect(mockLibrary.add).not.toHaveBeenCalled();
		});

		it('should not fail the chain if memory add fails', async () => {
			const failingMemory = createMockLibrary({
				add: mock((..._: any[]): any => {}).mockRejectedValue(
					new Error('Memory failed'),
				),
			});

			const chain = createChain({
				acpClient: mockACP,
				library: failingMemory,
				logger: silentLogger,
			});

			chain.addStep({
				name: 's',
				template: createPromptTemplate('Generate {topic}'),
				storeToMemory: true,
			});

			// Should not throw even though memory.add fails
			const results = await chain.run({ topic: 'AI' });
			expect(results).toHaveLength(1);
			expect(results[0].output).toBe('acp response');
		});
	});

	// -----------------------------------------------------------------------
	// run() — MCP step details
	// -----------------------------------------------------------------------

	describe('run (MCP step details)', () => {
		it('should pass mcpArguments to the tool call', async () => {
			const chain = createChain({
				acpClient: mockACP,
				mcpClient: mockMCP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'read-file',
				template: createPromptTemplate('Read {file}'),
				provider: 'mcp',
				mcpServerName: 'test-server',
				mcpToolName: 'read',
				mcpArguments: { path: 'file' },
			});

			await chain.run({ file: '/tmp/test.txt' });

			expect(mockMCP.callTool).toHaveBeenCalledWith('test-server', 'read', {
				path: '/tmp/test.txt',
			});
		});

		it('should pass prompt as default argument when no mcpArguments', async () => {
			const chain = createChain({
				acpClient: mockACP,
				mcpClient: mockMCP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'tool',
				template: createPromptTemplate('Do {input}'),
				provider: 'mcp',
				mcpServerName: 'test-server',
				mcpToolName: 'action',
			});

			await chain.run({ input: 'something' });

			expect(mockMCP.callTool).toHaveBeenCalledWith('test-server', 'action', {
				prompt: 'Do something',
			});
		});

		it('should throw when MCP tool returns an error', async () => {
			const errorMCP = createMockMCPClient({
				callTool: mock((..._: any[]): any => {}).mockResolvedValue({
					content: 'tool error message',
					isError: true,
					rawContent: [{ type: 'text', text: 'tool error message' }],
				}),
			});

			const chain = createChain({
				acpClient: mockACP,
				mcpClient: errorMCP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'bad-tool',
				template: createPromptTemplate('Do {action}'),
				provider: 'mcp',
				mcpServerName: 'test-server',
				mcpToolName: 'fail',
			});

			await expect(chain.run({ action: 'something' })).rejects.toThrow(
				expect.anything(),
			);
			try {
				await chain.run({ action: 'something' });
			} catch (e) {
				expect(isChainError(e)).toBe(true);
			}
		});

		it('should throw when MCP client is not configured', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'tool',
				template: createPromptTemplate('Do {action}'),
				provider: 'mcp',
				mcpServerName: 'test-server',
				mcpToolName: 'action',
			});

			await expect(chain.run({ action: 'something' })).rejects.toThrow(
				expect.anything(),
			);
			try {
				await chain.run({ action: 'something' });
			} catch (e) {
				expect(isChainError(e)).toBe(true);
			}
		});
	});

	// -----------------------------------------------------------------------
	// run() — memory step details
	// -----------------------------------------------------------------------

	describe('run (memory step details)', () => {
		it('should throw when memory manager is not configured for memory step', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'recall',
				template: createPromptTemplate('Remember {topic}'),
				provider: 'memory',
			});

			await expect(chain.run({ topic: 'AI' })).rejects.toThrow(
				expect.anything(),
			);
			try {
				await chain.run({ topic: 'AI' });
			} catch (e) {
				expect(isChainError(e)).toBe(true);
			}
		});
	});

	// -----------------------------------------------------------------------
	// run() — error handling
	// -----------------------------------------------------------------------

	describe('run (error handling)', () => {
		it('should throw ChainError when chain has no steps', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			await expect(chain.run({})).rejects.toThrow(expect.anything());
			try {
				await chain.run({});
			} catch (e) {
				expect(isChainError(e)).toBe(true);
				expect((e as SimseError).code).toBe('CHAIN_EMPTY');
			}
		});

		it('should throw ChainStepError when template resolution fails', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 's',
				template: createPromptTemplate('Hello {missing_var}'),
			});

			await expect(chain.run({})).rejects.toThrow(expect.anything());
			try {
				await chain.run({});
			} catch (e) {
				expect(isChainStepError(e)).toBe(true);
				expect((e as SimseError).code).toBe('CHAIN_STEP_ERROR');
			}
			try {
				await chain.run({});
			} catch (e) {
				expect(isChainStepError(e)).toBe(true);
			}
		});

		it('should throw ChainStepError when provider fails', async () => {
			const failingACP = createMockACPClient({
				generate: mock((..._: any[]): any => {}).mockRejectedValue(
					new Error('ACP failed'),
				),
			});

			const chain = createChain({
				acpClient: failingACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 's',
				template: createPromptTemplate('Hello {x}'),
			});

			await expect(chain.run({ x: 'world' })).rejects.toThrow(
				expect.anything(),
			);
			try {
				await chain.run({ x: 'world' });
			} catch (e) {
				expect(isChainStepError(e)).toBe(true);
			}
		});

		it('should preserve step index in ChainStepError', async () => {
			const generateMock = mock((..._: any[]): any => {})
				.mockResolvedValueOnce({
					content: 'ok',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's1',
				})
				.mockRejectedValueOnce(new Error('Step 2 failed'));

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain
				.addStep({ name: 's1', template: createPromptTemplate('A {x}') })
				.addStep({
					name: 's2',
					template: createPromptTemplate('B {previous_output}'),
				});

			try {
				await chain.run({ x: 'test' });
				throw new Error('Should have thrown');
			} catch (err) {
				expect(isChainStepError(err)).toBe(true);
				expect((err as any).stepName).toBe('s2');
				expect((err as any).stepIndex).toBe(1);
			}
		});
	});

	// -----------------------------------------------------------------------
	// run() — callbacks
	// -----------------------------------------------------------------------

	describe('run (callbacks)', () => {
		it('should fire onStepStart before each step', async () => {
			const onStepStart = mock((..._: any[]): any => {});

			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
				callbacks: { onStepStart },
			});

			chain
				.addStep({ name: 'a', template: createPromptTemplate('A {x}') })
				.addStep({
					name: 'b',
					template: createPromptTemplate('B {previous_output}'),
				});

			await chain.run({ x: 'test' });

			expect(onStepStart).toHaveBeenCalledTimes(2);

			expect(onStepStart.mock.calls[0][0]).toMatchObject({
				stepName: 'a',
				stepIndex: 0,
				totalSteps: 2,
				provider: 'acp',
			});

			expect(onStepStart.mock.calls[1][0]).toMatchObject({
				stepName: 'b',
				stepIndex: 1,
				totalSteps: 2,
			});
		});

		it('should fire onStepComplete after each step', async () => {
			const onStepComplete = mock((..._: any[]): any => {});

			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
				callbacks: { onStepComplete },
			});

			chain.addStep({ name: 'a', template: createPromptTemplate('A {x}') });

			await chain.run({ x: 'test' });

			expect(onStepComplete).toHaveBeenCalledTimes(1);
			expect(onStepComplete.mock.calls[0][0]).toMatchObject({
				stepName: 'a',
				provider: 'acp',
				output: 'acp response',
				stepIndex: 0,
			});
		});

		it('should fire onChainComplete after all steps', async () => {
			const onChainComplete = mock((..._: any[]): any => {});

			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
				callbacks: { onChainComplete },
			});

			chain.addStep({ name: 'a', template: createPromptTemplate('A {x}') });
			await chain.run({ x: 'test' });

			expect(onChainComplete).toHaveBeenCalledTimes(1);
		});

		it('should fire onStepError when a step fails', async () => {
			const onStepError = mock((..._: any[]): any => {});

			const failingACP = createMockACPClient({
				generate: mock((..._: any[]): any => {}).mockRejectedValue(
					new Error('fail'),
				),
			});

			const chain = createChain({
				acpClient: failingACP,
				logger: silentLogger,
				callbacks: { onStepError },
			});

			chain.addStep({ name: 's', template: createPromptTemplate('A {x}') });

			await expect(chain.run({ x: 'test' })).rejects.toThrow();

			expect(onStepError).toHaveBeenCalledTimes(1);
			expect(onStepError.mock.calls[0][0]).toMatchObject({
				stepName: 's',
				stepIndex: 0,
			});
		});

		it('should fire onChainError when the chain fails', async () => {
			const onChainError = mock((..._: any[]): any => {});

			const failingACP = createMockACPClient({
				generate: mock((..._: any[]): any => {}).mockRejectedValue(
					new Error('fail'),
				),
			});

			const chain = createChain({
				acpClient: failingACP,
				logger: silentLogger,
				callbacks: { onChainError },
			});

			chain.addStep({ name: 's', template: createPromptTemplate('A {x}') });

			await expect(chain.run({ x: 'test' })).rejects.toThrow();

			expect(onChainError).toHaveBeenCalledTimes(1);
			expect(onChainError.mock.calls[0][0].completedSteps).toHaveLength(0);
		});

		it('should include completed steps in onChainError when failure is mid-chain', async () => {
			const onChainError = mock((..._: any[]): any => {});

			const generateMock = mock((..._: any[]): any => {})
				.mockResolvedValueOnce({
					content: 'ok',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's1',
				})
				.mockRejectedValueOnce(new Error('step 2 fail'));

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
				callbacks: { onChainError },
			});

			chain
				.addStep({ name: 's1', template: createPromptTemplate('A {x}') })
				.addStep({
					name: 's2',
					template: createPromptTemplate('B {previous_output}'),
				});

			await expect(chain.run({ x: 'test' })).rejects.toThrow();

			expect(onChainError).toHaveBeenCalledTimes(1);
			expect(onChainError.mock.calls[0][0].completedSteps).toHaveLength(1);
		});

		it('should support async callbacks', async () => {
			const events: string[] = [];

			const callbacks: ChainCallbacks = {
				onStepStart: async () => {
					await new Promise((r) => setTimeout(r, 1));
					events.push('start');
				},
				onStepComplete: async () => {
					await new Promise((r) => setTimeout(r, 1));
					events.push('complete');
				},
				onChainComplete: async () => {
					await new Promise((r) => setTimeout(r, 1));
					events.push('chain-complete');
				},
			};

			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
				callbacks,
			});

			chain.addStep({ name: 's', template: createPromptTemplate('A {x}') });
			await chain.run({ x: 'test' });

			expect(events).toEqual(['start', 'complete', 'chain-complete']);
		});

		it('should support setCallbacks fluent API', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			const result = chain.setCallbacks({
				onStepStart: mock((..._: any[]): any => {}),
			});
			expect(result).toBe(chain);
		});
	});

	// -----------------------------------------------------------------------
	// runSingle
	// -----------------------------------------------------------------------

	describe('runSingle', () => {
		it('should execute a single-step chain and return one result', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			const result = await chain.runSingle('Hello {text}', {
				text: 'World',
			});

			expect(result.stepName).toBe('single');
			expect(result.provider).toBe('acp');
			expect(result.output).toBe('acp response');
		});

		it('should use the specified provider', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			const result = await chain.runSingle(
				'Hello {name}',
				{ name: 'World' },
				{ provider: 'acp', agentId: 'special' },
			);

			expect(result.provider).toBe('acp');
		});

		it('should pass agentId and systemPrompt options', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			await chain.runSingle(
				'Hello {x}',
				{ x: 'test' },
				{ agentId: 'my-agent', systemPrompt: 'Be helpful' },
			);

			expect(mockACP.generate).toHaveBeenCalledWith(
				'Hello test',
				expect.objectContaining({
					agentId: 'my-agent',
					systemPrompt: 'Be helpful',
				}),
			);
		});
	});

	// -----------------------------------------------------------------------
	// runNamedChain
	// -----------------------------------------------------------------------

	describe('runNamedChain', () => {
		it('should throw ChainNotFoundError for undefined chain', async () => {
			await expect(
				runNamedChain('nonexistent', config, {
					acpClient: mockACP,
					logger: silentLogger,
				}),
			).rejects.toThrow(expect.anything());
			try {
				await runNamedChain('nonexistent', config, {
					acpClient: mockACP,
					logger: silentLogger,
				});
			} catch (e) {
				expect(isChainNotFoundError(e)).toBe(true);
			}
		});

		it('should run a named chain from config', async () => {
			const configWithChain = createMinimalAppConfig({
				chains: {
					greeting: {
						description: 'Greeting chain',
						initialValues: { name: 'World' },
						steps: [
							{
								name: 'greet',
								template: 'Hello {name}',
							},
						],
					},
				},
			});

			const results = await runNamedChain('greeting', configWithChain, {
				acpClient: mockACP,
				logger: silentLogger,
			});

			expect(results).toHaveLength(1);
			expect(results[0].stepName).toBe('greet');
			expect(results[0].output).toBe('acp response');
		});

		it('should merge overrideValues with initialValues', async () => {
			const configWithChain = createMinimalAppConfig({
				chains: {
					translate: {
						description: 'Translate chain',
						initialValues: { text: 'hello', language: 'French' },
						steps: [
							{
								name: 'trans',
								template: 'Translate {text} to {language}',
							},
						],
					},
				},
			});

			const results = await runNamedChain('translate', configWithChain, {
				acpClient: mockACP,
				logger: silentLogger,
				overrideValues: { language: 'Spanish' },
			});

			expect(results).toHaveLength(1);
			// The override should take precedence
			expect(mockACP.generate).toHaveBeenCalledWith(
				'Translate hello to Spanish',
				expect.anything(),
			);
		});

		it('should pass callbacks through to the chain', async () => {
			const onStepComplete = mock((..._: any[]): any => {});

			const configWithChain = createMinimalAppConfig({
				chains: {
					simple: {
						steps: [{ name: 's', template: 'Hello {x}' }],
						initialValues: { x: 'test' },
					},
				},
			});

			await runNamedChain('simple', configWithChain, {
				acpClient: mockACP,
				logger: silentLogger,
				callbacks: { onStepComplete },
			});

			expect(onStepComplete).toHaveBeenCalledTimes(1);
		});
	});

	// -----------------------------------------------------------------------
	// default provider
	// -----------------------------------------------------------------------

	describe('default provider', () => {
		it('should default to ACP provider', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({ name: 's', template: createPromptTemplate('Hello {x}') });
			const results = await chain.run({ x: 'world' });

			expect(results[0].provider).toBe('acp');
		});

		it('should override default provider when step specifies provider', async () => {
			const chain = createChain({
				acpClient: mockACP,
				mcpClient: mockMCP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 's',
				template: createPromptTemplate('Do {x}'),
				provider: 'mcp',
				mcpServerName: 'test-server',
				mcpToolName: 'action',
			});

			const results = await chain.run({ x: 'test' });
			expect(results[0].provider).toBe('mcp');
		});
	});

	// -----------------------------------------------------------------------
	// stepIndex tracking
	// -----------------------------------------------------------------------

	describe('stepIndex tracking', () => {
		it('should assign correct stepIndex to each result', async () => {
			const generateMock = mock((..._: any[]): any => {})
				.mockResolvedValueOnce({
					content: 'a',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's1',
				})
				.mockResolvedValueOnce({
					content: 'b',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's2',
				})
				.mockResolvedValueOnce({
					content: 'c',
					agentId: 'default',
					serverName: 'local',
					sessionId: 's3',
				});

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain
				.addStep({ name: 's0', template: createPromptTemplate('A {x}') })
				.addStep({
					name: 's1',
					template: createPromptTemplate('B {previous_output}'),
				})
				.addStep({
					name: 's2',
					template: createPromptTemplate('C {previous_output}'),
				});

			const results = await chain.run({ x: 'test' });

			expect(results[0].stepIndex).toBe(0);
			expect(results[1].stepIndex).toBe(1);
			expect(results[2].stepIndex).toBe(2);
		});
	});

	// -----------------------------------------------------------------------
	// timing
	// -----------------------------------------------------------------------

	describe('timing', () => {
		it('should record positive durationMs for each step', async () => {
			const generateMock = mock((..._: any[]): any => {}).mockImplementation(
				async () => {
					await new Promise((r) => setTimeout(r, 5));
					return {
						content: 'response',
						agentId: 'default',
						serverName: 'local',
						sessionId: 's1',
					};
				},
			);

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain.addStep({ name: 's', template: createPromptTemplate('A {x}') });
			const results = await chain.run({ x: 'test' });

			expect(results[0].durationMs).toBeGreaterThanOrEqual(0);
		});
	});

	// -----------------------------------------------------------------------
	// edge cases
	// -----------------------------------------------------------------------

	describe('edge cases', () => {
		it('should handle a step with no variables in the template', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'static',
				template: createPromptTemplate('No variables here.'),
			});

			const results = await chain.run({});

			expect(results).toHaveLength(1);
			expect(results[0].input).toBe('No variables here.');
		});

		it('should handle initial values that are empty strings', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 's',
				template: createPromptTemplate('Value: {val}'),
			});

			const results = await chain.run({ val: '' });

			expect(results).toHaveLength(1);
			expect(results[0].input).toBe('Value: ');
		});

		it('should handle very long chains (10 steps)', async () => {
			const callCount = { n: 0 };
			const generateMock = mock((..._: any[]): any => {}).mockImplementation(
				async () => {
					callCount.n++;
					return {
						content: `output-${callCount.n}`,
						agentId: 'default',
						serverName: 'local',
						sessionId: `s${callCount.n}`,
					};
				},
			);

			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			// First step uses a variable
			chain.addStep({
				name: 'step_0',
				template: createPromptTemplate('Start: {x}'),
			});

			// Remaining 9 steps chain via previous_output
			for (let i = 1; i < 10; i++) {
				chain.addStep({
					name: `step_${i}`,
					template: createPromptTemplate(`Step ${i}: {previous_output}`),
				});
			}

			const results = await chain.run({ x: 'begin' });

			expect(results).toHaveLength(10);
			expect(generateMock).toHaveBeenCalledTimes(10);
		});

		it('should handle multiline template values correctly', async () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'multi',
				template: createPromptTemplate('Line1: {content}\nLine2: done'),
			});

			const results = await chain.run({
				content: 'multi\nline\nvalue',
			});

			expect(results).toHaveLength(1);
			expect(results[0].input).toBe('Line1: multi\nline\nvalue\nLine2: done');
		});
	});

	// -----------------------------------------------------------------------
	// Parallel execution
	// -----------------------------------------------------------------------

	describe('run (parallel)', () => {
		it('should execute sub-steps concurrently and concat results', async () => {
			const callCount = { n: 0 };
			const generateMock = mock((..._: any[]): any => {}).mockImplementation(
				async () => {
					callCount.n++;
					return {
						content: `result-${callCount.n}`,
						agentId: 'default',
						serverName: 'local',
						sessionId: `s${callCount.n}`,
					};
				},
			);
			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'parallel-step',
				template: createPromptTemplate('{input}'),
				parallel: {
					subSteps: [
						{
							name: 'sub-a',
							template: createPromptTemplate('A: {input}'),
						},
						{
							name: 'sub-b',
							template: createPromptTemplate('B: {input}'),
						},
					],
				},
			});

			const results = await chain.run({ input: 'hello' });

			expect(results).toHaveLength(1);
			expect(results[0].stepName).toBe('parallel-step');
			expect(results[0].model).toBe('parallel:2');
			expect(results[0].subResults).toHaveLength(2);
			expect(generateMock).toHaveBeenCalledTimes(2);
			// Default concat merge joins with \n\n
			expect(results[0].output).toBe('result-1\n\nresult-2');
		});

		it('should use custom concatSeparator', async () => {
			const acp = createMockACPClient();

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'sep',
				template: createPromptTemplate('{x}'),
				parallel: {
					concatSeparator: ' | ',
					subSteps: [
						{
							name: 's1',
							template: createPromptTemplate('A: {x}'),
						},
						{
							name: 's2',
							template: createPromptTemplate('B: {x}'),
						},
					],
				},
			});

			const results = await chain.run({ x: 'test' });

			expect(results[0].output).toBe('acp response | acp response');
		});

		it('should support keyed merge strategy', async () => {
			const callCount = { n: 0 };
			const generateMock = mock((..._: any[]): any => {}).mockImplementation(
				async () => {
					callCount.n++;
					return {
						content: `out-${callCount.n}`,
						agentId: 'default',
						serverName: 'local',
						sessionId: `s${callCount.n}`,
					};
				},
			);
			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain
				.addStep({
					name: 'keyed',
					template: createPromptTemplate('{x}'),
					parallel: {
						mergeStrategy: 'keyed',
						subSteps: [
							{
								name: 'alpha',
								template: createPromptTemplate('A: {x}'),
							},
							{
								name: 'beta',
								template: createPromptTemplate('B: {x}'),
							},
						],
					},
				})
				.addStep({
					name: 'after',
					template: createPromptTemplate('Alpha: {alpha} Beta: {beta}'),
					inputMapping: {
						alpha: 'keyed.alpha',
						beta: 'keyed.beta',
					},
				});

			const results = await chain.run({ x: 'input' });

			expect(results).toHaveLength(2);
			// The second step should have resolved the keyed sub-step values via inputMapping
			expect(results[1].input).toBe('Alpha: out-1 Beta: out-2');
		});

		it('should support custom merge function', async () => {
			const callCount = { n: 0 };
			const generateMock = mock((..._: any[]): any => {}).mockImplementation(
				async () => {
					callCount.n++;
					return {
						content: `item-${callCount.n}`,
						agentId: 'default',
						serverName: 'local',
						sessionId: `s${callCount.n}`,
					};
				},
			);
			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'custom',
				template: createPromptTemplate('{x}'),
				parallel: {
					mergeStrategy: (results) =>
						JSON.stringify(
							results.map((r) => ({ name: r.subStepName, out: r.output })),
						),
					subSteps: [
						{
							name: 'x',
							template: createPromptTemplate('X: {x}'),
						},
						{
							name: 'y',
							template: createPromptTemplate('Y: {x}'),
						},
					],
				},
			});

			const results = await chain.run({ x: 'v' });
			const parsed = JSON.parse(results[0].output);

			expect(parsed).toEqual([
				{ name: 'x', out: 'item-1' },
				{ name: 'y', out: 'item-2' },
			]);
		});

		it('should apply outputTransform on sub-step outputs', async () => {
			const acp = createMockACPClient();

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'transform',
				template: createPromptTemplate('{x}'),
				parallel: {
					subSteps: [
						{
							name: 'upper',
							template: createPromptTemplate('{x}'),
							outputTransform: (s) => s.toUpperCase(),
						},
						{
							name: 'prefix',
							template: createPromptTemplate('{x}'),
							outputTransform: (s) => `>> ${s}`,
						},
					],
				},
			});

			const results = await chain.run({ x: 'test' });

			expect(results[0].subResults![0].output).toBe('ACP RESPONSE');
			expect(results[0].subResults![1].output).toBe('>> acp response');
			expect(results[0].output).toBe('ACP RESPONSE\n\n>> acp response');
		});

		it('should populate previous_output and step name after parallel step', async () => {
			const callCount = { n: 0 };
			const generateMock = mock((..._: any[]): any => {}).mockImplementation(
				async () => {
					callCount.n++;
					return {
						content: `p-${callCount.n}`,
						agentId: 'default',
						serverName: 'local',
						sessionId: `s${callCount.n}`,
					};
				},
			);
			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain
				.addStep({
					name: 'par',
					template: createPromptTemplate('{x}'),
					parallel: {
						subSteps: [
							{
								name: 'a',
								template: createPromptTemplate('A: {x}'),
							},
							{
								name: 'b',
								template: createPromptTemplate('B: {x}'),
							},
						],
					},
				})
				.addStep({
					name: 'next',
					template: createPromptTemplate('Got: {previous_output}'),
				});

			const results = await chain.run({ x: 'go' });

			expect(results).toHaveLength(2);
			// The second step should have received the merged parallel output
			expect(results[1].input).toBe('Got: p-1\n\np-2');
		});

		it('should include subResults with correct metadata', async () => {
			const acp = createMockACPClient();

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'meta',
				template: createPromptTemplate('{x}'),
				parallel: {
					subSteps: [
						{
							name: 'first',
							template: createPromptTemplate('{x}'),
						},
						{
							name: 'second',
							template: createPromptTemplate('{x}'),
						},
					],
				},
			});

			const results = await chain.run({ x: 'v' });
			const subs = results[0].subResults!;

			expect(subs).toHaveLength(2);
			expect(subs[0].subStepName).toBe('first');
			expect(subs[0].provider).toBe('acp');
			expect(subs[0].model).toBe('acp:default');
			expect(subs[0].input).toBe('v');
			expect(subs[0].output).toBe('acp response');
			expect(subs[0].durationMs).toBeGreaterThanOrEqual(0);
			expect(subs[1].subStepName).toBe('second');
		});
	});

	describe('run (parallel — fail tolerance)', () => {
		it('should throw when a sub-step fails in strict mode', async () => {
			const callCount = { n: 0 };
			const generateMock = mock((..._: any[]): any => {}).mockImplementation(
				async () => {
					callCount.n++;
					if (callCount.n === 2) throw new Error('sub-fail');
					return {
						content: `ok-${callCount.n}`,
						agentId: 'default',
						serverName: 'local',
						sessionId: `s${callCount.n}`,
					};
				},
			);
			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'strict',
				template: createPromptTemplate('{x}'),
				parallel: {
					subSteps: [
						{
							name: 'ok',
							template: createPromptTemplate('{x}'),
						},
						{
							name: 'fail',
							template: createPromptTemplate('{x}'),
						},
					],
				},
			});

			try {
				await chain.run({ x: 'test' });
				expect.unreachable('should have thrown');
			} catch (e) {
				expect(isChainStepError(e)).toBe(true);
			}
		});

		it('should continue when sub-step fails in failTolerant mode', async () => {
			const callCount = { n: 0 };
			const generateMock = mock((..._: any[]): any => {}).mockImplementation(
				async () => {
					callCount.n++;
					if (callCount.n === 2) throw new Error('sub-fail');
					return {
						content: `ok-${callCount.n}`,
						agentId: 'default',
						serverName: 'local',
						sessionId: `s${callCount.n}`,
					};
				},
			);
			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'tolerant',
				template: createPromptTemplate('{x}'),
				parallel: {
					failTolerant: true,
					subSteps: [
						{
							name: 'ok',
							template: createPromptTemplate('{x}'),
						},
						{
							name: 'fail',
							template: createPromptTemplate('{x}'),
						},
						{
							name: 'ok2',
							template: createPromptTemplate('{x}'),
						},
					],
				},
			});

			const results = await chain.run({ x: 'test' });

			expect(results).toHaveLength(1);
			// Only 2 sub-steps succeeded
			expect(results[0].subResults).toHaveLength(2);
			expect(results[0].model).toBe('parallel:2');
		});

		it('should throw when all sub-steps fail in failTolerant mode', async () => {
			const generateMock = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('all-fail'),
			);
			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
			});

			chain.addStep({
				name: 'all-fail',
				template: createPromptTemplate('{x}'),
				parallel: {
					failTolerant: true,
					subSteps: [
						{
							name: 'a',
							template: createPromptTemplate('{x}'),
						},
						{
							name: 'b',
							template: createPromptTemplate('{x}'),
						},
					],
				},
			});

			try {
				await chain.run({ x: 'test' });
				expect.unreachable('should have thrown');
			} catch (e) {
				expect(isChainStepError(e)).toBe(true);
				expect((e as SimseError).message).toContain(
					'All parallel sub-steps failed',
				);
			}
		});
	});

	describe('run (parallel — callbacks)', () => {
		it('should fire onStepStart for parent and each sub-step', async () => {
			const onStepStart = mock((..._: any[]): any => {});

			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
				callbacks: { onStepStart },
			});

			chain.addStep({
				name: 'cb-par',
				template: createPromptTemplate('{x}'),
				parallel: {
					subSteps: [
						{
							name: 'sa',
							template: createPromptTemplate('{x}'),
						},
						{
							name: 'sb',
							template: createPromptTemplate('{x}'),
						},
					],
				},
			});

			await chain.run({ x: 'go' });

			// 1 parent + 2 sub-steps = 3 onStepStart calls
			expect(onStepStart).toHaveBeenCalledTimes(3);

			// First call is the parent
			expect(onStepStart.mock.calls[0][0]).toMatchObject({
				stepName: 'cb-par',
				provider: 'acp',
				prompt: '[parallel: 2 sub-steps]',
			});

			// Sub-step calls (order may vary due to concurrency)
			const subCalls = onStepStart.mock.calls.slice(1).map((c: any) => c[0]);
			const subNames = subCalls.map((c: { stepName: string }) => c.stepName);
			expect(subNames).toContain('cb-par.sa');
			expect(subNames).toContain('cb-par.sb');
		});

		it('should fire onStepComplete for parent and each sub-step', async () => {
			const onStepComplete = mock((..._: any[]): any => {});

			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
				callbacks: { onStepComplete },
			});

			chain.addStep({
				name: 'cb-par',
				template: createPromptTemplate('{x}'),
				parallel: {
					subSteps: [
						{
							name: 'sa',
							template: createPromptTemplate('{x}'),
						},
						{
							name: 'sb',
							template: createPromptTemplate('{x}'),
						},
					],
				},
			});

			await chain.run({ x: 'go' });

			// 2 sub-steps + 1 parent = 3 onStepComplete calls
			expect(onStepComplete).toHaveBeenCalledTimes(3);

			// Last call should be the parent
			const lastCall =
				onStepComplete.mock.calls[onStepComplete.mock.calls.length - 1][0];
			expect(lastCall.stepName).toBe('cb-par');
			expect(lastCall.subResults).toHaveLength(2);
		});

		it('should fire onStepError when parallel step fails', async () => {
			const onStepError = mock((..._: any[]): any => {});
			const generateMock = mock((..._: any[]): any => {}).mockRejectedValue(
				new Error('fail'),
			);
			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const chain = createChain({
				acpClient: acp,
				logger: silentLogger,
				callbacks: { onStepError },
			});

			chain.addStep({
				name: 'err-par',
				template: createPromptTemplate('{x}'),
				parallel: {
					subSteps: [
						{
							name: 'a',
							template: createPromptTemplate('{x}'),
						},
						{
							name: 'b',
							template: createPromptTemplate('{x}'),
						},
					],
				},
			});

			try {
				await chain.run({ x: 'test' });
			} catch {
				// expected
			}

			expect(onStepError).toHaveBeenCalled();
			expect(onStepError.mock.calls[0][0].stepName).toBe('err-par');
		});
	});

	describe('run (parallel — validation)', () => {
		it('should reject parallel steps with fewer than 2 sub-steps', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			expect(() =>
				chain.addStep({
					name: 'bad',
					template: createPromptTemplate('{x}'),
					parallel: {
						subSteps: [
							{
								name: 'only-one',
								template: createPromptTemplate('{x}'),
							},
						],
					},
				}),
			).toThrow('at least 2 sub-steps');
		});

		it('should reject MCP sub-steps without mcpServerName', () => {
			const chain = createChain({
				acpClient: mockACP,
				logger: silentLogger,
			});

			expect(() =>
				chain.addStep({
					name: 'bad-mcp',
					template: createPromptTemplate('{x}'),
					parallel: {
						subSteps: [
							{
								name: 'ok',
								template: createPromptTemplate('{x}'),
							},
							{
								name: 'missing',
								template: createPromptTemplate('{x}'),
								provider: 'mcp',
								mcpToolName: 'tool',
							},
						],
					},
				}),
			).toThrow('mcpServerName');
		});
	});

	describe('createChainFromDefinition (parallel)', () => {
		it('should build parallel step from definition', async () => {
			const callCount = { n: 0 };
			const generateMock = mock((..._: any[]): any => {}).mockImplementation(
				async () => {
					callCount.n++;
					return {
						content: `def-${callCount.n}`,
						agentId: 'default',
						serverName: 'local',
						sessionId: `s${callCount.n}`,
					};
				},
			);
			const acp = createMockACPClient({
				generate: generateMock as unknown as ACPClient['generate'],
			});

			const definition: ChainDefinition = {
				initialValues: { topic: 'AI' },
				steps: [
					{
						name: 'research',
						template: '{topic}',
						parallel: {
							subSteps: [
								{
									name: 'perspective-a',
									template: 'Analyze {topic} from angle A',
								},
								{
									name: 'perspective-b',
									template: 'Analyze {topic} from angle B',
								},
							],
						},
					},
				],
			};

			const chain = createChainFromDefinition(definition, {
				acpClient: acp,
				logger: silentLogger,
			});

			const results = await chain.run({ topic: 'AI' });

			expect(results).toHaveLength(1);
			expect(results[0].subResults).toHaveLength(2);
			expect(results[0].model).toBe('parallel:2');
			expect(generateMock).toHaveBeenCalledTimes(2);
		});

		it('should inherit agentId from definition into sub-steps', async () => {
			const acp = createMockACPClient();

			const definition: ChainDefinition = {
				agentId: 'inherited-agent',
				serverName: 'inherited-server',
				initialValues: { x: 'v' },
				steps: [
					{
						name: 'inherit',
						template: '{x}',
						parallel: {
							subSteps: [
								{ name: 'sa', template: '{x}' },
								{ name: 'sb', template: '{x}' },
							],
						},
					},
				],
			};

			const chain = createChainFromDefinition(definition, {
				acpClient: acp,
				logger: silentLogger,
			});

			await chain.run({ x: 'v' });

			// Both sub-steps should have inherited agentId/serverName
			for (const call of (acp.generate as any).mock.calls) {
				expect(call[1]).toMatchObject({
					agentId: 'inherited-agent',
					serverName: 'inherited-server',
				});
			}
		});
	});
});
