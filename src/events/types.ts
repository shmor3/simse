// ---------------------------------------------------------------------------
// EventBus — Type definitions
// ---------------------------------------------------------------------------

/**
 * Exhaustive map of every event the system can publish.
 * Each key is a dotted event name; its value is the readonly payload shape.
 */
export interface EventPayloadMap {
	'session.created': { readonly sessionId: string };
	'session.prompt': { readonly sessionId: string; readonly prompt: string };
	'session.completed': { readonly sessionId: string; readonly result: unknown };
	'session.error': { readonly sessionId: string; readonly error: Error };
	'stream.delta': { readonly text: string };
	'stream.complete': { readonly text: string };
	'tool.call.start': {
		readonly callId: string;
		readonly name: string;
		readonly args: Record<string, unknown>;
	};
	'tool.call.end': {
		readonly callId: string;
		readonly name: string;
		readonly output: string;
		readonly isError: boolean;
		readonly durationMs: number;
	};
	'tool.call.error': {
		readonly callId: string;
		readonly name: string;
		readonly error: Error;
	};
	'turn.complete': {
		readonly turn: number;
		readonly type: 'text' | 'tool_use';
	};
	'compaction.start': {
		readonly messageCount: number;
		readonly estimatedChars: number;
	};
	'compaction.prune': {
		readonly messageCount: number;
		readonly estimatedChars: number;
	};
	'compaction.complete': { readonly summaryLength: number };
	'permission.request': {
		readonly callId: string;
		readonly toolName: string;
		readonly args: Record<string, unknown>;
	};
	'permission.response': {
		readonly callId: string;
		readonly allowed: boolean;
	};
	abort: { readonly reason: string };
	// Memory events
	'memory.add': { readonly id: string; readonly contentLength: number };
	'memory.search': {
		readonly query: string;
		readonly resultCount: number;
		readonly durationMs: number;
	};
	'memory.delete': { readonly id: string };
	// Subagent events
	'subagent.start': {
		readonly subagentId: string;
		readonly type: string;
		readonly task: string;
	};
	'subagent.complete': {
		readonly subagentId: string;
		readonly type: string;
		readonly durationMs: number;
	};
	'subagent.error': {
		readonly subagentId: string;
		readonly type: string;
		readonly error: Error;
	};
}

/** Union of all recognised event names. */
export type EventType = keyof EventPayloadMap;

/** Payload type for a specific event. */
export type EventPayload<T extends EventType> = EventPayloadMap[T];

/** Handler function for a specific event type. */
export type EventHandler<T extends EventType> = (
	payload: EventPayload<T>,
) => void;

// ---------------------------------------------------------------------------
// EventBus interface
// ---------------------------------------------------------------------------

/**
 * A typed, synchronous publish/subscribe event bus.
 *
 * - `publish` delivers a payload to every subscriber of the given event type.
 * - `subscribe` registers a handler and returns an unsubscribe function.
 * - `subscribeAll` registers a wildcard handler that receives every event.
 */
export interface EventBus {
	readonly publish: <T extends EventType>(
		type: T,
		payload: EventPayload<T>,
	) => void;
	readonly subscribe: <T extends EventType>(
		type: T,
		handler: EventHandler<T>,
	) => () => void;
	readonly subscribeAll: (
		handler: (type: EventType, payload: unknown) => void,
	) => () => void;
}
