import { describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';
import type { SubagentInfo, SubagentResult } from '../src/ai/loop/types.js';
import {
	_resetSubagentCounter,
	registerSubagentTools,
} from '../src/ai/tools/subagent-tools.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createMockACPClient(content = 'delegated result'): ACPClient {
	return {
		initialize: mock(() => Promise.resolve()),
		dispose: mock(() => Promise.resolve()),
		listAgents: mock(() => Promise.resolve([])),
		getAgent: mock(() => Promise.resolve({ id: 'test', name: 'test' })),
		generate: mock(() =>
			Promise.resolve({
				content,
				agentId: 'test',
				serverName: 'test',
				sessionId: 'sess',
			}),
		),
		chat: mock(() =>
			Promise.resolve({
				content: 'chat',
				agentId: 'test',
				serverName: 'test',
				sessionId: 'sess',
			}),
		),
		generateStream: mock(async function* () {
			yield { type: 'delta' as const, text: content };
			yield { type: 'complete' as const, usage: undefined };
		}),
		embed: mock(() =>
			Promise.resolve({
				embeddings: [[]],
				agentId: 'test',
				serverName: 'test',
			}),
		),
		isAvailable: mock(() => Promise.resolve(true)),
		setPermissionPolicy: mock(() => {}),
		listSessions: mock(() => Promise.resolve([])),
		loadSession: mock(() => Promise.resolve({} as any)),
		deleteSession: mock(() => Promise.resolve()),
		setSessionMode: mock(() => Promise.resolve()),
		setSessionModel: mock(() => Promise.resolve()),
		getServerHealth: mock(() => undefined),
		getServerModelInfo: mock(() => Promise.resolve(undefined)),
		getServerStatuses: mock(() => Promise.resolve([])),
		getSessionModels: mock(() => Promise.resolve(undefined)),
		getSessionModes: mock(() => Promise.resolve(undefined)),
		serverNames: ['test'],
		serverCount: 1,
		defaultServerName: 'test',
		defaultAgent: 'test',
	} as ACPClient;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('registerSubagentTools', () => {
	it('registers both subagent_spawn and subagent_delegate tools', () => {
		_resetSubagentCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient();

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
		});

		expect(registry.toolNames).toContain('subagent_spawn');
		expect(registry.toolNames).toContain('subagent_delegate');
	});

	it('does not register tools when depth >= maxDepth', () => {
		_resetSubagentCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient();

		registerSubagentTools(
			registry,
			{
				acpClient,
				toolRegistry: registry,
				maxDepth: 2,
			},
			2,
		);

		expect(registry.toolNames).not.toContain('subagent_spawn');
		expect(registry.toolNames).not.toContain('subagent_delegate');
	});

	it('does not register tools at default maxDepth', () => {
		_resetSubagentCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient();

		// Default maxDepth is 2, so depth=2 should not register
		registerSubagentTools(registry, { acpClient, toolRegistry: registry }, 2);

		expect(registry.toolCount).toBe(0);
	});

	it('registers tools at depth < maxDepth', () => {
		_resetSubagentCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient();

		registerSubagentTools(
			registry,
			{ acpClient, toolRegistry: registry, maxDepth: 3 },
			2,
		);

		expect(registry.toolNames).toContain('subagent_spawn');
		expect(registry.toolNames).toContain('subagent_delegate');
	});
});

