// ---------------------------------------------------------------------------
// ACP result extraction — parse generate results and embeddings from ACPRun
// ---------------------------------------------------------------------------

import { createProviderGenerationError } from '../../errors/index.js';
import type { ACPGenerateResult, ACPRun } from './types.js';

/**
 * Extract text content from a completed ACP run.
 * Throws if the run failed.
 */
export function extractGenerateResult(
	run: ACPRun,
	serverName: string,
): ACPGenerateResult {
	if (run.status === 'failed') {
		throw createProviderGenerationError(
			'acp',
			`Agent run failed: ${run.error?.message ?? 'unknown error'}`,
			{ model: run.agent_id },
		);
	}

	if (run.status === 'cancelled') {
		throw createProviderGenerationError(
			'acp',
			`Agent run ${run.run_id} was cancelled`,
			{ model: run.agent_id },
		);
	}

	if (run.status === 'awaiting_input') {
		throw createProviderGenerationError(
			'acp',
			`Agent run ${run.run_id} is awaiting input, which is not supported by this client`,
			{ model: run.agent_id },
		);
	}

	let content = '';
	if (run.output && Array.isArray(run.output)) {
		for (const msg of run.output) {
			if (msg.role === 'agent' && Array.isArray(msg.parts)) {
				for (const part of msg.parts) {
					if (part.type === 'text') {
						content += (part as { type: 'text'; text: string }).text;
					}
				}
			}
		}
	}

	return {
		content,
		agentId: run.agent_id,
		serverName,
		runId: run.run_id,
	};
}

/**
 * Extract embedding vectors from a completed ACP run.
 * Returns undefined if no embeddings are found.
 */
export function extractEmbeddings(run: ACPRun): number[][] | undefined {
	if (!run.output) return undefined;

	for (const msg of run.output) {
		if (msg.role !== 'agent') continue;
		for (const part of msg.parts) {
			if (part.type === 'data') {
				const data = (part as { type: 'data'; data: unknown }).data;
				if (Array.isArray(data)) {
					return data as number[][];
				}
				if (typeof data === 'object' && data !== null && 'embeddings' in data) {
					return (data as { embeddings: number[][] }).embeddings;
				}
			}

			if (part.type === 'text') {
				try {
					const parsed = JSON.parse(
						(part as { type: 'text'; text: string }).text,
					);
					if (Array.isArray(parsed)) return parsed as number[][];
					if (parsed && 'embeddings' in parsed) {
						return (parsed as { embeddings: number[][] }).embeddings;
					}
				} catch {
					// Not JSON, skip
				}
			}
		}
	}

	return undefined;
}
