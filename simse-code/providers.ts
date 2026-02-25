/**
 * SimSE — ACP Provider Adapters
 *
 * Adapters that wrap the ACP client to satisfy the EmbeddingProvider
 * and TextGenerationProvider interfaces required by the memory system.
 * All AI traffic flows through ACP — use the acp-ollama-bridge to
 * connect to a local Ollama instance.
 */

import type {
	ACPClient,
	EmbeddingProvider,
	TextGenerationProvider,
} from 'simse';

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
			const texts: string | string[] =
				typeof input === 'string' ? input : [...input];
			const result = await client.embed(texts, model, serverName);
			return { embeddings: result.embeddings };
		},
	});
}

// ---------------------------------------------------------------------------
// Text Generation Provider
// ---------------------------------------------------------------------------

export function createACPGenerator(
	options: ACPGeneratorOptions,
): TextGenerationProvider {
	const { client, agentId } = options;

	return Object.freeze({
		generate: async (prompt: string, systemPrompt?: string) => {
			const result = await client.generate(prompt, {
				agentId,
				systemPrompt,
			});
			return result.content;
		},
	});
}