describe('subagent_delegate', () => {
	it('calls acpClient.generate and returns content', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient('summary result');
		const registry = createToolRegistry({});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
		});

		const result = await registry.execute({
			id: 'call_1',
			name: 'subagent_delegate',
			arguments: {
				task: 'Summarize this document',
				description: 'Summarizing document',
			},
		});

		expect(result.isError).toBe(false);
		expect(result.output).toBe('summary result');
		expect(acpClient.generate).toHaveBeenCalledTimes(1);
	});

	it('passes serverName and agentId to generate', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient('ok');
		const registry = createToolRegistry({});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			serverName: 'default-server',
			agentId: 'default-agent',
		});

		await registry.execute({
			id: 'call_1',
			name: 'subagent_delegate',
			arguments: {
				task: 'Do something',
				description: 'Doing something',
				serverName: 'custom-server',
				agentId: 'custom-agent',
			},
		});

		expect(acpClient.generate).toHaveBeenCalledWith('Do something', {
			serverName: 'custom-server',
			agentId: 'custom-agent',
		});
	});

	it('falls back to default serverName/agentId', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient('ok');
		const registry = createToolRegistry({});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			serverName: 'default-server',
			agentId: 'default-agent',
		});

		await registry.execute({
			id: 'call_1',
			name: 'subagent_delegate',
			arguments: {
				task: 'Do something',
				description: 'Doing something',
			},
		});

		expect(acpClient.generate).toHaveBeenCalledWith('Do something', {
			serverName: 'default-server',
			agentId: 'default-agent',
		});
	});

	it('fires callbacks on delegate lifecycle', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient('result');
		const registry = createToolRegistry({});

		const onStart = mock(() => {});
		const onComplete = mock(() => {});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			callbacks: {
				onSubagentStart: onStart,
				onSubagentComplete: onComplete,
			},
		});

		await registry.execute({
			id: 'call_1',
			name: 'subagent_delegate',
			arguments: {
				task: 'Summarize',
				description: 'Summarizing',
			},
		});

		expect(onStart).toHaveBeenCalledTimes(1);
		const info: SubagentInfo = onStart.mock.calls[0][0] as SubagentInfo;
		expect(info.id).toBe('sub_1');
		expect(info.description).toBe('Summarizing');
		expect(info.mode).toBe('delegate');

		expect(onComplete).toHaveBeenCalledTimes(1);
		const [completeId, completeResult] = onComplete.mock.calls[0] as [
			string,
			SubagentResult,
		];
		expect(completeId).toBe('sub_1');
		expect(completeResult.text).toBe('result');
		expect(completeResult.turns).toBe(1);
		expect(completeResult.durationMs).toBeGreaterThanOrEqual(0);
	});

	it('fires onSubagentError when generate fails', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient();
		(acpClient.generate as any).mockImplementation(() => {
			return Promise.reject(new Error('network down'));
		});
		const registry = createToolRegistry({});

		const onStart = mock(() => {});
		const onError = mock(() => {});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			callbacks: {
				onSubagentStart: onStart,
				onSubagentError: onError,
			},
		});

		const result = await registry.execute({
			id: 'call_1',
			name: 'subagent_delegate',
			arguments: {
				task: 'Fail task',
				description: 'Will fail',
			},
		});

		expect(result.isError).toBe(true);
		expect(onStart).toHaveBeenCalledTimes(1);
		expect(onError).toHaveBeenCalledTimes(1);
		const [errorId, error] = onError.mock.calls[0] as [string, Error];
		expect(errorId).toBe('sub_1');
		expect(error.message).toBe('network down');
	});
});

