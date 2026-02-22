// ---------------------------------------------------------------------------
// Agent Communication Protocol (ACP) Client
// ---------------------------------------------------------------------------

import {
	createEmbeddingError,
	createProviderGenerationError,
	createProviderTimeoutError,
	createProviderUnavailableError,
	isEmbeddingError,
	isSimseError,
	toError,
} from '../../errors/index.js';
import { getDefaultLogger, type Logger } from '../../logger.js';
import { isTransientError, retry } from '../../utils/retry.js';
import {
	buildHeaders,
	fetchWithTimeout,
	httpGet,
	httpPost,
	type ResolvedServer,
	wrapFetchError,
} from './acp-http.js';
import { extractEmbeddings, extractGenerateResult } from './acp-results.js';
import { extractStreamDelta } from './acp-stream.js';
import type {
	ACPAgentInfo,
	ACPConfig,
	ACPCreateRunRequest,
	ACPEmbedResult,
	ACPGenerateResult,
	ACPMessage,
	ACPRun,
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
	/** Override the polling configuration for run-based requests. */
	pollingOptions?: {
		/** Initial polling interval in milliseconds. Defaults to `500`. */
		initialIntervalMs?: number;
		/** Maximum polling interval in milliseconds. Defaults to `5000`. */
		maxIntervalMs?: number;
		/** Polling backoff multiplier. Defaults to `2`. */
		backoffMultiplier?: number;
	};
	/** Timeout for streaming requests in milliseconds. Defaults to `120000`. */
	streamTimeoutMs?: number;
	/** Timeout for health check (isAvailable) in milliseconds. Defaults to `5000`. */
	healthCheckTimeoutMs?: number;
	/** Inject a custom logger (defaults to the global logger). */
	logger?: Logger;
}

// ---------------------------------------------------------------------------
// ACPClient interface
// ---------------------------------------------------------------------------

