// ---------------------------------------------------------------------------
// ACP result extraction — parse token usage and content from ACP responses
// ---------------------------------------------------------------------------

import type {
	ACPContentBlock,
	ACPTokenUsage,
	ACPToolCall,
	ACPToolCallUpdate,
} from './types.js';

/**
 * Extract token usage from metadata.
 * Supports both snake_case (OpenAI-style) and camelCase field names.
 * Returns undefined if usage data is not present or malformed.
 */
export function extractTokenUsage(
	metadata: Readonly<Record<string, unknown>> | undefined,
): ACPTokenUsage | undefined {
	if (!metadata) return undefined;

	const usage = metadata.usage;
	if (typeof usage !== 'object' || usage === null) return undefined;

	const u = usage as Record<string, unknown>;

	// Try snake_case first (OpenAI-compatible), then camelCase
	const prompt =
		typeof u.prompt_tokens === 'number'
			? u.prompt_tokens
			: typeof u.promptTokens === 'number'
				? u.promptTokens
				: undefined;

	const completion =
		typeof u.completion_tokens === 'number'
			? u.completion_tokens
			: typeof u.completionTokens === 'number'
				? u.completionTokens
				: undefined;

	if (prompt === undefined || completion === undefined) return undefined;

	const total =
		typeof u.total_tokens === 'number'
			? u.total_tokens
			: typeof u.totalTokens === 'number'
				? u.totalTokens
				: prompt + completion;

	return {
		promptTokens: prompt,
		completionTokens: completion,
		totalTokens: total,
	};
}

/**
 * Extract concatenated text from ACP content blocks.
 * Returns empty string if no text blocks are present.
 */
export function extractContentText(
	content: readonly ACPContentBlock[] | undefined,
): string {
	if (!content) return '';
	let text = '';
	for (const block of content) {
		if (block.type === 'text') {
			text += block.text;
		}
	}
	return text;
}

/**
 * Extract a tool call from a session update notification.
 * Returns undefined if the update is not a tool_call event.
 */
export function extractToolCall(
	update: Record<string, unknown>,
): ACPToolCall | undefined {
	if (update.sessionUpdate !== 'tool_call') return undefined;
	return {
		toolCallId: update.toolCallId as string,
		title: (update.title as string) ?? '',
		kind: (update.kind as ACPToolCall['kind']) ?? 'other',
		status: (update.status as ACPToolCall['status']) ?? 'pending',
	};
}

/**
 * Extract a tool call update from a session update notification.
 * Returns undefined if the update is not a tool_call_update event.
 */
export function extractToolCallUpdate(
	update: Record<string, unknown>,
): ACPToolCallUpdate | undefined {
	if (update.sessionUpdate !== 'tool_call_update') return undefined;
	return {
		toolCallId: update.toolCallId as string,
		status: (update.status as ACPToolCallUpdate['status']) ?? 'in_progress',
		content: update.content,
	};
}
