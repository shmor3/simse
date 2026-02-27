// ---------------------------------------------------------------------------
// ACP Provider Adapters
//
// Bridges the ACP client to EmbeddingProvider and TextGenerationProvider
// interfaces required by the memory system. All AI traffic flows through
// ACP — use the acp-ollama-bridge to connect to a local Ollama instance.
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import {
	createEmbeddingError,
	createProviderGenerationError,
} from '../../errors/index.js';
import type {
	EmbeddingProvider,
	TextGenerationProvider,
} from '../library/types.js';
import type { ACPClient } from './acp-client.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface ACPEmbedderOptions {
	/** ACP client to delegate to. */
	readonly client: ACPClient;
	/** Embedding model / agent ID. If omitted, uses the ACP default. */
	readonly model?: string;
	/** ACP server name for embeddings. If omitted, uses the default server. */
	readonly serverName?: string;
}

export interface ACPGeneratorOptions {
	/** ACP client to delegate to. */
	readonly client: ACPClient;
	/** Agent ID for generation. If omitted, uses the ACP default. */
	readonly agentId?: string;
	/** ACP server name for generation. If omitted, uses the default server. */
	readonly serverName?: string;
	/** Optional system prompt prefix for all generation requests. */
	readonly systemPromptPrefix?: string;
}

// ---------------------------------------------------------------------------
// Embedding Provider
// ---------------------------------------------------------------------------

export function createACPEmbedder(
	options: ACPEmbedderOptions,
): EmbeddingProvider {
	const { client, model, serverName } = options;

	return Object.freeze({
		embed: async (input: string | readonly string[]) => {
			try {
				const texts: string | string[] =
					typeof input === 'string' ? input : [...input];
				const result = await client.embed(texts, model, serverName);
				return { embeddings: result.embeddings };
			} catch (err) {
				const error = toError(err);
				throw createEmbeddingError(`Embedding failed: ${error.message}`, {
					cause: err,
				});
			}
		},
	});
}

// ---------------------------------------------------------------------------
// Text Generation Provider
// ---------------------------------------------------------------------------

export function createACPGenerator(
	options: ACPGeneratorOptions,
): TextGenerationProvider {
	const { client, agentId, serverName, systemPromptPrefix } = options;

	return Object.freeze({
		generate: async (prompt: string, systemPrompt?: string) => {
			try {
				const fullSystemPrompt =
					[systemPromptPrefix, systemPrompt].filter(Boolean).join('\n\n') ||
					undefined;

				const result = await client.generate(prompt, {
					agentId,
					serverName,
					systemPrompt: fullSystemPrompt,
				});
				return result.content;
			} catch (err) {
				const error = toError(err);
				throw createProviderGenerationError(
					agentId ?? 'default',
					`Generation failed: ${error.message}`,
					{ cause: err },
				);
			}
		},
		generateWithModel: async (
			prompt: string,
			modelId: string,
			systemPrompt?: string,
		) => {
			try {
				const fullSystemPrompt =
					[systemPromptPrefix, systemPrompt].filter(Boolean).join('\n\n') ||
					undefined;

				const result = await client.generate(prompt, {
					agentId,
					serverName,
					systemPrompt: fullSystemPrompt,
					modelId,
				});
				return result.content;
			} catch (err) {
				const error = toError(err);
				throw createProviderGenerationError(
					agentId ?? 'default',
					`Generation with model ${modelId} failed: ${error.message}`,
					{ cause: err },
				);
			}
		},
	});
}
