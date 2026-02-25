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
	readonly messageCount: number;
	/** Approximate character count of the serialized conversation. */
	readonly estimatedChars: number;
	/** True when conversation exceeds the auto-compact threshold. */
	readonly needsCompaction: boolean;
}
