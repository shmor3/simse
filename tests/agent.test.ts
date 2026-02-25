import { describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';
import type { ACPGenerateResult } from '../src/ai/acp/types.js';
import { createAgentExecutor } from '../src/ai/agent/agent-executor.js';
import type { MCPClient } from '../src/ai/mcp/mcp-client.js';
import type { MemoryManager } from '../src/ai/memory/memory.js';
import type { SearchResult } from '../src/ai/memory/types.js';
import { isChainError, isMCPToolError } from '../src/errors/index.js';
import { createLogger, type Logger } from '../src/logger.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createSilentLogger(): Logger {
	return createLogger({ context: 'test', level: 'none', transports: [] });
}

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
		}),
		listResources: mock((..._: any[]): any => {}).mockResolvedValue([]),
		readResource: mock((..._: any[]): any => {}).mockResolvedValue(''),
		listPrompts: mock((..._: any[]): any => {}).mockResolvedValue([]),
		getPrompt: mock((..._: any[]): any => {}).mockResolvedValue(''),
		...overrides,
	} as unknown as MCPClient;
}

function createMockMemoryManager(
	overrides: Partial<MemoryManager> = {},
): MemoryManager {
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
				entry: {
					id: 'e1',
					text: 'previous memory',
					embedding: [0.1],
					metadata: {},
					timestamp: Date.now(),
				},
				score: 0.95,
			},
		] satisfies SearchResult[]),
		delete: mock((..._: any[]): any => {}).mockResolvedValue(true),
		deleteBatch: mock((..._: any[]): any => {}).mockResolvedValue(1),
		clear: mock((..._: any[]): any => {}),
		size: 5,
		isInitialized: true,
		isDirty: false,
		embeddingAgent: 'embedding',
		...overrides,
	} as unknown as MemoryManager;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('createAgentExecutor', () => {
	const logger = createSilentLogger();

	describe('execute (acp)', () => {
		it('should call acpClient.generate with correct options', async () => {
			const acp = createMockACPClient();
			const executor = createAgentExecutor({ acpClient: acp, logger });

			await executor.execute(
				{
					name: 'test-step',
					agentId: 'my-agent',
					serverName: 'my-server',
					systemPrompt: 'You are helpful.',
					agentConfig: { temperature: 0.5 },
				},
				'acp',
				'Hello world',
				{},
			);

			expect(acp.generate).toHaveBeenCalledWith('Hello world', {
				agentId: 'my-agent',
				serverName: 'my-server',
				systemPrompt: 'You are helpful.',
				config: { temperature: 0.5 },
			});
		});

		it('should return AgentResult with model "acp:{agentId}"', async () => {
			const acp = createMockACPClient();
			const executor = createAgentExecutor({ acpClient: acp, logger });

			const result = await executor.execute(
				{ name: 'step' },
				'acp',
				'prompt',
				{},
			);

			expect(result.output).toBe('acp response');
			expect(result.model).toBe('acp:default');
		});

		it('should include usage when present in generate result', async () => {
			const acp = createMockACPClient({
				generate: mock((..._: any[]): any => {}).mockResolvedValue({
					content: 'response with usage',
					agentId: 'agent-1',
					serverName: 'local',
					sessionId: 'session-3',
					usage: {
						promptTokens: 10,
						completionTokens: 20,
						totalTokens: 30,
					},
				}),
			});
			const executor = createAgentExecutor({ acpClient: acp, logger });

			const result = await executor.execute(
				{ name: 'step' },
				'acp',
				'prompt',
				{},
			);

			expect(result.usage).toEqual({
				promptTokens: 10,
				completionTokens: 20,
				totalTokens: 30,
			});
		});
	});

	describe('execute (mcp)', () => {
		it('should call mcpClient.callTool with correct args', async () => {
			const acp = createMockACPClient();
			const mcp = createMockMCPClient();
			const executor = createAgentExecutor({
				acpClient: acp,
				mcpClient: mcp,
				logger,
			});

			await executor.execute(
				{
					name: 'mcp-step',
					mcpServerName: 'test-server',
					mcpToolName: 'test-tool',
				},
				'mcp',
				'prompt text',
				{},
			);

			expect(mcp.callTool).toHaveBeenCalledWith('test-server', 'test-tool', {
				prompt: 'prompt text',
			});
		});

		it('should resolve mcpArguments from currentValues', async () => {
			const acp = createMockACPClient();
			const mcp = createMockMCPClient();
			const executor = createAgentExecutor({
				acpClient: acp,
				mcpClient: mcp,
				logger,
			});

			await executor.execute(
				{
					name: 'mcp-step',
					mcpServerName: 'server',
					mcpToolName: 'tool',
					mcpArguments: { input: 'sourceKey' },
				},
				'mcp',
				'ignored prompt',
				{ sourceKey: 'resolved value' },
			);

			expect(mcp.callTool).toHaveBeenCalledWith('server', 'tool', {
				input: 'resolved value',
			});
		});

		it('should return model "mcp:{server}/{tool}" and toolMetrics', async () => {
			const acp = createMockACPClient();
			const mcp = createMockMCPClient();
			const executor = createAgentExecutor({
				acpClient: acp,
				mcpClient: mcp,
				logger,
			});

			const result = await executor.execute(
				{
					name: 'mcp-step',
					mcpServerName: 'test-server',
					mcpToolName: 'test-tool',
				},
				'mcp',
				'prompt',
				{},
			);

			expect(result.model).toBe('mcp:test-server/test-tool');
			expect(result.output).toBe('tool result');
			expect(result.toolMetrics).toBeDefined();
			expect(result.toolMetrics?.serverName).toBe('test-server');
		});

		it('should throw CHAIN_MCP_NOT_CONFIGURED when mcpClient is absent', async () => {
			const acp = createMockACPClient();
			const executor = createAgentExecutor({ acpClient: acp, logger });

			try {
				await executor.execute(
					{
						name: 'mcp-step',
						mcpServerName: 'server',
						mcpToolName: 'tool',
					},
					'mcp',
					'prompt',
					{},
				);
				expect.unreachable('should have thrown');
			} catch (e) {
				expect(isChainError(e)).toBe(true);
				expect((e as any).code).toBe('CHAIN_MCP_NOT_CONFIGURED');
			}
		});

		it('should throw CHAIN_INVALID_STEP when mcpServerName is missing', async () => {
			const acp = createMockACPClient();
			const mcp = createMockMCPClient();
			const executor = createAgentExecutor({
				acpClient: acp,
				mcpClient: mcp,
				logger,
			});

			try {
				await executor.execute(
					{ name: 'mcp-step', mcpToolName: 'tool' },
					'mcp',
					'prompt',
					{},
				);
				expect.unreachable('should have thrown');
			} catch (e) {
				expect(isChainError(e)).toBe(true);
				expect((e as any).code).toBe('CHAIN_INVALID_STEP');
			}
		});

		it('should throw MCP tool error when tool returns isError:true', async () => {
			const acp = createMockACPClient();
			const mcp = createMockMCPClient({
				callTool: mock((..._: any[]): any => {}).mockResolvedValue({
					content: 'error: something broke',
					isError: true,
					rawContent: [],
					metrics: {
						durationMs: 10,
						serverName: 's',
						toolName: 't',
						startedAt: new Date().toISOString(),
					},
				}),
			});
			const executor = createAgentExecutor({
				acpClient: acp,
				mcpClient: mcp,
				logger,
			});

			try {
				await executor.execute(
					{
						name: 'mcp-step',
						mcpServerName: 's',
						mcpToolName: 't',
					},
					'mcp',
					'prompt',
					{},
				);
				expect.unreachable('should have thrown');
			} catch (e) {
				expect(isMCPToolError(e)).toBe(true);
			}
		});
	});

	describe('execute (memory)', () => {
		it('should call memoryManager.search with the prompt', async () => {
			const acp = createMockACPClient();
			const memory = createMockMemoryManager();
			const executor = createAgentExecutor({
				acpClient: acp,
				memoryManager: memory,
				logger,
			});

			await executor.execute(
				{ name: 'mem-step' },
				'memory',
				'search query',
				{},
			);

			expect(memory.search).toHaveBeenCalledWith('search query');
		});

		it('should return formatted search results', async () => {
			const acp = createMockACPClient();
			const memory = createMockMemoryManager();
			const executor = createAgentExecutor({
				acpClient: acp,
				memoryManager: memory,
				logger,
			});

			const result = await executor.execute(
				{ name: 'mem-step' },
				'memory',
				'query',
				{},
			);

			expect(result.output).toContain('previous memory');
			expect(result.model).toBe('memory:vector-search');
		});

		it('should throw CHAIN_MEMORY_NOT_CONFIGURED when memoryManager is absent', async () => {
			const acp = createMockACPClient();
			const executor = createAgentExecutor({ acpClient: acp, logger });

			try {
				await executor.execute({ name: 'mem-step' }, 'memory', 'query', {});
				expect.unreachable('should have thrown');
			} catch (e) {
				expect(isChainError(e)).toBe(true);
				expect((e as any).code).toBe('CHAIN_MEMORY_NOT_CONFIGURED');
			}
		});
	});

	describe('execute (unknown provider)', () => {
		it('should throw CHAIN_UNKNOWN_PROVIDER for invalid provider', async () => {
			const acp = createMockACPClient();
			const executor = createAgentExecutor({ acpClient: acp, logger });

			try {
				await executor.execute(
					{ name: 'bad-step' },
					'unknown' as any,
					'prompt',
					{},
				);
				expect.unreachable('should have thrown');
			} catch (e) {
				expect(isChainError(e)).toBe(true);
				expect((e as any).code).toBe('CHAIN_UNKNOWN_PROVIDER');
			}
		});
	});

	describe('Object.freeze', () => {
		it('should return a frozen executor', () => {
			const acp = createMockACPClient();
			const executor = createAgentExecutor({ acpClient: acp, logger });
			expect(Object.isFrozen(executor)).toBe(true);
		});
	});
});
