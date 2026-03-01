// ---------------------------------------------------------------------------
// Agent Client Protocol (ACP) Client — thin wrapper over Rust engine
// ---------------------------------------------------------------------------

import {
	createEmbeddingError,
	createProviderGenerationError,
	createProviderUnavailableError,
	isSimseError,
	toError,
} from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import type { HealthSnapshot } from '../../utils/health-monitor.js';
import {
	type AcpEngineClient,
	createAcpEngineClient,
} from './acp-engine-client.js';
import type {
	ACPAgentInfo,
	ACPConfig,
	ACPEmbedResult,
	ACPGenerateResult,
	ACPModelsInfo,
	ACPModesInfo,
	ACPPermissionPolicy,
	ACPSamplingParams,
	ACPServerStatus,
	ACPSessionInfo,
	ACPSessionListEntry,
	ACPStreamChunk,
	ACPTokenUsage,
	ACPToolCall,
	ACPToolCallUpdate,
} from './types.js';

// ---------------------------------------------------------------------------
// Permission request info (previously in acp-connection.ts)
// ---------------------------------------------------------------------------

/**
 * A permission option presented by the ACP agent.
 */
export interface ACPPermissionOption {
	readonly optionId: string;
	readonly kind: string;
	/** ACP spec field — human-readable label for the option. */
	readonly name?: string;
	/** @deprecated Use `name` — kept for backwards compat with older servers. */
	readonly title?: string;
	readonly description?: string;
}

/**
 * Tool call details attached to a permission request.
 */
export interface ACPPermissionToolCall {
	readonly toolCallId?: string;
	readonly title?: string;
	readonly kind?: string;
	readonly rawInput?: unknown;
	readonly status?: string;
}

/**
 * Info passed to the permission handler callback in 'prompt' mode.
 */
