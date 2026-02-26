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
	const tokenEstimator = options?.tokenEstimator;
	const contextWindowTokens = options?.contextWindowTokens;
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

	const getSystemPrompt = (): string | undefined => systemPrompt;

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

	const toJSON = (): string => {
		return JSON.stringify({
			systemPrompt: systemPrompt ?? null,
			messages: messages.map((m) => ({
				role: m.role,
				content: m.content,
				...(m.toolCallId ? { toolCallId: m.toolCallId } : {}),
				...(m.toolName ? { toolName: m.toolName } : {}),
				...(m.timestamp ? { timestamp: m.timestamp } : {}),
			})),
		});
	};

	const fromJSON = (json: string): void => {
		const data = JSON.parse(json) as {
			systemPrompt?: string | null;
			messages?: Array<{
				role: string;
				content: string;
				toolCallId?: string;
				toolName?: string;
				timestamp?: number;
			}>;
		};
		if (data.systemPrompt) {
			systemPrompt = data.systemPrompt;
		}
		messages.length = 0;
		if (data.messages) {
			for (const msg of data.messages) {
				if (msg.role === 'system') continue;
				messages.push(
					Object.freeze({
						role: msg.role as ConversationMessage['role'],
						content: msg.content,
						toolCallId: msg.toolCallId,
						toolName: msg.toolName,
						timestamp: msg.timestamp,
					}),
				);
			}
		}
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

	const estimateTokens = (): number => {
		if (tokenEstimator) {
			let total = 0;
			if (systemPrompt) total += tokenEstimator(systemPrompt);
			for (const msg of messages) {
				total += tokenEstimator(msg.content);
			}
			return total;
		}
		return Math.ceil(estimateChars() / 4);
	};

	return Object.freeze({
		addUser,
		addAssistant,
		addToolResult,
		getSystemPrompt,
		setSystemPrompt,
		toMessages,
		serialize,
		clear,
		compact,
		toJSON,
		fromJSON,
		replaceMessages,
		get messageCount() {
			return messages.length;
		},
		get estimatedChars() {
			return estimateChars();
		},
		get estimatedTokens() {
			return estimateTokens();
		},
		get needsCompaction() {
			return estimateChars() > autoCompactChars;
		},
		get contextUsagePercent() {
			if (!contextWindowTokens || contextWindowTokens <= 0) return 0;
			return Math.min(
				100,
				Math.round((estimateTokens() / contextWindowTokens) * 100),
			);
		},
	});
}