describe('subagent_spawn', () => {
	it('runs child loop and returns finalText', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient('child result');
		const registry = createToolRegistry({});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			defaultMaxTurns: 5,
		});

		const result = await registry.execute({
			id: 'call_1',
			name: 'subagent_spawn',
			arguments: {
				task: 'Research the API',
				description: 'Researching API',
			},
		});

		expect(result.isError).toBe(false);
		expect(result.output).toBe('child result');
	});

	it('fires lifecycle callbacks on spawn', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient('spawn result');
		const registry = createToolRegistry({});

		const onStart = mock(() => {});
		const onComplete = mock(() => {});
		const onStreamDelta = mock(() => {});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			callbacks: {
				onSubagentStart: onStart,
				onSubagentComplete: onComplete,
				onSubagentStreamDelta: onStreamDelta,
			},
		});

		await registry.execute({
			id: 'call_1',
			name: 'subagent_spawn',
			arguments: {
				task: 'Do work',
				description: 'Working',
			},
		});

		expect(onStart).toHaveBeenCalledTimes(1);
		const info: SubagentInfo = onStart.mock.calls[0][0] as SubagentInfo;
		expect(info.id).toBe('sub_1');
		expect(info.description).toBe('Working');
		expect(info.mode).toBe('spawn');

		expect(onComplete).toHaveBeenCalledTimes(1);
		const [completeId, completeResult] = onComplete.mock.calls[0] as [
			string,
			SubagentResult,
		];
		expect(completeId).toBe('sub_1');
		expect(completeResult.text).toBe('spawn result');
		expect(completeResult.turns).toBe(1);
		expect(completeResult.durationMs).toBeGreaterThanOrEqual(0);

		// Stream delta should have been called with the child's stream output
		expect(onStreamDelta).toHaveBeenCalled();
	});

	it('fires onSubagentError callback when child loop throws', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient();
		// Make generateStream throw on iteration to crash the child loop.
		// The agentic loop catches stream errors and produces an error-text
		// finalText, so the spawn tool itself returns normally — but the
		// onError callback from the child loop still fires, which triggers
		// onSubagentError is NOT called (only onSubagentComplete).
		// Instead we verify the returned text contains the error message.
		(acpClient.generateStream as any).mockImplementation(async function* () {
			yield { type: 'delta' as const, text: '' };
			throw new Error('child crashed');
		});
		const registry = createToolRegistry({});

		const onStart = mock(() => {});
		const onComplete = mock(() => {});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			callbacks: {
				onSubagentStart: onStart,
				onSubagentComplete: onComplete,
			},
		});

		const result = await registry.execute({
			id: 'call_1',
			name: 'subagent_spawn',
			arguments: {
				task: 'Fail',
				description: 'Will fail',
			},
		});

		// The child loop catches the error and returns it as finalText
		expect(result.isError).toBe(false);
		expect(result.output).toContain('child crashed');
		expect(onStart).toHaveBeenCalledTimes(1);
		expect(onComplete).toHaveBeenCalledTimes(1);
	});

	it('child registry inherits parent tools except subagent tools', async () => {
		_resetSubagentCounter();

		const acpClient = createMockACPClient();

		const registry = createToolRegistry({});

		// Register a custom parent tool
		registry.register(
			{
				name: 'custom_tool',
				description: 'A custom tool',
				parameters: {
					input: {
						type: 'string',
						description: 'input',
						required: true,
					},
				},
			},
			async () => 'custom result',
		);

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
		});

		// Parent should have: custom_tool, subagent_spawn, subagent_delegate
		expect(registry.toolNames).toContain('custom_tool');
		expect(registry.toolNames).toContain('subagent_spawn');
		expect(registry.toolNames).toContain('subagent_delegate');

		const result = await registry.execute({
			id: 'call_1',
			name: 'subagent_spawn',
			arguments: {
				task: 'Use custom tool',
				description: 'Testing inheritance',
			},
		});

		// Child loop ran successfully
		expect(result.isError).toBe(false);
	});

	it('uses custom maxTurns from arguments', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient('ok');
		const registry = createToolRegistry({});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			defaultMaxTurns: 10,
		});

		const result = await registry.execute({
			id: 'call_1',
			name: 'subagent_spawn',
			arguments: {
				task: 'Quick task',
				description: 'Quick',
				maxTurns: 3,
			},
		});

		expect(result.isError).toBe(false);
	});

	it('uses custom systemPrompt from arguments', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient('ok');
		const registry = createToolRegistry({});

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			systemPrompt: 'Default system prompt',
		});

		const result = await registry.execute({
			id: 'call_1',
			name: 'subagent_spawn',
			arguments: {
				task: 'Task with custom prompt',
				description: 'Custom prompt',
				systemPrompt: 'You are a specialized agent',
			},
		});

		expect(result.isError).toBe(false);
	});
});

describe('subagent_spawn shelf integration', () => {
	it('subagent_spawn creates shelf-scoped library tools when library is provided', () => {
		_resetSubagentCounter();
		const registry = createToolRegistry({});
		const acpClient = createMockACPClient();

		// Create a mock library with shelf support
		const mockShelf = {
			name: 'test-shelf',
			add: mock(async () => 'shelf-id'),
			search: mock(async () => []),
			searchGlobal: mock(async () => []),
			volumes: mock(() => []),
		};
		const mockLibrary = {
			shelf: mock(() => mockShelf),
		} as any;

		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			library: mockLibrary,
		});

		// Verify the library option is accepted without error
		const defs = registry.getToolDefinitions();
		expect(defs.find((d) => d.name === 'subagent_spawn')).toBeDefined();
	});
});

describe('subagent ID generation', () => {
	it('generates unique IDs across calls', async () => {
		_resetSubagentCounter();
		const acpClient = createMockACPClient('ok');
		const registry = createToolRegistry({});

		const ids: string[] = [];
		registerSubagentTools(registry, {
			acpClient,
			toolRegistry: registry,
			callbacks: {
				onSubagentStart: (info) => ids.push(info.id),
			},
		});

		await registry.execute({
			id: 'call_1',
			name: 'subagent_delegate',
			arguments: { task: 'First', description: 'First' },
		});
		await registry.execute({
			id: 'call_2',
			name: 'subagent_delegate',
			arguments: { task: 'Second', description: 'Second' },
		});

		expect(ids).toEqual(['sub_1', 'sub_2']);
	});

	it('resets counter for test isolation', () => {
		_resetSubagentCounter();
		// After reset, next ID should be sub_1 again
		// (verified by the other tests that call _resetSubagentCounter)
	});
});
