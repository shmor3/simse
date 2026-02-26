import { afterEach, beforeEach, describe, expect, it } from 'bun:test';
import { createSimseServer } from '../src/server/server.js';
import type { SimseServer } from '../src/server/types.js';

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

const testConfig = {
	port: 0,
	host: '127.0.0.1',
	acpServers: [{ name: 'test-agent', command: 'echo', args: ['hello'] }],
	workingDirectory: '.',
};

// ---------------------------------------------------------------------------
// Server tests
// ---------------------------------------------------------------------------

describe('SimseServer', () => {
	let server: SimseServer;

	beforeEach(async () => {
		server = createSimseServer(testConfig);
		await server.start();
	});

	afterEach(async () => {
		await server.stop();
	});

	// ---- Health check -----------------------------------------------------

	it('responds to health check', async () => {
		const res = await fetch(`${server.url}/health`);
		expect(res.status).toBe(200);
		const body = await res.json();
		expect(body.status).toBe('ok');
		expect(typeof body.timestamp).toBe('number');
	});

	// ---- Session CRUD -----------------------------------------------------

	it('creates a session', async () => {
		const res = await fetch(`${server.url}/sessions`, { method: 'POST' });
		expect(res.status).toBe(201);
		const body = await res.json();
		expect(typeof body.sessionId).toBe('string');
		expect(body.sessionId.length).toBeGreaterThan(0);
	});

	it('gets session state', async () => {
		const createRes = await fetch(`${server.url}/sessions`, {
			method: 'POST',
		});
		const { sessionId } = await createRes.json();

		const res = await fetch(`${server.url}/sessions/${sessionId}`);
		expect(res.status).toBe(200);
		const body = await res.json();
		expect(body.sessionId).toBe(sessionId);
		expect(body.status).toBe('active');
		expect(typeof body.createdAt).toBe('number');
		expect(body.messageCount).toBe(0);
	});

	it('returns 404 for unknown session', async () => {
		const res = await fetch(`${server.url}/sessions/nonexistent`);
		expect(res.status).toBe(404);
	});

	it('deletes a session', async () => {
		const createRes = await fetch(`${server.url}/sessions`, {
			method: 'POST',
		});
		const { sessionId } = await createRes.json();

		const deleteRes = await fetch(`${server.url}/sessions/${sessionId}`, {
			method: 'DELETE',
		});
		expect(deleteRes.status).toBe(200);
		const body = await deleteRes.json();
		expect(body.deleted).toBe(true);

		// Verify session is gone
		const getRes = await fetch(`${server.url}/sessions/${sessionId}`);
		expect(getRes.status).toBe(404);
	});

	// ---- Tools & Agents ---------------------------------------------------

	it('lists available tools', async () => {
		const res = await fetch(`${server.url}/tools`);
		expect(res.status).toBe(200);
		const body = await res.json();
		expect(Array.isArray(body.tools)).toBe(true);
	});

	it('lists configured agents', async () => {
		const res = await fetch(`${server.url}/agents`);
		expect(res.status).toBe(200);
		const body = await res.json();
		expect(Array.isArray(body.agents)).toBe(true);
		expect(body.agents).toHaveLength(1);
		expect(body.agents[0].name).toBe('test-agent');
		expect(body.agents[0].command).toBe('echo');
	});

	// ---- SSE stream -------------------------------------------------------

	it('returns 404 for SSE on unknown session', async () => {
		const res = await fetch(`${server.url}/sessions/nonexistent/events`);
		expect(res.status).toBe(404);
	});

	it('returns SSE content-type for events endpoint', async () => {
		const createRes = await fetch(`${server.url}/sessions`, {
			method: 'POST',
		});
		const { sessionId } = await createRes.json();

		// Use a short-lived abort so the test doesn't hang on the open stream
		const controller = new AbortController();
		const fetchPromise = fetch(`${server.url}/sessions/${sessionId}/events`, {
			signal: controller.signal,
		});

		// Give the server a moment to start the response, then abort
		await new Promise((r) => setTimeout(r, 50));
		controller.abort();

		// fetch may reject with AbortError or may have already resolved headers
		try {
			const res = await fetchPromise;
			expect(res.headers.get('content-type')).toBe('text/event-stream');
			expect(res.headers.get('cache-control')).toBe('no-cache');
		} catch (err: unknown) {
			// AbortError is expected when the stream is interrupted
			if (err instanceof Error && err.name !== 'AbortError') {
				throw err;
			}
		}
	});
});
