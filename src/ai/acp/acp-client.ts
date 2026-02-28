// ---------------------------------------------------------------------------
// Agent Client Protocol (ACP) Client — JSON-RPC 2.0 over stdio
// ---------------------------------------------------------------------------

import {
	createEmbeddingError,
	createProviderGenerationError,
	createProviderUnavailableError,
	isSimseError,
	toError,
} from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import {
	type CircuitBreaker,
	createCircuitBreaker,
} from '../../utils/circuit-breaker.js';
import {
	createHealthMonitor,
	type HealthMonitor,
	type HealthSnapshot,
} from '../../utils/health-monitor.js';
import { isTransientError, retry } from '../../utils/retry.js';
import { type ACPConnection, createACPConnection } from './acp-connection.js';
import { extractContentText, extractTokenUsage } from './acp-results.js';
import type {
	ACPAgentInfo,
	ACPConfig,
	ACPContentBlock,
	ACPEmbedResult,
	ACPGenerateResult,
	ACPModelInfo,
	ACPModelsInfo,
	ACPModesInfo,
	ACPPermissionPolicy,
	ACPSamplingParams,
	ACPServerEntry,
	ACPServerStatus,
	ACPSessionInfo,
	ACPSessionListEntry,
	ACPSessionNewResult,
	ACPSessionPromptResult,
	ACPSessionUpdateParams,
	ACPStreamChunk,
	ACPTokenUsage,
	ACPToolCall,
	ACPToolCallUpdate,
} from './types.js';

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
		info: import('./acp-connection.js').ACPPermissionRequestInfo,
	) => Promise<string | undefined>;
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
 * Create an ACP client that communicates with one or more ACP-compatible
 * agent servers over JSON-RPC 2.0 / NDJSON stdio.
 *
 * @param config - ACP server definitions (command, args, env per server).
 * @param options - Retry policy, stream timeout, circuit breaker thresholds.
 * @returns A frozen {@link ACPClient} with generate, chat, embed, and session management.
 * @throws {ProviderUnavailableError} When the target server process cannot be spawned.
 */
