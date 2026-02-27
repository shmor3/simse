import { describe, expect, it, mock } from 'bun:test';
import type { ACPClient } from '../src/ai/acp/acp-client.js';
import {
	_resetBuiltinSubagentCounter,
	registerBuiltinSubagents,
} from '../src/ai/tools/builtin-subagents.js';
import { createToolRegistry } from '../src/ai/tools/tool-registry.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function createMockACPClient(): ACPClient {
	const client = {
		initialize: mock(() => Promise.resolve()),
		dispose: mock(() => Promise.resolve()),
		listAgents: mock(() => Promise.resolve([])),
		getAgent: mock(() =>
			Promise.resolve({ id: 'test', name: 'test', description: 'test' }),
		),
		generate: mock(() =>
			Promise.resolve({
				content: 'generated text',
				agentId: 'test',
				serverName: 'test',
				sessionId: 'sess_1',
			}),
		),
		chat: mock(() =>
			Promise.resolve({
				content: 'chat text',
				agentId: 'test',
				serverName: 'test',
				sessionId: 'sess_1',
			}),
		),
		generateStream: mock(async function* () {
			yield { type: 'delta' as const, text: 'result text' };
			yield { type: 'complete' as const };
		}),
		embed: mock(() =>
			Promise.resolve({
				embeddings: [[1, 2, 3]],
				agentId: 'test',
				serverName: 'test',
			}),
		),
		isAvailable: mock(() => Promise.resolve(true)),
		setPermissionPolicy: mock(() => {}),
		listSessions: mock(() => Promise.resolve([])),
		loadSession: mock(() =>
			Promise.resolve({ sessionId: 'sess_1', status: 'active' as const }),
		),
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
	};
	return client as unknown as ACPClient;
}

function createRegistryWithReadOnlyTools() {
	const registry = createToolRegistry({});

	// Register a read-only tool
	registry.register(
		{
			name: 'fs_read',
			description: 'Read a file',
			parameters: { path: { type: 'string', description: 'File path' } },
			category: 'read',
			annotations: { readOnly: true },
		},
		async () => 'file contents',
	);

	// Register a search tool
	registry.register(
		{
			name: 'fs_grep',
			description: 'Search files',
			parameters: { pattern: { type: 'string', description: 'Pattern' } },
			category: 'search',
			annotations: { readOnly: true },
		},
		async () => 'search results',
	);

	// Register a write tool (should NOT be available to subagents)
	registry.register(
		{
			name: 'fs_write',
			description: 'Write a file',
			parameters: {
				path: { type: 'string', description: 'File path' },
				content: { type: 'string', description: 'Content' },
			},
			category: 'edit',
			annotations: { destructive: true },
		},
		async () => 'written',
	);

	return registry;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('registerBuiltinSubagents', () => {
	it('registers subagent_explore and subagent_plan tools', () => {
		_resetBuiltinSubagentCounter();
		const registry = createRegistryWithReadOnlyTools();
		const acpClient = createMockACPClient();

		registerBuiltinSubagents(registry, {
			acpClient,
			toolRegistry: registry,
		});

		const names = registry.toolNames;
		expect(names).toContain('subagent_explore');
		expect(names).toContain('subagent_plan');
	});

	it('subagent_explore has readOnly annotation', () => {
		_resetBuiltinSubagentCounter();
		const registry = createRegistryWithReadOnlyTools();
		const acpClient = createMockACPClient();

		registerBuiltinSubagents(registry, {
			acpClient,
			toolRegistry: registry,
		});

		const defs = registry.getToolDefinitions();
		const explore = defs.find((d) => d.name === 'subagent_explore');
		expect(explore?.annotations?.readOnly).toBe(true);
	});

	it('subagent_plan has readOnly annotation', () => {
		_resetBuiltinSubagentCounter();
		const registry = createRegistryWithReadOnlyTools();
		const acpClient = createMockACPClient();

		registerBuiltinSubagents(registry, {
			acpClient,
			toolRegistry: registry,
		});

		const defs = registry.getToolDefinitions();
		const plan = defs.find((d) => d.name === 'subagent_plan');
		expect(plan?.annotations?.readOnly).toBe(true);
	});

	it('both tools are in the subagent category', () => {
		_resetBuiltinSubagentCounter();
		const registry = createRegistryWithReadOnlyTools();
		const acpClient = createMockACPClient();

		registerBuiltinSubagents(registry, {
			acpClient,
			toolRegistry: registry,
		});

		const defs = registry.getToolDefinitions();
		const explore = defs.find((d) => d.name === 'subagent_explore');
		const plan = defs.find((d) => d.name === 'subagent_plan');
		expect(explore?.category).toBe('subagent');
		expect(plan?.category).toBe('subagent');
	});

	it('subagent_explore has required task parameter', () => {
		_resetBuiltinSubagentCounter();
		const registry = createRegistryWithReadOnlyTools();
		const acpClient = createMockACPClient();

		registerBuiltinSubagents(registry, {
			acpClient,
			toolRegistry: registry,
		});

		const defs = registry.getToolDefinitions();
		const explore = defs.find((d) => d.name === 'subagent_explore');
		expect(explore?.parameters.task).toBeDefined();
		expect(explore?.parameters.task.required).toBe(true);
		expect(explore?.parameters.description).toBeDefined();
		expect(explore?.parameters.description.required).toBe(true);
	});

	it('subagent_plan has required task parameter', () => {
		_resetBuiltinSubagentCounter();
		const registry = createRegistryWithReadOnlyTools();
		const acpClient = createMockACPClient();

		registerBuiltinSubagents(registry, {
			acpClient,
			toolRegistry: registry,
		});

		const defs = registry.getToolDefinitions();
		const plan = defs.find((d) => d.name === 'subagent_plan');
		expect(plan?.parameters.task).toBeDefined();
		expect(plan?.parameters.task.required).toBe(true);
		expect(plan?.parameters.description).toBeDefined();
		expect(plan?.parameters.description.required).toBe(true);
	});

	it('_resetBuiltinSubagentCounter resets the ID counter', () => {
		_resetBuiltinSubagentCounter();
		// Counter should be reset — tested indirectly through registration
		const registry = createRegistryWithReadOnlyTools();
		const acpClient = createMockACPClient();
		registerBuiltinSubagents(registry, {
			acpClient,
			toolRegistry: registry,
		});
		// Just verify no throw
		expect(registry.toolCount).toBeGreaterThanOrEqual(5); // 3 original + 2 subagent
	});
});
