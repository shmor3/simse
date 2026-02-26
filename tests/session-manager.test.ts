import { describe, expect, it } from 'bun:test';
import { createSessionManager } from '../src/server/session-manager.js';

describe('session manager', () => {
	it('creates a session with active status', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		expect(session.id).toMatch(/^sess_/);
		expect(session.status).toBe('active');
		expect(session.createdAt).toBeGreaterThan(0);
		expect(session.updatedAt).toBe(session.createdAt);
	});

	it('retrieves a session by id', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		expect(mgr.get(session.id)).toBe(session);
	});

	it('returns undefined for unknown id', () => {
		const mgr = createSessionManager();
		expect(mgr.get('unknown')).toBeUndefined();
	});

	it('deletes a session', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		expect(mgr.delete(session.id)).toBe(true);
		expect(mgr.get(session.id)).toBeUndefined();
	});

	it('lists all sessions', () => {
		const mgr = createSessionManager();
		mgr.create();
		mgr.create();
		expect(mgr.list()).toHaveLength(2);
	});

	it('updateStatus transitions active to completed', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		const updated = mgr.updateStatus(session.id, 'completed');
		expect(updated).toBeDefined();
		expect(updated!.status).toBe('completed');
		expect(updated!.updatedAt).toBeGreaterThanOrEqual(session.updatedAt);
		expect(updated!.id).toBe(session.id);
		// Conversation and eventBus should be preserved
		expect(updated!.conversation).toBe(session.conversation);
		expect(updated!.eventBus).toBe(session.eventBus);
	});

	it('updateStatus transitions active to aborted', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		const updated = mgr.updateStatus(session.id, 'aborted');
		expect(updated).toBeDefined();
		expect(updated!.status).toBe('aborted');
	});

	it('updateStatus returns undefined for unknown id', () => {
		const mgr = createSessionManager();
		expect(mgr.updateStatus('unknown', 'completed')).toBeUndefined();
	});

	it('updateStatus replaces session in manager', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		mgr.updateStatus(session.id, 'completed');
		const retrieved = mgr.get(session.id);
		expect(retrieved!.status).toBe('completed');
	});

	it('updated session is frozen', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		const updated = mgr.updateStatus(session.id, 'completed');
		expect(Object.isFrozen(updated)).toBe(true);
	});

	it('forks a session with cloned conversation', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		session.conversation.addUser('Hello');
		session.conversation.addAssistant('Hi there');

		const forked = mgr.fork(session.id);
		expect(forked).toBeDefined();
		expect(forked!.id).not.toBe(session.id);
		expect(forked!.status).toBe('active');
		// Conversation content is cloned
		const cloned = forked!.conversation.serialize();
		expect(cloned).toContain('Hello');
		expect(cloned).toContain('Hi there');
		// But it's a different conversation object
		expect(forked!.conversation).not.toBe(session.conversation);
	});

	it('forked session has independent conversation', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		session.conversation.addUser('Initial');

		const forked = mgr.fork(session.id)!;
		forked.conversation.addUser('Forked message');

		// Original should not have the new message
		expect(session.conversation.serialize()).not.toContain('Forked message');
		expect(forked.conversation.serialize()).toContain('Forked message');
	});

	it('fork returns undefined for unknown id', () => {
		const mgr = createSessionManager();
		expect(mgr.fork('unknown')).toBeUndefined();
	});

	it('forked session is frozen', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		const forked = mgr.fork(session.id);
		expect(Object.isFrozen(forked)).toBe(true);
	});

	it('forked session is retrievable by id', () => {
		const mgr = createSessionManager();
		const session = mgr.create();
		const forked = mgr.fork(session.id)!;
		expect(mgr.get(forked.id)).toBe(forked);
	});
});
