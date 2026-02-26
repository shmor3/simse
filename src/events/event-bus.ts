// ---------------------------------------------------------------------------
// EventBus — Factory implementation
// ---------------------------------------------------------------------------

import type {
	EventBus,
	EventHandler,
	EventPayload,
	EventType,
} from './types.js';

/**
 * Create a typed event bus that decouples event producers from consumers.
 *
 * - Handlers are invoked synchronously in registration order.
 * - Errors thrown by individual handlers are caught and logged to
 *   `console.error`; they never propagate to other handlers or the
 *   publisher.
 * - `subscribe` returns an idempotent unsubscribe function.
 * - `subscribeAll` registers a wildcard listener that fires for every
 *   event type.
 */
export const createEventBus = (): EventBus => {
	/** Per-event-type handler sets. */
	const handlers = new Map<EventType, Set<EventHandler<EventType>>>();

	/** Wildcard handlers that receive every event. */
	const globalHandlers = new Set<(type: EventType, payload: unknown) => void>();

	const publish = <T extends EventType>(
		type: T,
		payload: EventPayload<T>,
	): void => {
		const set = handlers.get(type);
		if (set) {
			for (const handler of set) {
				try {
					(handler as EventHandler<T>)(payload);
				} catch (err) {
					console.error(`[EventBus] handler error for "${type}":`, err);
				}
			}
		}

		for (const handler of globalHandlers) {
			try {
				handler(type, payload);
			} catch (err) {
				console.error(`[EventBus] global handler error for "${type}":`, err);
			}
		}
	};

	const subscribe = <T extends EventType>(
		type: T,
		handler: EventHandler<T>,
	): (() => void) => {
		let set = handlers.get(type);
		if (!set) {
			set = new Set();
			handlers.set(type, set);
		}
		set.add(handler as EventHandler<EventType>);

		return () => {
			set.delete(handler as EventHandler<EventType>);
		};
	};

	const subscribeAll = (
		handler: (type: EventType, payload: unknown) => void,
	): (() => void) => {
		globalHandlers.add(handler);
		return () => {
			globalHandlers.delete(handler);
		};
	};

	return Object.freeze({ publish, subscribe, subscribeAll });
};
