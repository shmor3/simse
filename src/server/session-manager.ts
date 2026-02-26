// ---------------------------------------------------------------------------
// Session Manager — Factory implementation
// ---------------------------------------------------------------------------

import { createConversation } from '../ai/conversation/conversation.js';
import type { Conversation } from '../ai/conversation/types.js';
import { createEventBus } from '../events/event-bus.js';
import type { EventBus } from '../events/types.js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type SessionStatus = 'active' | 'completed' | 'aborted';

export interface Session {
	readonly id: string;
	readonly conversation: Conversation;
	readonly eventBus: EventBus;
	readonly status: SessionStatus;
	readonly createdAt: number;
	readonly updatedAt: number;
}

export interface SessionManager {
	readonly create: () => Session;
	readonly get: (id: string) => Session | undefined;
	readonly delete: (id: string) => boolean;
	readonly list: () => readonly Session[];
	readonly updateStatus: (
		id: string,
		status: SessionStatus,
	) => Session | undefined;
	/** Fork a session — create a new session with cloned conversation state. */
	readonly fork: (id: string) => Session | undefined;
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

let idCounter = 0;

const generateId = (): string => {
	idCounter += 1;
	return `sess_${Date.now()}_${idCounter.toString(36)}`;
};

export function createSessionManager(): SessionManager {
	const sessions = new Map<string, Session>();

	const create = (): Session => {
		const id = generateId();
		const now = Date.now();
		const session: Session = Object.freeze({
			id,
			conversation: createConversation(),
			eventBus: createEventBus(),
			status: 'active' as const,
			createdAt: now,
			updatedAt: now,
		});
		sessions.set(id, session);
		return session;
	};

	const get = (id: string): Session | undefined => sessions.get(id);

	const del = (id: string): boolean => sessions.delete(id);

	const list = (): readonly Session[] => Object.freeze([...sessions.values()]);

	const updateStatus = (
		id: string,
		status: SessionStatus,
	): Session | undefined => {
		const existing = sessions.get(id);
		if (!existing) return undefined;
		const updated: Session = Object.freeze({
			...existing,
			status,
			updatedAt: Date.now(),
		});
		sessions.set(id, updated);
		return updated;
	};

	const fork = (id: string): Session | undefined => {
		const existing = sessions.get(id);
		if (!existing) return undefined;

		const newId = generateId();
		const now = Date.now();
		const newConversation = createConversation();

		// Clone conversation state via JSON serialization
		newConversation.fromJSON(existing.conversation.toJSON());

		const forked: Session = Object.freeze({
			id: newId,
			conversation: newConversation,
			eventBus: createEventBus(),
			status: 'active' as const,
			createdAt: now,
			updatedAt: now,
		});
		sessions.set(newId, forked);
		return forked;
	};

	return Object.freeze({
		create,
		get,
		delete: del,
		list,
		updateStatus,
		fork,
	});
}