export interface ACPClient {
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
		},
	) => Promise<ACPGenerateResult>;
	readonly generateStream: (
		prompt: string,
		options?: {
			agentId?: string;
			serverName?: string;
			systemPrompt?: string;
			config?: Record<string, unknown>;
		},
	) => AsyncGenerator<string>;
	readonly embed: (
		input: string | string[],
		model?: string,
	) => Promise<ACPEmbedResult>;
	readonly isAvailable: (serverName?: string) => Promise<boolean>;
	readonly serverNames: string[];
	readonly serverCount: number;
	readonly defaultServerName: string | undefined;
	readonly defaultAgent: string | undefined;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createACPClient(
	config: ACPConfig,
	options?: ACPClientOptions,
): ACPClient {
	const logger = (options?.logger ?? getDefaultLogger()).child('acp');

	const maxRetryAttempts = options?.retryOptions?.maxAttempts ?? 3;
	const pollInitialIntervalMs =
		options?.pollingOptions?.initialIntervalMs ?? 500;
	const pollMaxIntervalMs = options?.pollingOptions?.maxIntervalMs ?? 5_000;
	const pollBackoffMultiplier = options?.pollingOptions?.backoffMultiplier ?? 2;
	const streamTimeoutMs = options?.streamTimeoutMs ?? 120_000;
	const healthCheckTimeoutMs = options?.healthCheckTimeoutMs ?? 5_000;
	const retryBaseDelayMs = options?.retryOptions?.baseDelayMs ?? 500;
	const retryMaxDelayMs = options?.retryOptions?.maxDelayMs ?? 15_000;

	// Index servers by name — validate URL schemes to prevent SSRF
	const servers = new Map<string, ResolvedServer>();
	for (const entry of config.servers) {
		let parsed: URL;
		try {
			parsed = new URL(entry.url);
		} catch {
			throw createProviderUnavailableError('acp', {
				metadata: {
					reason: `Invalid URL for ACP server "${entry.name}": ${entry.url}`,
				},
			});
		}
		if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
			throw createProviderUnavailableError('acp', {
				metadata: {
					reason: `ACP server "${entry.name}" URL must use http or https, got "${parsed.protocol}"`,
				},
			});
		}
		const baseUrl = entry.url.replace(/\/+$/, '');
		servers.set(entry.name, { entry, baseUrl });
	}

	// -----------------------------------------------------------------------
	// Internal helpers — resolution
	// -----------------------------------------------------------------------

	const resolveServer = (serverName?: string): ResolvedServer => {
		const name = serverName ?? config.defaultServer ?? config.servers[0]?.name;

		if (!name) {
			throw createProviderUnavailableError('acp', {
				metadata: { reason: 'No ACP servers configured' },
			});
		}

		const server = servers.get(name);
		if (!server) {
			throw createProviderUnavailableError('acp', {
				metadata: {
					reason: `ACP server "${name}" is not configured`,
					configuredServers: [...servers.keys()],
				},
			});
		}

		return server;
	};

	const resolveAgentId = (
		server: ResolvedServer,
		stepAgentId?: string,
	): string => {
		const agentId =
			stepAgentId ?? server.entry.defaultAgent ?? config.defaultAgent;

		if (!agentId) {
			throw createProviderGenerationError(
				'acp',
				'No agent ID specified. Set a default agent in config or provide one per step.',
			);
		}

		return agentId;
	};

	// -----------------------------------------------------------------------
	// Internal helpers — message building
	// -----------------------------------------------------------------------

	const buildInputMessages = (
		prompt: string,
		systemPrompt?: string,
	): ACPMessage[] => {
		const text = systemPrompt ? `${systemPrompt}\n\n${prompt}` : prompt;
		return [
			{
				role: 'user',
				parts: [{ type: 'text' as const, text }],
			},
		];
	};

	// -----------------------------------------------------------------------
	// Internal helpers — retry
	// -----------------------------------------------------------------------

	const withRetry = async <T>(
		operation: string,
		fn: () => Promise<T>,
	): Promise<T> => {
		return retry(
			async (attempt) => {
				if (attempt > 1) {
					logger.debug(
						`Retrying "${operation}" (attempt ${attempt}/${maxRetryAttempts})`,
					);
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
	};

	// -----------------------------------------------------------------------
	// Run management
	// -----------------------------------------------------------------------

	const isEndpointNotFound = (error: unknown): boolean =>
		isSimseError(error) &&
		(error.statusCode === 404 || error.statusCode === 405);

	const createAndPollRun = async (
		server: ResolvedServer,
		body: ACPCreateRunRequest,
	): Promise<ACPRun> => {
		const run = await httpPost<ACPRun>(server, '/runs', body);

		if (run.status === 'completed' || run.status === 'failed') {
			return run;
		}

		const maxPollMs = server.entry.timeoutMs ?? 30_000;
		let pollIntervalMs = pollInitialIntervalMs;
		const deadline = Date.now() + maxPollMs;

		while (Date.now() < deadline) {
			const remaining = deadline - Date.now();
			if (remaining <= 0) break;
			await new Promise((resolve) =>
				setTimeout(resolve, Math.min(pollIntervalMs, remaining)),
			);
			pollIntervalMs = Math.min(
				pollIntervalMs * pollBackoffMultiplier,
				pollMaxIntervalMs,
			);

			const updated = await httpGet<ACPRun>(
				server,
				`/runs/${encodeURIComponent(run.run_id)}`,
			);

			if (updated.status === 'completed' || updated.status === 'failed') {
				return updated;
			}

			if (updated.status === 'cancelled') {
				throw createProviderGenerationError(
					'acp',
					`Run ${run.run_id} was cancelled`,
					{ model: body.agent_id },
				);
			}

			if (updated.status === 'awaiting_input') {
				throw createProviderGenerationError(
					'acp',
					`Run ${run.run_id} is awaiting input, which is not supported by this client`,
					{ model: body.agent_id },
				);
			}
		}

		throw createProviderTimeoutError('acp', maxPollMs, {
			cause: new Error(
				`Run ${run.run_id} did not complete within ${maxPollMs}ms`,
			),
		});
	};

	// -----------------------------------------------------------------------
	// Public methods
	// -----------------------------------------------------------------------

	const listAgents = async (serverName?: string): Promise<ACPAgentInfo[]> => {
		const server = resolveServer(serverName);

		logger.debug(`Listing agents on "${server.entry.name}"`, {
			url: server.baseUrl,
		});

		const response = await httpGet<ACPAgentInfo[] | { agents: ACPAgentInfo[] }>(
			server,
			'/agents',
		);

		const agents = Array.isArray(response) ? response : response.agents;
		if (!Array.isArray(agents)) {
			throw createProviderGenerationError(
				'acp',
				`Unexpected response format from /agents on "${server.entry.name}"`,
			);
		}
		logger.debug(`Found ${agents.length} agent(s) on "${server.entry.name}"`);
		return agents;
	};

	const getAgent = async (
		agentId: string,
		serverName?: string,
	): Promise<ACPAgentInfo> => {
		const server = resolveServer(serverName);

		logger.debug(`Getting agent "${agentId}" on "${server.entry.name}"`);

		return httpGet<ACPAgentInfo>(
			server,
			`/agents/${encodeURIComponent(agentId)}`,
		);
	};

	const generate = async (
		prompt: string,
		generateOptions?: {
			agentId?: string;
			serverName?: string;
			systemPrompt?: string;
			config?: Record<string, unknown>;
		},
	): Promise<ACPGenerateResult> => {
		const server = resolveServer(generateOptions?.serverName);
		const agentId = resolveAgentId(server, generateOptions?.agentId);

		logger.debug('Starting generate request', {
			server: server.entry.name,
			agent: agentId,
			promptLength: prompt.length,
			hasSystemPrompt: !!generateOptions?.systemPrompt,
		});

		return withRetry('generate', async () => {
			const input = buildInputMessages(prompt, generateOptions?.systemPrompt);

			const body: ACPCreateRunRequest = {
				agent_id: agentId,
				input,
				...(generateOptions?.config ? { config: generateOptions.config } : {}),
			};

			let run: ACPRun;
			try {
				run = await httpPost<ACPRun>(server, '/runs/wait', body);
			} catch (error) {
				if (isEndpointNotFound(error)) {
					logger.debug(
						'/runs/wait not available, falling back to async run + polling',
					);
					run = await createAndPollRun(server, body);
				} else {
					throw error;
				}
			}

			return extractGenerateResult(run, server.entry.name);
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
		},
	): Promise<ACPGenerateResult> => {
		if (messages.length === 0) {
			throw createProviderGenerationError(
				'acp',
				'Cannot send an empty message list to ACP chat',
			);
		}

		const server = resolveServer(chatOptions?.serverName);
		const agentId = resolveAgentId(server, chatOptions?.agentId);

		logger.debug('Starting chat request', {
			server: server.entry.name,
			agent: agentId,
			messageCount: messages.length,
		});

		return withRetry('chat', async () => {
			const systemMessages = messages.filter((m) => m.role === 'system');
			const systemText = systemMessages.map((m) => m.content).join('\n\n');
			const conversationMessages = messages.filter((m) => m.role !== 'system');

			// If only system messages were provided, convert to a user message
			let systemInjected = false;
			if (conversationMessages.length === 0 && systemText.length > 0) {
				conversationMessages.push({
					role: 'user',
					content: systemText,
				});
				systemInjected = true;
			}
			const input: ACPMessage[] = conversationMessages.map((msg) => {
				const role: 'user' | 'agent' =
					msg.role === 'assistant' ? 'agent' : 'user';
				let text = msg.content;

				if (!systemInjected && role === 'user' && systemText.length > 0) {
					text = `${systemText}\n\n${text}`;
					systemInjected = true;
				}

				return {
					role,
					parts: [{ type: 'text' as const, text }],
				};
			});

			const body: ACPCreateRunRequest = {
				agent_id: agentId,
				input,
				...(chatOptions?.config ? { config: chatOptions.config } : {}),
			};

			let run: ACPRun;
			try {
				run = await httpPost<ACPRun>(server, '/runs/wait', body);
			} catch (error) {
				if (isEndpointNotFound(error)) {
					run = await createAndPollRun(server, body);
				} else {
					throw error;
				}
			}

			return extractGenerateResult(run, server.entry.name);
		});
	};

	async function* generateStream(
		prompt: string,
		streamOptions?: {
			agentId?: string;
			serverName?: string;
			systemPrompt?: string;
			config?: Record<string, unknown>;
		},
	): AsyncGenerator<string> {
		const server = resolveServer(streamOptions?.serverName);
		const agentId = resolveAgentId(server, streamOptions?.agentId);

		logger.debug('Starting generate stream', {
			server: server.entry.name,
			agent: agentId,
			promptLength: prompt.length,
		});

		const input = buildInputMessages(prompt, streamOptions?.systemPrompt);

		const body: ACPCreateRunRequest = {
			agent_id: agentId,
			input,
			...(streamOptions?.config ? { config: streamOptions.config } : {}),
		};

		const timeoutMs = server.entry.timeoutMs ?? streamTimeoutMs;

		let response: Response;
		try {
			response = await fetchWithTimeout(
				`${server.baseUrl}/runs/stream`,
				{
					method: 'POST',
					headers: {
						...buildHeaders(server),
						Accept: 'text/event-stream',
					},
					body: JSON.stringify(body),
				},
				timeoutMs,
			);
		} catch (error) {
			throw wrapFetchError(
				'generateStream',
				error,
				server.entry.name,
				timeoutMs,
			);
		}

		if (!response.ok) {
			const text = await response.text().catch(() => '');
			throw createProviderGenerationError(
				'acp',
				`Streaming request failed (${response.status}): ${text}`,
				{ model: agentId },
			);
		}

		if (!response.body) {
			throw createProviderGenerationError(
				'acp',
				'Server returned no response body for streaming request',
				{ model: agentId },
			);
		}

		const reader = response.body.getReader();
		const decoder = new TextDecoder();
		let buffer = '';
		// Track whether any incremental deltas were yielded, so we only
		// use the completed-event full-text fallback when no deltas arrived.
		let anyDeltaYielded = false;

		try {
			while (true) {
				const { done, value } = await reader.read();
				if (done) break;

				buffer += decoder.decode(value, { stream: true });

				// SSE spec allows \r\n, \r, and \n as line terminators
				const lines = buffer.split(/\r\n|\r|\n/);
				buffer = lines.pop() ?? '';

				for (const line of lines) {
					const trimmed = line.trim();
					if (trimmed.startsWith('data:')) {
						const jsonStr = trimmed.slice(5).trim();
						if (jsonStr === '[DONE]') return;
						if (jsonStr.length === 0) continue;

						try {
							const event = JSON.parse(jsonStr) as Record<string, unknown>;
							const delta = extractStreamDelta(event);
							if (delta) {
								// Skip completed-event full-text if deltas were already yielded
								const isCompletedFallback =
									event.status === 'completed' && anyDeltaYielded;
								if (!isCompletedFallback) {
									anyDeltaYielded = true;
									yield delta;
								}
							}
						} catch {
							logger.debug('Skipping malformed SSE data', {
								data: jsonStr,
							});
						}
					}
				}
			}

			if (buffer.trim().startsWith('data:')) {
				const jsonStr = buffer.trim().slice(5).trim();
				if (jsonStr.length > 0 && jsonStr !== '[DONE]') {
					try {
						const event = JSON.parse(jsonStr) as Record<string, unknown>;
						const delta = extractStreamDelta(event);
						if (delta) {
							const isCompletedFallback =
								event.status === 'completed' && anyDeltaYielded;
							if (!isCompletedFallback) {
								yield delta;
							}
						}
					} catch {
						// Ignore
					}
				}
			}
		} catch (error) {
			// Don't double-wrap errors that are already structured SimseErrors
			if (isSimseError(error)) throw error;
			throw createProviderGenerationError(
				'acp',
				`Stream interrupted: ${toError(error).message}`,
				{ cause: error, model: agentId },
			);
		} finally {
			reader.releaseLock();
			try {
				await response.body?.cancel();
			} catch {
				// Ignore cancel errors
			}
		}
	}

	const embed = async (
		input: string | string[],
		model?: string,
	): Promise<ACPEmbedResult> => {
		const texts = Array.isArray(input) ? input : [input];
		const server = resolveServer();
		const agentId = model ?? resolveAgentId(server);

		logger.debug('Generating embeddings', {
			server: server.entry.name,
			agent: agentId,
			inputCount: texts.length,
		});

		return withRetry('embed', async () => {
			const inputMessages: ACPMessage[] = [
				{
					role: 'user',
					parts: [
						{
							type: 'data' as const,
							data: { texts, action: 'embed' },
							mimeType: 'application/json',
						},
					],
				},
			];

			const body: ACPCreateRunRequest = {
				agent_id: agentId,
				input: inputMessages,
				config: { mode: 'embedding' },
			};

			let run: ACPRun;
			try {
				run = await httpPost<ACPRun>(server, '/runs/wait', body);
			} catch (error) {
				if (isEndpointNotFound(error)) {
					run = await createAndPollRun(server, body);
				} else {
					if (isEmbeddingError(error)) throw error;
					// Re-throw transient errors unwrapped so withRetry can match them
					if (isTransientError(error)) throw error;
					throw createEmbeddingError(
						`Embedding request failed: ${toError(error).message}`,
						{ cause: error, model: agentId },
					);
				}
			}

			if (run.status === 'failed') {
				throw createEmbeddingError(
					`Embedding agent failed: ${run.error?.message ?? 'unknown error'}`,
					{ model: agentId },
				);
			}

			const embeddings = extractEmbeddings(run);

			if (!embeddings || embeddings.length === 0) {
				throw createEmbeddingError('Embedding agent returned no embeddings', {
					model: agentId,
				});
			}

			return {
				embeddings,
				agentId,
				serverName: server.entry.name,
			};
		});
	};

	const isAvailable = async (serverName?: string): Promise<boolean> => {
		try {
			const server = resolveServer(serverName);
			const response = await fetchWithTimeout(
				`${server.baseUrl}/agents`,
				{
					method: 'GET',
					headers: buildHeaders(server),
				},
				healthCheckTimeoutMs,
			);
			// Drain body to release the underlying connection
			await response.body?.cancel();
			return response.ok;
		} catch {
			logger.debug('ACP server is not available', {
				server: serverName ?? config.defaultServer ?? '(default)',
			});
			return false;
		}
	};

	// -----------------------------------------------------------------------
	// Return the frozen record
	// -----------------------------------------------------------------------

	return Object.freeze({
		listAgents,
		getAgent,
		generate,
		chat,
		generateStream,
		embed,
		isAvailable,
		get serverNames() {
			return [...servers.keys()];
		},
		get serverCount() {
			return servers.size;
		},
		get defaultServerName() {
			return config.defaultServer ?? config.servers[0]?.name;
		},
		get defaultAgent() {
			return config.defaultAgent;
		},
	});
}
