/**
 * SimSE — Agent Service
 *
 * Wraps the ACP client with convenience methods for text generation,
 * streaming, chat, and embeddings. Handles server availability checks
 * and provides a unified interface for AI operations.
 */

import {
	type ACPChatMessage,
	type ACPClient,
	type ACPClientOptions,
	type ACPConfig,
	type ACPEmbedResult,
	type ACPGenerateOptions,
	type ACPGenerateResult,
	type ACPStreamChunk,
	createACPClient,
	type Logger,
} from 'simse';

// ---------------------------------------------------------------------------
// Interface
// ---------------------------------------------------------------------------

export interface AgentServiceOptions {
	readonly config: ACPConfig;
	readonly logger: Logger;
	readonly clientOptions?: Omit<ACPClientOptions, 'logger'>;
}

export interface AgentService {
	/** The underlying ACP client. */
	readonly client: ACPClient;
	/** Spawn all command-based ACP servers and wait for readiness. */
	readonly initialize: () => Promise<void>;
	/** Kill all spawned ACP server processes. */
	readonly dispose: () => Promise<void>;
	/** Check availability of all configured servers. */
	readonly checkAvailability: () => Promise<Readonly<Record<string, boolean>>>;
	/** Generate text from a prompt. */
	readonly generate: (
		prompt: string,
		options?: ACPGenerateOptions,
	) => Promise<ACPGenerateResult>;
	/** Stream text generation chunk by chunk. */
	readonly generateStream: (
		prompt: string,
		options?: ACPGenerateOptions,
	) => AsyncGenerator<ACPStreamChunk>;
	/** Multi-turn chat conversation. */
	readonly chat: (messages: ACPChatMessage[]) => Promise<ACPGenerateResult>;
	/** Generate embeddings for one or more texts. */
	readonly embed: (
		input: string | string[],
		model?: string,
		serverName?: string,
	) => Promise<ACPEmbedResult>;
	/** Whether the primary server is reachable. */
	readonly isAvailable: () => Promise<boolean>;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createAgentService(options: AgentServiceOptions): AgentService {
	const client = createACPClient(options.config, {
		logger: options.logger,
		...options.clientOptions,
	});

	const checkAvailability = async (): Promise<
		Readonly<Record<string, boolean>>
	> => {
		const result: Record<string, boolean> = {};
		for (const name of client.serverNames) {
			result[name] = await client.isAvailable(name);
		}
		return Object.freeze(result);
	};

	const generate = (
		prompt: string,
		opts?: ACPGenerateOptions,
	): Promise<ACPGenerateResult> => client.generate(prompt, opts);

	const generateStream = (
		prompt: string,
		opts?: ACPGenerateOptions,
	): AsyncGenerator<ACPStreamChunk> => client.generateStream(prompt, opts);

	const chat = (messages: ACPChatMessage[]): Promise<ACPGenerateResult> =>
		client.chat(messages);

	const embed = (
		input: string | string[],
		model?: string,
		serverName?: string,
	): Promise<ACPEmbedResult> => client.embed(input, model, serverName);

	const isAvailable = (): Promise<boolean> => client.isAvailable();

	return Object.freeze({
		client,
		initialize: () => client.initialize(),
		dispose: () => client.dispose(),
		checkAvailability,
		generate,
		generateStream,
		chat,
		embed,
		isAvailable,
	});
}
