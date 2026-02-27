// ---------------------------------------------------------------------------
// Local In-Process Embedder
//
// Implements EmbeddingProvider using @huggingface/transformers to run ONNX
// embedding models directly in Bun/Node. No server required.
//
// The model loads lazily on the first embed() call and is reused for all
// subsequent calls (in-flight promise dedup pattern).
// ---------------------------------------------------------------------------

import { toError } from '../../errors/base.js';
import { createEmbeddingError } from '../../errors/index.js';
import type { EmbeddingProvider } from '../library/types.js';

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

export interface LocalEmbedderOptions {
	/** Hugging Face model ID. Defaults to `nomic-ai/nomic-embed-text-v1.5`. */
	readonly model?: string;
	/** ONNX quantization dtype. Defaults to `q8`. */
	readonly dtype?: 'fp32' | 'fp16' | 'q8' | 'q4';
	/** Normalize embeddings (L2). Defaults to true. */
	readonly normalize?: boolean;
	/** Pooling strategy. Defaults to `mean`. */
	readonly pooling?: 'mean' | 'cls';
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createLocalEmbedder(
	options: LocalEmbedderOptions = {},
): EmbeddingProvider {
	const {
		model = 'nomic-ai/nomic-embed-text-v1.5',
		dtype = 'q8',
		normalize = true,
		pooling = 'mean',
	} = options;

	// In-flight promise dedup for lazy pipeline loading.
	let pipelinePromise: Promise<unknown> | undefined;

	const getPipeline = async () => {
		if (!pipelinePromise) {
			pipelinePromise = (async () => {
				try {
					const { pipeline } = await import('@huggingface/transformers');
					return await pipeline('feature-extraction', model, { dtype });
				} catch (err) {
					pipelinePromise = undefined;
					const error = toError(err);
					throw createEmbeddingError(
						`Failed to load embedding model "${model}": ${error.message}`,
						{ cause: err, model },
					);
				}
			})();
		}
		return pipelinePromise as Promise<
			(
				input: string | string[],
				options: { pooling: string; normalize: boolean },
			) => Promise<{ tolist(): number[][] }>
		>;
	};

	return Object.freeze({
		embed: async (input: string | readonly string[]) => {
			const texts = typeof input === 'string' ? [input] : [...input];

			try {
				const extractor = await getPipeline();
				const result = await extractor(texts, { pooling, normalize });
				const embeddings = result.tolist();

				if (!Array.isArray(embeddings) || embeddings.length === 0) {
					throw createEmbeddingError('Model returned no embeddings', { model });
				}

				return { embeddings };
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
				throw createEmbeddingError(`Local embedding failed: ${error.message}`, {
					cause: err,
					model,
				});
			}
		},
	});
}
