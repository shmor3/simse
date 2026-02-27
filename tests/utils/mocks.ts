// ---------------------------------------------------------------------------
// Shared mock factories — reused across multiple test files
// ---------------------------------------------------------------------------

import { mock } from 'bun:test';
import type { ACPClient } from '../../src/ai/acp/acp-client.js';
import type { ACPGenerateResult } from '../../src/ai/acp/types.js';
import type { StorageBackend } from '../../src/ai/library/storage.js';
import type { Logger } from '../../src/logger.js';
import { createLogger } from '../../src/logger.js';

// ---------------------------------------------------------------------------
// Mock ACP Client
// ---------------------------------------------------------------------------

export function createMockACPClient(
	overrides: Partial<ACPClient> = {},
): ACPClient {
	return {
		initialize: mock((..._: unknown[]): unknown => {}).mockResolvedValue(
			undefined,
		),
		dispose: mock((..._: unknown[]): unknown => {}).mockResolvedValue(
			undefined,
		),
		generate: mock((..._: unknown[]): unknown => {}).mockResolvedValue({
			content: 'acp response',
			agentId: 'default',
			serverName: 'local',
			sessionId: 'session-1',
		} satisfies ACPGenerateResult),
		chat: mock((..._: unknown[]): unknown => {}).mockResolvedValue({
			content: 'chat response',
			agentId: 'default',
			serverName: 'local',
			sessionId: 'session-1',
		} satisfies ACPGenerateResult),
		generateStream: mock(async function* () {
			yield { type: 'delta' as const, text: 'streamed' };
			yield { type: 'complete' as const, usage: undefined };
		}),
		listAgents: mock((..._: unknown[]): unknown => {}).mockResolvedValue([]),
		getAgent: mock((..._: unknown[]): unknown => {}).mockResolvedValue({
			id: 'default',
			name: 'Default Agent',
		}),
		embed: mock((..._: unknown[]): unknown => {}).mockResolvedValue({
			embeddings: [[0.1, 0.2, 0.3]],
			agentId: 'embedding',
			serverName: 'local',
		}),
		isAvailable: mock((..._: unknown[]): unknown => {}).mockResolvedValue(true),
		setPermissionPolicy: mock((..._: unknown[]): unknown => {}),
		listSessions: mock((..._: unknown[]): unknown => {}).mockResolvedValue([]),
		loadSession: mock((..._: unknown[]): unknown => {}).mockResolvedValue({}),
		deleteSession: mock((..._: unknown[]): unknown => {}).mockResolvedValue(
			undefined,
		),
		setSessionMode: mock((..._: unknown[]): unknown => {}).mockResolvedValue(
			undefined,
		),
		setSessionModel: mock((..._: unknown[]): unknown => {}).mockResolvedValue(
			undefined,
		),
		getSessionModels: mock((..._: unknown[]): unknown => {}).mockResolvedValue(
			undefined,
		),
		getSessionModes: mock((..._: unknown[]): unknown => {}).mockResolvedValue(
			undefined,
		),
		getServerHealth: mock((..._: unknown[]): unknown => {}).mockReturnValue(
			undefined,
		),
		getServerModelInfo: mock(
			(..._: unknown[]): unknown => {},
		).mockResolvedValue(undefined),
		getServerStatuses: mock((..._: unknown[]): unknown => {}).mockResolvedValue(
			[],
		),
		serverNames: ['local'],
		serverCount: 1,
		defaultServerName: 'local',
		defaultAgent: 'default',
		...overrides,
	} as unknown as ACPClient;
}

// ---------------------------------------------------------------------------
// In-memory StorageBackend
// ---------------------------------------------------------------------------

export function createMemoryStorage(
	sharedData?: Map<string, Buffer>,
): StorageBackend {
	const data: Map<string, Buffer> = sharedData ?? new Map();

	return Object.freeze({
		load: async () => new Map(data),
		save: async (newData: Map<string, Buffer>) => {
			data.clear();
			for (const [k, v] of newData) {
				data.set(k, v);
			}
		},
		close: async () => {},
	});
}

// ---------------------------------------------------------------------------
// Silent Logger
// ---------------------------------------------------------------------------

export function createSilentLogger(): Logger {
	return createLogger({ context: 'test', level: 'none', transports: [] });
}
