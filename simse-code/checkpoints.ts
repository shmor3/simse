/**
 * SimSE Code — Checkpointing / Rewind
 *
 * Saves VFS snapshots + conversation state for rewind functionality.
 * Ring buffer of max N checkpoints.
 * No external deps.
 */

import { randomUUID } from 'node:crypto';
import type { VFSSnapshot, VirtualFS } from 'simse';
import type { CheckpointManager, CheckpointSummary } from './app-context.js';
import type { Conversation, ConversationMessage } from './conversation.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type { CheckpointManager, CheckpointSummary };

interface Checkpoint {
	readonly id: string;
	readonly label: string | undefined;
	readonly timestamp: number;
	readonly vfsSnapshot: VFSSnapshot;
	readonly messages: readonly ConversationMessage[];
}

export interface CheckpointManagerOptions {
	readonly vfs: VirtualFS;
	readonly conversation: Conversation;
	/** Max number of checkpoints to keep. Default: 20 */
	readonly maxCheckpoints?: number;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createCheckpointManager(
	options: CheckpointManagerOptions,
): CheckpointManager {
	const { vfs, conversation } = options;
	const maxCheckpoints = options.maxCheckpoints ?? 20;
	const checkpoints: Checkpoint[] = [];

	const save = (label?: string): string => {
		const id = randomUUID().slice(0, 8);
		const checkpoint: Checkpoint = {
			id,
			label,
			timestamp: Date.now(),
			vfsSnapshot: vfs.snapshot(),
			messages: [...conversation.toMessages()],
		};

		checkpoints.push(checkpoint);

		// Ring buffer — remove oldest if over limit
		while (checkpoints.length > maxCheckpoints) {
			checkpoints.shift();
		}

		return id;
	};

	const rewind = (id: string): boolean => {
		const idx = checkpoints.findIndex((cp) => cp.id === id);
		if (idx < 0) return false;

		const checkpoint = checkpoints[idx];

		// Restore VFS state
		vfs.restore(checkpoint.vfsSnapshot);

		// Restore conversation state
		conversation.clear();
		for (const msg of checkpoint.messages) {
			switch (msg.role) {
				case 'system':
					conversation.setSystemPrompt(msg.content);
					break;
				case 'user':
					conversation.addUser(msg.content);
					break;
				case 'assistant':
					conversation.addAssistant(msg.content);
					break;
				case 'tool_result':
					conversation.addToolResult(
						msg.toolCallId ?? '',
						msg.toolName ?? '',
						msg.content,
					);
					break;
			}
		}

		// Remove all checkpoints after this one (they're now invalid)
		checkpoints.splice(idx + 1);

		return true;
	};

	const list = (): readonly CheckpointSummary[] => {
		return checkpoints.map((cp) =>
			Object.freeze({
				id: cp.id,
				label: cp.label,
				timestamp: cp.timestamp,
				messageCount: cp.messages.length,
			}),
		);
	};

	const clear = (): void => {
		checkpoints.length = 0;
	};

	const lastCheckpoint = (): CheckpointSummary | undefined => {
		if (checkpoints.length === 0) return undefined;
		const cp = checkpoints[checkpoints.length - 1];
		return Object.freeze({
			id: cp.id,
			label: cp.label,
			timestamp: cp.timestamp,
			messageCount: cp.messages.length,
		});
	};

	return Object.freeze({ save, rewind, list, clear, lastCheckpoint });
}
