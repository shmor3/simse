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
}

export interface SessionManager {
	readonly create: () => Session;
	readonly get: (id: string) => Session | undefined;
	readonly delete: (id: string) => boolean;
	readonly list: () => readonly Session[];
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
		const session: Session = Object.freeze({
			id,
			conversation: createConversation(),
			eventBus: createEventBus(),
			status: 'active' as const,
			createdAt: Date.now(),
		});
		sessions.set(id, session);
		return session;
	};

	const get = (id: string): Session | undefined => sessions.get(id);

	const del = (id: string): boolean => sessions.delete(id);

	const list = (): readonly Session[] => Object.freeze([...sessions.values()]);

	return Object.freeze({
		create,
		get,
		delete: del,
		list,
	});
}