export function createACPClient(
	config: ACPConfig,
	options?: ACPClientOptions,
): ACPClient {
	const logger = (options?.logger ?? getDefaultLogger()).child('acp');

	const maxRetryAttempts = options?.retryOptions?.maxAttempts ?? 3;
	const retryBaseDelayMs = options?.retryOptions?.baseDelayMs ?? 500;
	const retryMaxDelayMs = options?.retryOptions?.maxDelayMs ?? 15_000;
	const streamTimeoutMs = options?.streamTimeoutMs ?? 120_000;
	const clientName = options?.clientName ?? 'simse';
	const clientVersion = options?.clientVersion ?? '1.0.0';

	// Connection pool — one connection per configured server
	const connections = new Map<string, ACPConnection>();

	// Per-server circuit breakers and health monitors
	const breakers = new Map<string, CircuitBreaker>();
	const monitors = new Map<string, HealthMonitor>();

	// Session info cache — stores models/modes from session/new and loadSession
	const sessionInfoCache = new Map<string, ACPSessionInfo>();

	// -----------------------------------------------------------------------
	// Initialize / dispose
	// -----------------------------------------------------------------------

	const initialize = async (): Promise<void> => {
		const results = await Promise.allSettled(
			config.servers.map(async (entry) => {
				if (connections.has(entry.name)) return;

				logger.info(
					`Connecting to ACP server "${entry.name}": ${entry.command} ${(entry.args ?? []).join(' ')}`,
				);

				const connection = createACPConnection({
					command: entry.command,
					args: entry.args,
					cwd: entry.cwd,
					env: entry.env,
					timeoutMs: entry.timeoutMs,
					permissionPolicy: entry.permissionPolicy,
					clientName,
					clientVersion,
					stderrHandler: (text) => {
						logger.debug(`ACP server "${entry.name}" stderr: ${text}`);
					},
					onPermissionRequest: options?.onPermissionRequest,
				});

				const result = await connection.initialize();
				connections.set(entry.name, connection);

				// Create circuit breaker and health monitor for this server
				if (options?.circuitBreaker) {
					breakers.set(
						entry.name,
						createCircuitBreaker({
							name: `acp:${entry.name}`,
							failureThreshold: options.circuitBreaker.failureThreshold,
							resetTimeoutMs: options.circuitBreaker.resetTimeoutMs,
							shouldCount: isTransientError,
							onStateChange: (from, to) => {
								logger.warn(
									`Circuit breaker for ACP server "${entry.name}": ${from} → ${to}`,
								);
							},
						}),
					);
				}
				monitors.set(entry.name, createHealthMonitor());

				const info = result.agentInfo;
				logger.info(
					`ACP server "${entry.name}" initialized${info ? `: ${info.name} v${info.version}` : ''}`,
				);
			}),
		);

		for (let i = 0; i < results.length; i++) {
			const result = results[i];
			if (result.status === 'rejected') {
				const name = config.servers[i].name;
				logger.warn(
					`ACP server "${name}" failed to initialize: ${toError(result.reason).message}`,
				);
			}
		}

		if (connections.size === 0) {
			throw createProviderUnavailableError('acp', {
				metadata: {
					reason: 'All ACP servers failed to initialize',
					servers: config.servers.map((s) => s.name),
				},
			});
		}
	};

	const dispose = async (): Promise<void> => {
		const closePromises = [...connections.entries()].map(
			async ([name, conn]) => {
				logger.debug(`Closing ACP connection "${name}"`);
				await conn.close();
			},
		);
		await Promise.all(closePromises);
		connections.clear();
	};

	// -----------------------------------------------------------------------
	// Internal helpers
	// -----------------------------------------------------------------------

	const resolveConnection = (
		serverName?: string,
	): {
		connection: ACPConnection;
		entry: ACPServerEntry;
		name: string;
	} => {
		const name = serverName ?? config.defaultServer ?? config.servers[0]?.name;

		if (!name) {
			throw createProviderUnavailableError('acp', {
				metadata: { reason: 'No ACP servers configured' },
			});
		}

		const connection = connections.get(name);
		if (!connection) {
			throw createProviderUnavailableError('acp', {
				metadata: {
					reason: `ACP server "${name}" is not connected. Call initialize() first.`,
					configuredServers: config.servers.map((s) => s.name),
				},
			});
		}

		const entry = config.servers.find((s) => s.name === name) as ACPServerEntry;
		return { connection, entry, name };
	};

	const resolveAgentId = (
		entry: ACPServerEntry,
		stepAgentId?: string,
	): string => {
		// Resolution order: step → server default → global default → server name
		// The server name is the ultimate fallback so a single-server config
		// "just works" without requiring an explicit agent ID anywhere.
		return (
			stepAgentId ?? entry.defaultAgent ?? config.defaultAgent ?? entry.name
		);
	};

	const withResilience = async <T>(
		sName: string,
		operation: string,
		fn: () => Promise<T>,
	): Promise<T> => {
		const breaker = breakers.get(sName);
		const monitor = monitors.get(sName);

		const retryFn = () =>
			retry(
				async (attempt) => {
					if (attempt > 1) {
						logger.debug(
							`Retrying "${operation}" (attempt ${attempt}/${maxRetryAttempts})`,
						);
						// Check connection health before retry — stale connections
						// can persist even after increasing timeout defaults
						const conn = connections.get(sName);
						if (conn && !conn.isHealthy) {
							logger.warn(
								`Connection to "${sName}" is unhealthy before retry, reconnecting`,
							);
							await conn.close();
							connections.delete(sName);
							const entry = config.servers.find((s) => s.name === sName);
							if (entry) {
								const fresh = createACPConnection({
									command: entry.command,
									args: entry.args,
									cwd: entry.cwd,
									env: entry.env,
									timeoutMs: entry.timeoutMs,
									permissionPolicy: entry.permissionPolicy,
									clientName,
									clientVersion,
									stderrHandler: (text) => {
										logger.debug(`ACP server "${entry.name}" stderr: ${text}`);
									},
									onPermissionRequest: options?.onPermissionRequest,
								});
								await fresh.initialize();
								connections.set(sName, fresh);
							}
						}
					}
					return fn();
				},
				{
					maxAttempts: maxRetryAttempts,
					baseDelayMs: retryBaseDelayMs,
					maxDelayMs: retryMaxDelayMs,
					shouldRetry: (error) => isTransientError(error),
					onRetry: (error, nextAttempt, delayMs) => {
						logger.warn(
							`Operation "${operation}" failed, retrying in ${delayMs}ms (attempt ${nextAttempt})`,
							{ error: toError(error).message },
						);
					},
				},
			);

		try {
			const result = breaker ? await breaker.execute(retryFn) : await retryFn();
			monitor?.recordSuccess();
			return result;
		} catch (error) {
			monitor?.recordFailure(error instanceof Error ? error : undefined);
			throw error;
		}
	};

	// -----------------------------------------------------------------------
	// ACP session helpers
	// -----------------------------------------------------------------------

	const createSession = async (connection: ACPConnection): Promise<string> => {
		const result = await connection.request<ACPSessionNewResult>(
			'session/new',
			{
				cwd: process.cwd(),
				mcpServers: config.mcpServers ?? [],
			},
		);

		// Cache session info (models/modes) for later retrieval
		sessionInfoCache.set(result.sessionId, result);

		// Set the permission mode based on the connection's policy.
		// This tells the agent how to handle tool permissions for this session.
		const modeId =
			connection.permissionPolicy === 'auto-approve'
				? 'bypassPermissions'
				: connection.permissionPolicy === 'deny'
					? 'plan'
					: 'default';

		// Fire-and-forget — don't block session creation if the agent
		// doesn't support mode switching (it's best-effort).
		connection
			.request('session/set_config_option', {
				sessionId: result.sessionId,
				configOptionId: 'mode',
				groupId: modeId,
			})
			.catch(() => {
				// Agent may not support config options — that's fine,
				// permissions still fall back to session/request_permission.
			});

		return result.sessionId;
	};

	const sendPrompt = async (
		connection: ACPConnection,
		sessionId: string,
		content: readonly ACPContentBlock[],
		metadata?: Record<string, unknown>,
		promptTimeoutMs?: number,
	): Promise<ACPSessionPromptResult> => {
		return connection.request<ACPSessionPromptResult>(
			'session/prompt',
			{
				sessionId,
				prompt: content,
				...(metadata && { metadata }),
			},
			promptTimeoutMs,
		);
	};

	// -----------------------------------------------------------------------
	// Build content blocks
	// -----------------------------------------------------------------------

	const buildTextContent = (
		prompt: string,
		systemPrompt?: string,
	): ACPContentBlock[] => {
		const blocks: ACPContentBlock[] = [];
		if (systemPrompt) {
			blocks.push({ type: 'text', text: systemPrompt });
		}
		blocks.push({ type: 'text', text: prompt });
		return blocks;
	};

	const buildSamplingMetadata = (
		sampling?: ACPSamplingParams,
	): Record<string, unknown> | undefined => {
		if (!sampling) return undefined;
		const meta: Record<string, unknown> = {};
		if (sampling.temperature !== undefined)
			meta.temperature = sampling.temperature;
		if (sampling.maxTokens !== undefined) meta.max_tokens = sampling.maxTokens;
		if (sampling.topP !== undefined) meta.top_p = sampling.topP;
		if (sampling.topK !== undefined) meta.top_k = sampling.topK;
		if (sampling.stopSequences !== undefined)
			meta.stop_sequences = sampling.stopSequences;
		return Object.keys(meta).length > 0 ? meta : undefined;
	};

	// -----------------------------------------------------------------------
	// Public methods
	// -----------------------------------------------------------------------

	const listAgents = async (serverName?: string): Promise<ACPAgentInfo[]> => {
		// Native ACP has no agent listing — return synthetic info from config
		if (serverName) {
			const entry = config.servers.find((s) => s.name === serverName);
			if (!entry) {
				throw createProviderUnavailableError('acp', {
					metadata: {
						reason: `ACP server "${serverName}" is not configured`,
					},
				});
			}
			return [
				{
					id: entry.defaultAgent ?? entry.name,
					name: entry.name,
					description: `ACP agent on server "${entry.name}"`,
				},
			];
		}

		return config.servers.map((entry) => ({
			id: entry.defaultAgent ?? entry.name,
			name: entry.name,
			description: `ACP agent on server "${entry.name}"`,
		}));
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
		const { connection, entry, name } = resolveConnection(
			generateOptions?.serverName,
		);
		const agentId = resolveAgentId(entry, generateOptions?.agentId);

		logger.debug('Starting generate request', {
			server: name,
			agent: agentId,
			promptLength: prompt.length,
			hasSystemPrompt: !!generateOptions?.systemPrompt,
		});

		return withResilience(name, 'generate', async () => {
			const sessionId = await createSession(connection);
			if (generateOptions?.modelId) {
				await setSessionModel(sessionId, generateOptions.modelId, name);
			}
			const content = buildTextContent(prompt, generateOptions?.systemPrompt);
			const result = await sendPrompt(
				connection,
				sessionId,
				content,
				buildSamplingMetadata(generateOptions?.sampling),
				streamTimeoutMs,
			);

			return {
				content: extractContentText(result.content),
				agentId,
				serverName: name,
				sessionId,
				usage: extractTokenUsage(result.metadata),
				stopReason: result.stopReason,
			};
		});
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

		const { connection, entry, name } = resolveConnection(
			chatOptions?.serverName,
		);
		const agentId = resolveAgentId(entry, chatOptions?.agentId);

		logger.debug('Starting chat request', {
			server: name,
			agent: agentId,
			messageCount: messages.length,
		});

		return withResilience(name, 'chat', async () => {
			const sessionId = await createSession(connection);

			// Combine all messages into content blocks for a single prompt.
			// System messages become prefixed text blocks, assistant messages
			// are included as context.
			const content: ACPContentBlock[] = [];
			for (const msg of messages) {
				const prefix =
					msg.role === 'system'
						? '[System] '
						: msg.role === 'assistant'
							? '[Assistant] '
							: '';
				content.push({ type: 'text', text: `${prefix}${msg.content}` });
			}

			const result = await sendPrompt(
				connection,
				sessionId,
				content,
				buildSamplingMetadata(chatOptions?.sampling),
				streamTimeoutMs,
			);

			return {
				content: extractContentText(result.content),
				agentId,
				serverName: name,
				sessionId,
				usage: extractTokenUsage(result.metadata),
				stopReason: result.stopReason,
			};
		});
	};

	async function* generateStream(
		prompt: string,
		streamOptions?: ACPStreamOptions,
	): AsyncGenerator<ACPStreamChunk> {
		const { connection, entry, name } = resolveConnection(
			streamOptions?.serverName,
		);
		const agentId = resolveAgentId(entry, streamOptions?.agentId);
		const monitor = monitors.get(name);

		logger.debug('Starting generate stream', {
			server: name,
			agent: agentId,
			promptLength: prompt.length,
		});

		// Retry wrapper: on transient failure, reset and retry from scratch
		let lastStreamError: unknown;
		for (let attempt = 1; attempt <= maxRetryAttempts; attempt++) {
			if (attempt > 1) {
				logger.debug(
					`Retrying generateStream (attempt ${attempt}/${maxRetryAttempts})`,
				);
				// Backoff before retry
				const delay = retryBaseDelayMs * 2 ** (attempt - 2);
				await new Promise<void>((r) =>
					setTimeout(r, Math.min(delay, retryMaxDelayMs)),
				);
			}

			const sessionId = await createSession(connection);
			const content = buildTextContent(prompt, streamOptions?.systemPrompt);

			// Append image content blocks if provided
			if (streamOptions?.images && streamOptions.images.length > 0) {
				for (const img of streamOptions.images) {
					content.push({
						type: 'resource',
						resource: {
							uri: `data:${img.mimeType};base64`,
							mimeType: img.mimeType,
							blob: img.base64,
						},
					});
				}
			}

			// Collect streaming chunks via notification handler
			type ChunkItem =
				| { text: string }
				| { keepalive: true }
				| { done: true }
				| { toolCall: ACPToolCall }
				| { toolCallUpdate: ACPToolCallUpdate };
			const chunks: ChunkItem[] = [];
			let chunkResolve: (() => void) | undefined;
			let streamUsage: ACPTokenUsage | undefined;

			const pushKeepalive = (): void => {
				chunks.push({ keepalive: true });
				chunkResolve?.();
			};

			const unsubscribe = connection.onNotification(
				'session/update',
				(params: unknown) => {
					const p = params as ACPSessionUpdateParams;
					if (p.sessionId !== sessionId) return;

					const update = p.update;
					if (!update) return;

					if (
						update.sessionUpdate === 'agent_message_chunk' &&
						update.content
					) {
						const content = update.content;
						// Content may be a single block or an array
						const blocks = Array.isArray(content) ? content : [content];
						const text = extractContentText(
							blocks as readonly ACPContentBlock[],
						);
						if (text) {
							chunks.push({ text });
							chunkResolve?.();
						}
					}

					if (update.sessionUpdate === 'tool_call') {
						const tc: ACPToolCall = {
							toolCallId: update.toolCallId as string,
							title: update.title as string,
							kind: (update.kind as ACPToolCall['kind']) ?? 'other',
							status: (update.status as ACPToolCall['status']) ?? 'pending',
						};
						streamOptions?.onToolCall?.(tc);
						// Yield as a typed chunk so consumers can render tool progress
						chunks.push({ toolCall: tc });
						chunkResolve?.();
					}

					if (update.sessionUpdate === 'tool_call_update') {
						const tcu: ACPToolCallUpdate = {
							toolCallId: update.toolCallId as string,
							status:
								(update.status as ACPToolCallUpdate['status']) ?? 'in_progress',
							content: update.content,
						};
						streamOptions?.onToolCallUpdate?.(tcu);
						// Yield as a typed chunk so consumers can render tool progress
						chunks.push({ toolCallUpdate: tcu });
						chunkResolve?.();
					}

					if (update.metadata) {
						const usage = extractTokenUsage(
							update.metadata as Readonly<Record<string, unknown>>,
						);
						if (usage) streamUsage = usage;
					}
				},
			);

			// Subscribe to permission activity events to push keepalives
			// while the user decides — prevents stream timeout during prompts
			const unsubscribePermission = connection.onPermissionActivity(() => {
				pushKeepalive();
			});

			// Send the prompt — don't await yet, chunks arrive as notifications
			const promptTimeoutMs = streamTimeoutMs;
			const promptPromise = sendPrompt(
				connection,
				sessionId,
				content,
				buildSamplingMetadata(streamOptions?.sampling),
				promptTimeoutMs,
			).then((result) => {
				// Final result arrived — mark stream as done
				if (result.metadata) {
					const usage = extractTokenUsage(result.metadata);
					if (usage) streamUsage = usage;
				}
				chunks.push({ done: true });
				chunkResolve?.();
				return result;
			});

			try {
				// Sliding-window stream timeout — resets each time a chunk arrives
				const timeoutMs = streamTimeoutMs;
				let lastActivity = Date.now();

				let idx = 0;
				while (true) {
					// Check abort signal
					if (streamOptions?.signal?.aborted) {
						yield { type: 'complete', usage: streamUsage };
						return;
					}

					if (idx < chunks.length) {
						const chunk = chunks[idx++];
						lastActivity = Date.now();
						if ('done' in chunk) break;
						// Keepalive chunks reset lastActivity but don't yield text
						if ('keepalive' in chunk) continue;
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
							yield { type: 'complete', usage: streamUsage };
							return;
						}

						const elapsed = Date.now() - lastActivity;
						if (elapsed >= timeoutMs) {
							throw createProviderGenerationError(
								'acp',
								`Stream timed out after ${timeoutMs}ms of inactivity`,
								{ model: agentId },
							);
						}

						const remaining = timeoutMs - elapsed;
						await new Promise<void>((resolve) => {
							chunkResolve = resolve;
							setTimeout(resolve, Math.min(remaining, 100));
						});
					}
				}

				yield { type: 'complete', usage: streamUsage };
				monitor?.recordSuccess();
				return; // success — exit retry loop
			} catch (error) {
				lastStreamError = error;
				// Only retry on transient errors and not on last attempt
				if (attempt < maxRetryAttempts && isTransientError(error)) {
					logger.warn(
						`Stream attempt ${attempt} failed with transient error, will retry`,
						{ error: toError(error).message },
					);
					continue;
				}

				monitor?.recordFailure(error instanceof Error ? error : undefined);
				if (isSimseError(error)) throw error;
				throw createProviderGenerationError(
					'acp',
					`Stream interrupted: ${toError(error).message}`,
					{ cause: error, model: agentId },
				);
			} finally {
				unsubscribe();
				unsubscribePermission();
				// Ensure the prompt promise settles
				promptPromise.catch(() => {});
			}
		}

		// All retry attempts exhausted
		monitor?.recordFailure(
			lastStreamError instanceof Error ? lastStreamError : undefined,
		);
		if (isSimseError(lastStreamError)) throw lastStreamError;
		throw createProviderGenerationError(
			'acp',
			`Stream failed after ${maxRetryAttempts} attempts: ${toError(lastStreamError).message}`,
			{ cause: lastStreamError, model: agentId },
		);
	}

	const embed = async (
		input: string | string[],
		model?: string,
		serverName?: string,
	): Promise<ACPEmbedResult> => {
		// ACP does not support embeddings natively.
		// Send embedding data as a data content block and hope the server handles it.
		const texts = Array.isArray(input) ? input : [input];
		const { connection, entry, name } = resolveConnection(serverName);
		const agentId = model ?? resolveAgentId(entry);

		logger.debug('Generating embeddings via ACP', {
			server: name,
			agent: agentId,
			inputCount: texts.length,
		});

		return withResilience(name, 'embed', async () => {
			const sessionId = await createSession(connection);

			// ACP does not define an embedding-specific content block.
			// Send as a text prompt asking the agent to return embeddings.
			const content: ACPContentBlock[] = [
				{
					type: 'text',
					text: JSON.stringify({ texts, action: 'embed' }),
				},
			];

			const result = await sendPrompt(connection, sessionId, content);

			// Try to extract embeddings from the response
			for (const block of result.content ?? []) {
				if (block.type === 'data') {
					const data = block.data;
					if (Array.isArray(data)) {
						return {
							embeddings: data as number[][],
							agentId,
							serverName: name,
							usage: extractTokenUsage(result.metadata),
						};
					}
					if (
						typeof data === 'object' &&
						data !== null &&
						'embeddings' in data
					) {
						return {
							embeddings: (data as { embeddings: number[][] }).embeddings,
							agentId,
							serverName: name,
							usage: extractTokenUsage(result.metadata),
						};
					}
				}

				if (block.type === 'text') {
					try {
						const parsed = JSON.parse(block.text);
						if (Array.isArray(parsed)) {
							return {
								embeddings: parsed as number[][],
								agentId,
								serverName: name,
								usage: extractTokenUsage(result.metadata),
							};
						}
						if (parsed && 'embeddings' in parsed) {
							return {
								embeddings: (parsed as { embeddings: number[][] }).embeddings,
								agentId,
								serverName: name,
								usage: extractTokenUsage(result.metadata),
							};
						}
					} catch {
						// Not JSON
					}
				}
			}

			throw createEmbeddingError(
				'ACP server returned no embeddings in response',
				{ model: agentId },
			);
		});
	};

	const listSessions = async (
		serverName?: string,
	): Promise<ACPSessionListEntry[]> => {
		const { connection } = resolveConnection(serverName);
		const result = await connection.request<{
			sessions: ACPSessionListEntry[];
		}>('session/list', {});
		return result.sessions ?? [];
	};

	const loadSession = async (
		sessionId: string,
		serverName?: string,
	): Promise<ACPSessionInfo> => {
		const { connection } = resolveConnection(serverName);
		const info = await connection.request<ACPSessionInfo>('session/load', {
			sessionId,
		});
		// Update session info cache with fresh data
		sessionInfoCache.set(sessionId, info);
		return info;
	};

	const deleteSession = async (
		sessionId: string,
		serverName?: string,
	): Promise<void> => {
		const { connection } = resolveConnection(serverName);
		await connection.request('session/delete', { sessionId });
	};

	const setSessionMode = async (
		sessionId: string,
		modeId: string,
		serverName?: string,
	): Promise<void> => {
		const { connection } = resolveConnection(serverName);
		await connection
			.request('session/set_config_option', {
				sessionId,
				configOptionId: 'mode',
				groupId: modeId,
			})
			.catch(() => {}); // Best-effort
	};

	const setSessionModel = async (
		sessionId: string,
		modelId: string,
		serverName?: string,
	): Promise<void> => {
		const { connection } = resolveConnection(serverName);
		await connection
			.request('session/set_config_option', {
				sessionId,
				configOptionId: 'model',
				groupId: modelId,
			})
			.catch(() => {}); // Best-effort
	};

	const isAvailable = async (serverName?: string): Promise<boolean> => {
		try {
			const name =
				serverName ?? config.defaultServer ?? config.servers[0]?.name;
			if (!name) return false;
			const connection = connections.get(name);
			return connection?.isConnected ?? false;
		} catch {
			return false;
		}
	};

	// -----------------------------------------------------------------------
	// Return the frozen record
	// -----------------------------------------------------------------------

	const getServerHealth = (serverName?: string): HealthSnapshot | undefined => {
		const name = serverName ?? config.defaultServer ?? config.servers[0]?.name;
		if (!name) return undefined;
		return monitors.get(name)?.getHealth();
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
		for (const conn of connections.values()) {
			conn.setPermissionPolicy(policy);
		}
	};

	const getServerModelInfo = async (
		serverName?: string,
	): Promise<ACPModelsInfo | undefined> => {
		try {
			const { connection } = resolveConnection(serverName);
			const result = await connection.request<ACPSessionNewResult>(
				'session/new',
				{
					cwd: process.cwd(),
					mcpServers: config.mcpServers ?? [],
				},
			);

			// Cache for later use
			sessionInfoCache.set(result.sessionId, result);

			// Clean up the temp session (fire-and-forget)
			connection
				.request('session/delete', { sessionId: result.sessionId })
				.catch(() => {});

			return result.models;
		} catch {
			return undefined;
		}
	};

	const getServerStatuses = async (): Promise<readonly ACPServerStatus[]> => {
		const statuses: ACPServerStatus[] = [];

		for (const entry of config.servers) {
			const connected = connections.has(entry.name);
			let currentModel: string | undefined;
			let availableModels: ACPModelInfo[] | undefined;

			if (connected) {
				try {
					const models = await getServerModelInfo(entry.name);
					if (models) {
						currentModel = models.currentModelId;
						availableModels = [...models.availableModels];
					}
				} catch {
					// Server may not support model info
				}
			}

			statuses.push(
				Object.freeze({
					name: entry.name,
					connected,
					currentModel,
					availableModels: availableModels
						? Object.freeze(availableModels)
						: undefined,
					agentId: entry.defaultAgent ?? config.defaultAgent,
				}),
			);
		}

		return Object.freeze(statuses);
	};

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
