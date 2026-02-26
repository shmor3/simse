// ---------------------------------------------------------------------------
// Conversation Types
// ---------------------------------------------------------------------------

export type ConversationRole = 'system' | 'user' | 'assistant' | 'tool_result';

export interface ConversationMessage {
	readonly role: ConversationRole;
	readonly content: string;
	readonly toolCallId?: string;
	readonly toolName?: string;
	readonly timestamp?: number;
}

export interface ConversationOptions {
	readonly systemPrompt?: string;
	readonly maxMessages?: number;
	/** Approximate max character budget before auto-compact triggers. Default: 100_000 (~25k tokens). */
	readonly autoCompactChars?: number;
	/** Number of recent user-turns to protect from context pruning. Default: 2. */
	readonly pruneProtectTurns?: number;
	/** Tool names whose results should never be pruned by the context pruner. */
	readonly pruneProtectedTools?: readonly string[];
	/** Custom token estimator function. Default: `Math.ceil(chars / 4)`. */
	readonly tokenEstimator?: (text: string) => number;
}

export interface Conversation {
	readonly addUser: (content: string) => void;
	readonly addAssistant: (content: string) => void;
	readonly addToolResult: (
		toolCallId: string,
		toolName: string,
		content: string,
	) => void;
	readonly setSystemPrompt: (prompt: string) => void;
	readonly toMessages: () => readonly ConversationMessage[];
	readonly serialize: () => string;
	readonly clear: () => void;
	readonly compact: (summary: string) => void;
	/** Replace all messages (excluding system prompt). Used by the context pruner. */
	readonly replaceMessages?: (messages: readonly ConversationMessage[]) => void;
	readonly messageCount: number;
	/** Approximate character count of the serialized conversation. */
	readonly estimatedChars: number;
	/** Approximate token count (chars / 4 by default, or custom estimator). */
	readonly estimatedTokens: number;
	/** True when conversation exceeds the auto-compact threshold. */
	readonly needsCompaction: boolean;
}
