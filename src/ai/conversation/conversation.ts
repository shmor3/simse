// ---------------------------------------------------------------------------
// Conversation Buffer
//
// Accumulates messages for multi-turn agentic interactions.
// Tracks user messages, assistant responses, and tool results
// to build the full conversation context for each ACP call.
// ---------------------------------------------------------------------------

import type {
	Conversation,
	ConversationMessage,
	ConversationOptions,
} from './types.js';

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createConversation(
	options?: ConversationOptions,
): Conversation {
	const maxMessages = options?.maxMessages ?? 0;
	const autoCompactChars = options?.autoCompactChars ?? 100_000;
	let systemPrompt = options?.systemPrompt;
	const messages: ConversationMessage[] = [];

	const trimIfNeeded = (): void => {
		if (maxMessages <= 0) return;
		// Keep system messages out of the trim count
		const nonSystem = messages.filter((m) => m.role !== 'system');
		while (nonSystem.length > maxMessages) {
			const oldest = nonSystem.shift();
			if (oldest) {
				const idx = messages.indexOf(oldest);
				if (idx !== -1) messages.splice(idx, 1);
			}
		}
	};

	const addUser = (content: string): void => {
		messages.push(
			Object.freeze({ role: 'user', content, timestamp: Date.now() }),
		);
		trimIfNeeded();
	};

	const addAssistant = (content: string): void => {
		messages.push(
			Object.freeze({ role: 'assistant', content, timestamp: Date.now() }),
		);
		trimIfNeeded();
	};

	const addToolResult = (
		toolCallId: string,
		toolName: string,
		content: string,
	): void => {
		messages.push(
			Object.freeze({
				role: 'tool_result' as const,
				content,
				toolCallId,
				toolName,
				timestamp: Date.now(),
			}),
		);
		trimIfNeeded();
	};

	const setSystemPrompt = (prompt: string): void => {
		systemPrompt = prompt;
	};

	const toMessages = (): readonly ConversationMessage[] => {
		const result: ConversationMessage[] = [];
		if (systemPrompt) {
			result.push(Object.freeze({ role: 'system', content: systemPrompt }));
		}
		result.push(...messages);
		return Object.freeze(result);
	};

	const formatMessage = (msg: ConversationMessage): string => {
		switch (msg.role) {
			case 'system':
				return `[System]\n${msg.content}`;
			case 'user':
				return `[User]\n${msg.content}`;
			case 'assistant':
				return `[Assistant]\n${msg.content}`;
			case 'tool_result':
				return `[Tool Result: ${msg.toolName ?? msg.toolCallId}]\n${msg.content}`;
		}
	};

	const serialize = (): string => {
		const allMessages = toMessages();
		return allMessages.map(formatMessage).join('\n\n');
	};

	const clear = (): void => {
		messages.length = 0;
	};

	const compact = (summary: string): void => {
		messages.length = 0;
		messages.push(
			Object.freeze({
				role: 'user' as const,
				content: `[Conversation summary]\n${summary}`,
				timestamp: Date.now(),
			}),
		);
	};

	const replaceMessages = (
		newMessages: readonly ConversationMessage[],
	): void => {
		messages.length = 0;
		// Filter out system messages — those are managed via setSystemPrompt
		for (const msg of newMessages) {
			if (msg.role !== 'system') {
				messages.push(Object.freeze({ ...msg }));
			}
		}
	};

	const estimateChars = (): number => {
		let total = systemPrompt?.length ?? 0;
		for (const msg of messages) {
			total += msg.content.length;
		}
		return total;
	};

	return Object.freeze({
		addUser,
		addAssistant,
		addToolResult,
		setSystemPrompt,
		toMessages,
		serialize,
		clear,
		compact,
		replaceMessages,
		get messageCount() {
			return messages.length;
		},
		get estimatedChars() {
			return estimateChars();
		},
		get needsCompaction() {
			return estimateChars() > autoCompactChars;
		},
	});
}
