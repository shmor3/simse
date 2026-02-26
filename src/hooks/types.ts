// ---------------------------------------------------------------------------
// Hook System Types
// ---------------------------------------------------------------------------

import type { ConversationMessage } from '../ai/conversation/types.js';
import type { ToolCallRequest, ToolCallResult } from '../ai/tools/types.js';

// ---------------------------------------------------------------------------
// Blocked Result
// ---------------------------------------------------------------------------

export interface BlockedResult {
	readonly blocked: true;
	readonly reason: string;
}

// ---------------------------------------------------------------------------
// Hook Context & Result Maps
// ---------------------------------------------------------------------------

export interface HookContextMap {
	readonly 'tool.execute.before': { readonly request: ToolCallRequest };
	readonly 'tool.execute.after': {
		readonly request: ToolCallRequest;
		readonly result: ToolCallResult;
	};
	readonly 'tool.result.validate': {
		readonly request: ToolCallRequest;
		readonly result: ToolCallResult;
	};
	readonly 'prompt.system.transform': { readonly prompt: string };
	readonly 'prompt.messages.transform': {
		readonly messages: readonly ConversationMessage[];
	};
	readonly 'session.compacting': {
		readonly messages: readonly ConversationMessage[];
		readonly summary: string;
	};
}

export interface HookResultMap {
	readonly 'tool.execute.before': ToolCallRequest | BlockedResult;
	readonly 'tool.execute.after': ToolCallResult;
	readonly 'tool.result.validate': readonly string[];
	readonly 'prompt.system.transform': string;
	readonly 'prompt.messages.transform': readonly ConversationMessage[];
	readonly 'session.compacting': string;
}

// ---------------------------------------------------------------------------
// Hook Handler
// ---------------------------------------------------------------------------

export type HookType = keyof HookContextMap;

export type HookHandler<T extends HookType> = (
	context: HookContextMap[T],
) => Promise<HookResultMap[T]>;

// ---------------------------------------------------------------------------
// Hook System Interface
// ---------------------------------------------------------------------------

export interface HookSystem {
	readonly register: <T extends HookType>(
		type: T,
		handler: HookHandler<T>,
	) => () => void;
	readonly run: <T extends HookType>(
		type: T,
		context: HookContextMap[T],
	) => Promise<HookResultMap[T]>;
}
