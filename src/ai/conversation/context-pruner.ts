// ---------------------------------------------------------------------------
// Context Pruner
//
// Two-phase context management: replaces old tool outputs with compact
// `[OUTPUT PRUNED — N chars]` markers to reduce token consumption while
// preserving recent turns and specified protected tools.
// ---------------------------------------------------------------------------

import type { ConversationMessage } from './types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ContextPrunerOptions {
	/** Number of recent user-turns to protect from pruning. Default: 2. */
	readonly protectRecentTurns?: number;
	/** Tool names whose results should never be pruned. */
	readonly pruneProtectedTools?: readonly string[];
}

export interface ContextPruner {
	readonly prune: (
		messages: readonly ConversationMessage[],
	) => readonly ConversationMessage[];
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/** Content shorter than this is not worth pruning. */
const MIN_PRUNE_LENGTH = 200;

const SUMMARY_MARKER = '[SUMMARY]';

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createContextPruner(
	options?: ContextPrunerOptions,
): ContextPruner {
	const protectRecentTurns = options?.protectRecentTurns ?? 2;
	const protectedTools = new Set(options?.pruneProtectedTools ?? []);

	const prune = (
		messages: readonly ConversationMessage[],
	): readonly ConversationMessage[] => {
		// ---- 1. Walk backward, count user messages as turn boundaries ----
		let turnCount = 0;
		let protectedFromIndex = messages.length; // default: nothing protected

		if (protectRecentTurns > 0) {
			for (let i = messages.length - 1; i >= 0; i--) {
				if (messages[i].role === 'user') {
					turnCount++;
					if (turnCount >= protectRecentTurns) {
						protectedFromIndex = i;
						break;
					}
				}
			}
		}

		// ---- 2. Find most recent [SUMMARY] marker in assistant messages ----
		let summaryBarrierIndex = -1;
		for (let i = messages.length - 1; i >= 0; i--) {
			if (
				messages[i].role === 'assistant' &&
				messages[i].content.includes(SUMMARY_MARKER)
			) {
				summaryBarrierIndex = i;
				break;
			}
		}

		// ---- 3. Replace eligible tool_result messages ----
		const result: ConversationMessage[] = [];
		let anyPruned = false;

		for (let i = 0; i < messages.length; i++) {
			const msg = messages[i];

			// Inside protected window — keep as-is
			if (i >= protectedFromIndex) {
				result.push(msg);
				continue;
			}

			// At or after summary barrier — keep as-is
			if (summaryBarrierIndex >= 0 && i >= summaryBarrierIndex) {
				result.push(msg);
				continue;
			}

			// Only prune tool_result messages
			if (msg.role !== 'tool_result') {
				result.push(msg);
				continue;
			}

			// Skip if content is too short to be worth pruning
			if (msg.content.length < MIN_PRUNE_LENGTH) {
				result.push(msg);
				continue;
			}

			// Skip if tool is in the protected set
			if (msg.toolName && protectedTools.has(msg.toolName)) {
				result.push(msg);
				continue;
			}

			// Prune: replace content with size marker
			const originalSize = msg.content.length;
			const pruned: ConversationMessage = Object.freeze({
				role: msg.role,
				content: `[OUTPUT PRUNED \u2014 ${originalSize} chars]`,
				toolCallId: msg.toolCallId,
				toolName: msg.toolName,
				timestamp: msg.timestamp,
			});
			result.push(pruned);
			anyPruned = true;
		}

		// If nothing was pruned, return the original array to preserve identity
		if (!anyPruned) {
			return messages;
		}

		return Object.freeze(result);
	};

	return Object.freeze({ prune });
}
