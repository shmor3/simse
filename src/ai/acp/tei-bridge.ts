// ---------------------------------------------------------------------------
// TEI (Text Embeddings Inference) Bridge
//
// Implements EmbeddingProvider by calling a Hugging Face Text Embeddings
// Inference server. Use this when the ACP server (e.g. Claude Code) does
// not support native embeddings.
//
// https://github.com/huggingface/text-embeddings-inference
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import { createEmbeddingError } from '../../errors/index.js';
import type { EmbeddingProvider } from '../memory/types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface TEIEmbedderOptions {
	/** TEI server base URL. Defaults to `http://localhost:8080`. */
	readonly baseUrl?: string;
	/** Request timeout in milliseconds. Defaults to 30_000. */
	readonly timeout?: number;
	/** Normalize embeddings. Defaults to true. */
	readonly normalize?: boolean;
	/** Truncate inputs that exceed the model's max length. Defaults to false. */
	readonly truncate?: boolean;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createTEIEmbedder(
	options: TEIEmbedderOptions = {},
): EmbeddingProvider {
	const {
		baseUrl = 'http://localhost:8080',
		timeout = 30_000,
		normalize = true,
		truncate = false,
	} = options;

	const url = `${baseUrl.replace(/\/+$/, '')}/embed`;

	return Object.freeze({
		embed: async (input: string | readonly string[]) => {
			const inputs = typeof input === 'string' ? input : [...input];

			try {
				const controller = new AbortController();
				const timer = setTimeout(() => controller.abort(), timeout);

				const response = await fetch(url, {
					method: 'POST',
					headers: { 'Content-Type': 'application/json' },
					body: JSON.stringify({ inputs, normalize, truncate }),
					signal: controller.signal,
				});

				clearTimeout(timer);

				if (!response.ok) {
					const body = await response.text().catch(() => '');
					throw createEmbeddingError(
						`TEI returned ${response.status}: ${body}`,
					);
				}

				const data = (await response.json()) as number[][];

				if (
					!Array.isArray(data) ||
					(data.length > 0 && !Array.isArray(data[0]))
				) {
					throw createEmbeddingError(
						'TEI response is not an array of embeddings',
					);
				}

				return { embeddings: data };
			} catch (err) {
				if (
					typeof err === 'object' &&
					err !== null &&
					'code' in err &&
					typeof (err as { code: unknown }).code === 'string'
				) {
					throw err;
				}

				const error = toError(err);
				throw createEmbeddingError(`TEI embedding failed: ${error.message}`, {
					cause: err,
				});
			}
		},
	});
}