export interface ACPPermissionRequestInfo {
	readonly title?: string;
	readonly description?: string;
	readonly toolCall?: ACPPermissionToolCall;
	readonly options: readonly ACPPermissionOption[];
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ACPClientOptions {
	/** Override the default retry configuration. */
	retryOptions?: {
		maxAttempts?: number;
		baseDelayMs?: number;
		maxDelayMs?: number;
	};
	/** Timeout for streaming requests in milliseconds. Defaults to `120000`. */
	streamTimeoutMs?: number;
	/** Inject a custom logger (defaults to the global logger). */
	logger?: Logger;
	/** Client name advertised during ACP initialize. Defaults to 'simse'. */
	clientName?: string;
	/** Client version advertised during ACP initialize. Defaults to '1.0.0'. */
	clientVersion?: string;
	/** Circuit breaker configuration for per-server failure handling. */
	circuitBreaker?: {
		failureThreshold?: number;
		resetTimeoutMs?: number;
	};
	/**
	 * Called when an ACP agent requests permission and the policy is 'prompt'.
	 * Return the selected optionId, or undefined to reject.
	 */
	onPermissionRequest?: (
		info: ACPPermissionRequestInfo,
	) => Promise<string | undefined>;
	/** Path to the ACP engine binary. Required. */
	enginePath?: string;
}

export interface ACPStreamOptions {
	readonly agentId?: string;
	readonly serverName?: string;
	readonly systemPrompt?: string;
	readonly config?: Record<string, unknown>;
	readonly sampling?: ACPSamplingParams;
	readonly onToolCall?: (toolCall: ACPToolCall) => void;
	readonly onToolCallUpdate?: (update: ACPToolCallUpdate) => void;
	/** AbortSignal to cancel the stream early. Stops yielding chunks when aborted. */
	readonly signal?: AbortSignal;
	/** Image content blocks to include alongside the text prompt. */
	readonly images?: readonly {
		readonly mimeType: string;
		readonly base64: string;
	}[];
}

// ---------------------------------------------------------------------------
// ACPClient interface
// ---------------------------------------------------------------------------

export interface ACPClient {
	/** Spawn all servers and initialize ACP connections. */
	readonly initialize: () => Promise<void>;
	/** Close all ACP connections and kill spawned processes. */
	readonly dispose: () => Promise<void>;
	readonly listAgents: (serverName?: string) => Promise<ACPAgentInfo[]>;
	readonly getAgent: (
		agentId: string,
		serverName?: string,
	) => Promise<ACPAgentInfo>;
	readonly generate: (
		prompt: string,
		options?: {
			agentId?: string;
			serverName?: string;
			systemPrompt?: string;
			config?: Record<string, unknown>;
			sampling?: ACPSamplingParams;
			modelId?: string;
		},
	) => Promise<ACPGenerateResult>;
	readonly chat: (
		messages: Array<{
			role: 'system' | 'user' | 'assistant';
			content: string;
		}>,
		options?: {
			agentId?: string;
			serverName?: string;
			config?: Record<string, unknown>;
			sampling?: ACPSamplingParams;
		},
	) => Promise<ACPGenerateResult>;
	readonly generateStream: (
		prompt: string,
		options?: ACPStreamOptions,
	) => AsyncGenerator<ACPStreamChunk>;
	readonly embed: (
		input: string | string[],
		model?: string,
		serverName?: string,
	) => Promise<ACPEmbedResult>;
	readonly isAvailable: (serverName?: string) => Promise<boolean>;
	/** Set the permission policy on all active connections. */
	readonly setPermissionPolicy: (policy: ACPPermissionPolicy) => void;
	readonly listSessions: (
		serverName?: string,
	) => Promise<ACPSessionListEntry[]>;
	readonly loadSession: (
		sessionId: string,
		serverName?: string,
	) => Promise<ACPSessionInfo>;
	readonly deleteSession: (
		sessionId: string,
		serverName?: string,
	) => Promise<void>;
	readonly setSessionMode: (
		sessionId: string,
		modeId: string,
		serverName?: string,
	) => Promise<void>;
	readonly setSessionModel: (
		sessionId: string,
		modelId: string,
		serverName?: string,
	) => Promise<void>;
	readonly getSessionModels: (
		sessionId: string,
		serverName?: string,
	) => Promise<ACPModelsInfo | undefined>;
	readonly getSessionModes: (
		sessionId: string,
		serverName?: string,
	) => Promise<ACPModesInfo | undefined>;
	readonly getServerHealth: (serverName?: string) => HealthSnapshot | undefined;
	/** Get model info for a specific server by creating a temp session. */
	readonly getServerModelInfo: (
		serverName?: string,
	) => Promise<ACPModelsInfo | undefined>;
	/** Get status of all configured servers (connectivity, model, agent). */
	readonly getServerStatuses: () => Promise<readonly ACPServerStatus[]>;
	readonly serverNames: string[];
	readonly serverCount: number;
	readonly defaultServerName: string | undefined;
	readonly defaultAgent: string | undefined;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

/**
 * Create an ACP client that delegates to the Rust ACP engine via
 * JSON-RPC 2.0 / NDJSON stdio.
 *
 * @param config - ACP server definitions (command, args, env per server).
 * @param options - Engine path, retry policy, stream timeout, circuit breaker thresholds.
 * @returns A frozen {@link ACPClient} with generate, chat, embed, and session management.
 * @throws {ProviderUnavailableError} When the engine binary cannot be spawned.
 */
export function createACPClient(
	config: ACPConfig,
	options?: ACPClientOptions,
): ACPClient {
	const logger = (options?.logger ?? getDefaultLogger()).child('acp');
	const streamTimeoutMs = options?.streamTimeoutMs ?? 120_000;
	const enginePath = options?.enginePath ?? 'simse-acp-engine';

	let engineClient: AcpEngineClient | undefined;

	// Session info cache — stores models/modes from session/new and loadSession
	const sessionInfoCache = new Map<string, ACPSessionInfo>();

	// -----------------------------------------------------------------------
	// Initialize / dispose
	// -----------------------------------------------------------------------

	const initialize = async (): Promise<void> => {
		if (engineClient?.isHealthy) return;

		logger.info('Spawning ACP engine process');

		engineClient = createAcpEngineClient({
			enginePath,
			timeoutMs: streamTimeoutMs,
			logger,
		});

		// Register permission notification handler
		if (options?.onPermissionRequest) {
			const handler = options.onPermissionRequest;
			engineClient.onNotification('permission/request', (params: unknown) => {
				const p = params as {
					requestId: number;
					title?: string;
					description?: string;
					toolCall?: ACPPermissionToolCall;
					options?: readonly ACPPermissionOption[];
				};

				handler({
					title: p?.title,
					description: p?.description,
					toolCall: p?.toolCall,
					options: p?.options ?? [],
				})
					.then((selectedId) => {
						if (selectedId) {
							engineClient
								?.request('acp/permissionResponse', {
									requestId: p.requestId,
									optionId: selectedId,
								})
								.catch(() => {});
						}
					})
					.catch(() => {});
			});
		}

		// Build server config for the Rust engine
		const servers = config.servers.map((entry) => ({
			name: entry.name,
			command: entry.command,
			args: entry.args ? [...entry.args] : undefined,
			cwd: entry.cwd,
			env: entry.env ? { ...entry.env } : undefined,
			defaultAgent: entry.defaultAgent,
			timeoutMs: entry.timeoutMs,
			permissionPolicy: entry.permissionPolicy,
		}));

		const mcpServers = config.mcpServers?.map((mcp) => ({
			name: mcp.name,
			config: {
				command: mcp.command,
				args: mcp.args ? [...mcp.args] : undefined,
				env: mcp.env ? { ...mcp.env } : undefined,
			},
		}));

		try {
			await engineClient.request('acp/initialize', {
				servers,
				defaultServer: config.defaultServer,
				defaultAgent: config.defaultAgent,
				mcpServers,
			});

			logger.info('ACP engine initialized');
		} catch (error) {
			logger.error('ACP engine initialization failed', toError(error));
			throw createProviderUnavailableError('acp', {
				cause: error,
				metadata: {
					reason: `ACP engine initialization failed: ${toError(error).message}`,
				},
			});
		}
	};

	const dispose = async (): Promise<void> => {
		if (!engineClient) return;

		logger.debug('Disposing ACP engine');

		try {
			await engineClient.request('acp/dispose');
		} catch {
			// Best-effort cleanup
		}

		await engineClient.dispose();
		engineClient = undefined;
		sessionInfoCache.clear();
	};

	// -----------------------------------------------------------------------
	// Require engine
	// -----------------------------------------------------------------------

	const requireEngine = (): AcpEngineClient => {
		if (!engineClient?.isHealthy) {
			throw createProviderUnavailableError('acp', {
				metadata: {
					reason: 'ACP engine is not connected. Call initialize() first.',
				},
			});
		}
		return engineClient;
	};

	// -----------------------------------------------------------------------
	// Public methods
	// -----------------------------------------------------------------------

	const listAgents = async (serverName?: string): Promise<ACPAgentInfo[]> => {
		const engine = requireEngine();
		const result = await engine.request<{ agents: ACPAgentInfo[] }>(
			'acp/listAgents',
			{ server: serverName },
		);
		return result.agents ?? [];
	};

	const getAgent = async (
		agentId: string,
		serverName?: string,
	): Promise<ACPAgentInfo> => {
		const agents = await listAgents(serverName);
		const agent = agents.find((a) => a.id === agentId);
		if (!agent) {
			throw createProviderGenerationError(
				'acp',
				`Agent "${agentId}" not found`,
			);
		}
		return agent;
	};

	const generate = async (
		prompt: string,
		generateOptions?: {
			agentId?: string;
			serverName?: string;
			systemPrompt?: string;
			config?: Record<string, unknown>;
			sampling?: ACPSamplingParams;
			modelId?: string;
		},
	): Promise<ACPGenerateResult> => {
		const engine = requireEngine();

		logger.debug('Starting generate request', {
			promptLength: prompt.length,
			hasSystemPrompt: !!generateOptions?.systemPrompt,
		});

		const result = await engine.request<ACPGenerateResult>('acp/generate', {
			prompt,
			agentId: generateOptions?.agentId,
			serverName: generateOptions?.serverName,
			systemPrompt: generateOptions?.systemPrompt,
			sampling: generateOptions?.sampling,
			sessionId: undefined,
		});

		return result;
	};

	const chat = async (
		messages: Array<{
			role: 'system' | 'user' | 'assistant';
			content: string;
		}>,
		chatOptions?: {
			agentId?: string;
			serverName?: string;
			config?: Record<string, unknown>;
			sampling?: ACPSamplingParams;
		},
	): Promise<ACPGenerateResult> => {
		if (messages.length === 0) {
			throw createProviderGenerationError(
				'acp',
				'Cannot send an empty message list to ACP chat',
			);
		}

		const engine = requireEngine();

		logger.debug('Starting chat request', {
			messageCount: messages.length,
		});

		// Send messages as content blocks for each role
		const chatMessages = messages.map((msg) => ({
			role: msg.role,
			content: [{ type: 'text' as const, text: msg.content }],
		}));

		const result = await engine.request<ACPGenerateResult>('acp/chat', {
			messages: chatMessages,
			agentId: chatOptions?.agentId,
			serverName: chatOptions?.serverName,
			sampling: chatOptions?.sampling,
			sessionId: undefined,
		});

		return result;
	};

	async function* generateStream(
		prompt: string,
		streamOptions?: ACPStreamOptions,
	): AsyncGenerator<ACPStreamChunk> {
		const engine = requireEngine();

		logger.debug('Starting generate stream', {
			promptLength: prompt.length,
		});

		// Start the stream — returns a stream ID
		const { streamId } = await engine.request<{ streamId: string }>(
			'acp/streamStart',
			{
				prompt,
				agentId: streamOptions?.agentId,
				serverName: streamOptions?.serverName,
				systemPrompt: streamOptions?.systemPrompt,
				sampling: streamOptions?.sampling,
				sessionId: undefined,
				streamTimeoutMs: streamTimeoutMs,
			},
		);

		// Collect streaming chunks via notification handlers
		type ChunkItem =
			| { text: string }
			| { done: true; usage?: ACPTokenUsage }
			| { toolCall: ACPToolCall }
			| { toolCallUpdate: ACPToolCallUpdate };

		const chunks: ChunkItem[] = [];
		let chunkResolve: (() => void) | undefined;

		const unsubDelta = engine.onNotification(
			'stream/delta',
			(params: unknown) => {
				const p = params as { streamId: string; text: string };
				if (p.streamId !== streamId) return;
				chunks.push({ text: p.text });
				chunkResolve?.();
			},
		);

		const unsubToolCall = engine.onNotification(
			'stream/toolCall',
			(params: unknown) => {
				const p = params as { streamId: string; toolCall: ACPToolCall };
				if (p.streamId !== streamId) return;
				streamOptions?.onToolCall?.(p.toolCall);
				chunks.push({ toolCall: p.toolCall });
				chunkResolve?.();
			},
		);

		const unsubToolCallUpdate = engine.onNotification(
			'stream/toolCallUpdate',
			(params: unknown) => {
				const p = params as {
					streamId: string;
					update: ACPToolCallUpdate;
				};
				if (p.streamId !== streamId) return;
				streamOptions?.onToolCallUpdate?.(p.update);
				chunks.push({ toolCallUpdate: p.update });
				chunkResolve?.();
			},
		);

		const unsubComplete = engine.onNotification(
			'stream/complete',
			(params: unknown) => {
				const p = params as {
					streamId: string;
					usage?: ACPTokenUsage;
				};
				if (p.streamId !== streamId) return;
				chunks.push({ done: true, usage: p.usage });
				chunkResolve?.();
			},
		);

		try {
			// Sliding-window timeout — resets each time a chunk arrives
			const timeoutDuration = streamTimeoutMs;
			let lastActivity = Date.now();
			let idx = 0;

			while (true) {
				// Check abort signal
				if (streamOptions?.signal?.aborted) {
					yield { type: 'complete', usage: undefined };
					return;
				}

				if (idx < chunks.length) {
					const chunk = chunks[idx++];
					lastActivity = Date.now();

					if ('done' in chunk) {
						yield { type: 'complete', usage: chunk.usage };
						return;
					}
					if ('toolCall' in chunk) {
						yield { type: 'tool_call', toolCall: chunk.toolCall };
						continue;
					}
					if ('toolCallUpdate' in chunk) {
						yield {
							type: 'tool_call_update',
							update: chunk.toolCallUpdate,
						};
						continue;
					}
					yield { type: 'delta', text: chunk.text };
				} else {
					// Check abort before waiting
					if (streamOptions?.signal?.aborted) {
						yield { type: 'complete', usage: undefined };
						return;
					}

					const elapsed = Date.now() - lastActivity;
					if (elapsed >= timeoutDuration) {
						throw createProviderGenerationError(
							'acp',
							`Stream timed out after ${timeoutDuration}ms of inactivity`,
						);
					}

					const remaining = timeoutDuration - elapsed;
					await new Promise<void>((resolve) => {
						chunkResolve = resolve;
						setTimeout(resolve, Math.min(remaining, 100));
					});
				}
			}
		} catch (error) {
			if (isSimseError(error)) throw error;
			throw createProviderGenerationError(
				'acp',
				`Stream interrupted: ${toError(error).message}`,
				{ cause: error },
			);
		} finally {
			unsubDelta();
			unsubToolCall();
			unsubToolCallUpdate();
			unsubComplete();
		}
	}

	const embed = async (
		input: string | string[],
		model?: string,
		serverName?: string,
	): Promise<ACPEmbedResult> => {
		const engine = requireEngine();
		const texts = Array.isArray(input) ? input : [input];

		logger.debug('Generating embeddings via ACP engine', {
			inputCount: texts.length,
		});

		try {
			const result = await engine.request<ACPEmbedResult>('acp/embed', {
				input: texts,
				model,
				server: serverName,
			});
			return result;
		} catch (error) {
			if (isSimseError(error)) throw error;
			throw createEmbeddingError(
				`ACP embedding failed: ${toError(error).message}`,
				{ cause: error },
			);
		}
	};

	const listSessions = async (
		serverName?: string,
	): Promise<ACPSessionListEntry[]> => {
		const engine = requireEngine();
		const result = await engine.request<{
			sessions: ACPSessionListEntry[];
		}>('acp/listSessions', { server: serverName });
		return result.sessions ?? [];
	};

	const loadSession = async (
		sessionId: string,
		serverName?: string,
	): Promise<ACPSessionInfo> => {
		const engine = requireEngine();
		const info = await engine.request<ACPSessionInfo>('acp/loadSession', {
			sessionId,
			server: serverName,
		});
		sessionInfoCache.set(sessionId, info);
		return info;
	};

	const deleteSession = async (
		sessionId: string,
		serverName?: string,
	): Promise<void> => {
		const engine = requireEngine();
		await engine.request('acp/deleteSession', {
			sessionId,
			server: serverName,
		});
		sessionInfoCache.delete(sessionId);
	};

	const setSessionMode = async (
		sessionId: string,
		modeId: string,
		serverName?: string,
	): Promise<void> => {
		const engine = requireEngine();
		await engine
			.request('acp/setSessionMode', {
				sessionId,
				value: modeId,
				server: serverName,
			})
			.catch(() => {}); // Best-effort
	};

	const setSessionModel = async (
		sessionId: string,
		modelId: string,
		serverName?: string,
	): Promise<void> => {
		const engine = requireEngine();
		await engine
			.request('acp/setSessionModel', {
				sessionId,
				value: modelId,
				server: serverName,
			})
			.catch(() => {}); // Best-effort
	};

	const isAvailable = async (serverName?: string): Promise<boolean> => {
		try {
			const engine = requireEngine();
			const result = await engine.request<{ available: boolean }>(
				'acp/serverHealth',
				{ server: serverName },
			);
			return result.available;
		} catch {
			return false;
		}
	};

	const getServerHealth = (
		_serverName?: string,
	): HealthSnapshot | undefined => {
		// Health monitoring is now in Rust — return undefined for TS consumers
		// that check this. The engine handles resilience internally.
		return undefined;
	};

	const getSessionModels = async (
		sessionId: string,
		_serverName?: string,
	): Promise<ACPModelsInfo | undefined> => {
		return sessionInfoCache.get(sessionId)?.models;
	};

	const getSessionModes = async (
		sessionId: string,
		_serverName?: string,
	): Promise<ACPModesInfo | undefined> => {
		return sessionInfoCache.get(sessionId)?.modes;
	};

	const setPermissionPolicy = (policy: ACPPermissionPolicy): void => {
		if (!engineClient?.isHealthy) return;
		engineClient.request('acp/setPermissionPolicy', { policy }).catch(() => {});
	};

	const getServerModelInfo = async (
		serverName?: string,
	): Promise<ACPModelsInfo | undefined> => {
		// The Rust engine doesn't expose a direct getServerModelInfo method.
		// We can approximate by listing agents or checking cached session info.
		// For now, return undefined — this is a best-effort method.
		try {
			const engine = requireEngine();
			// Try to load via the engine's server health which returns serverNames
			const result = await engine.request<{
				available: boolean;
				serverNames: string[];
			}>('acp/serverHealth', { server: serverName });
			if (!result.available) return undefined;
			return undefined;
		} catch {
			return undefined;
		}
	};

	const getServerStatuses = async (): Promise<readonly ACPServerStatus[]> => {
		const statuses: ACPServerStatus[] = [];

		for (const entry of config.servers) {
			let connected = false;

			try {
				const engine = requireEngine();
				const result = await engine.request<{ available: boolean }>(
					'acp/serverHealth',
					{ server: entry.name },
				);
				connected = result.available;
			} catch {
				connected = false;
			}

			statuses.push(
				Object.freeze({
					name: entry.name,
					connected,
					currentModel: undefined,
					availableModels: undefined,
					agentId: entry.defaultAgent ?? config.defaultAgent,
				}),
			);
		}

		return Object.freeze(statuses);
	};

	// -----------------------------------------------------------------------
	// Return the frozen record
	// -----------------------------------------------------------------------

	return Object.freeze({
		initialize,
		dispose,
		listAgents,
		getAgent,
		generate,
		chat,
		generateStream,
		embed,
		isAvailable,
		setPermissionPolicy,
		getSessionModels,
		getSessionModes,
		getServerHealth,
		getServerModelInfo,
		getServerStatuses,
		listSessions,
		loadSession,
		deleteSession,
		setSessionMode,
		setSessionModel,
		get serverNames() {
			return config.servers.map((s) => s.name);
		},
		get serverCount() {
			return config.servers.length;
		},
		get defaultServerName() {
			return config.defaultServer ?? config.servers[0]?.name;
		},
		get defaultAgent() {
			return config.defaultAgent;
		},
	});
}
