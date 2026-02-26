// ---------------------------------------------------------------------------
// Headless HTTP+SSE Server — Factory implementation
// ---------------------------------------------------------------------------

import { Hono } from 'hono';
import { createSessionManager } from './session-manager.js';
import type { SimseServer, SimseServerConfig } from './types.js';

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

export function createSimseServer(config: SimseServerConfig): SimseServer {
	const host = config.host ?? '127.0.0.1';
	const requestedPort = config.port ?? 3000;

	const sessionManager = createSessionManager();
	const app = new Hono();

	// ---- Health check -------------------------------------------------------
	app.get('/health', (c) =>
		c.json({ status: 'ok', timestamp: Date.now() }, 200),
	);

	// ---- Sessions -----------------------------------------------------------
	app.post('/sessions', (c) => {
		const session = sessionManager.create();
		return c.json({ sessionId: session.id }, 201);
	});

	app.get('/sessions/:id', (c) => {
		const session = sessionManager.get(c.req.param('id'));
		if (!session) {
			return c.json({ error: 'session not found' }, 404);
		}
		return c.json(
			{
				sessionId: session.id,
				status: session.status,
				createdAt: session.createdAt,
				messageCount: session.conversation.messageCount,
			},
			200,
		);
	});

	app.delete('/sessions/:id', (c) => {
		const deleted = sessionManager.delete(c.req.param('id'));
		if (!deleted) {
			return c.json({ error: 'session not found' }, 404);
		}
		return c.json({ deleted: true }, 200);
	});

	// ---- SSE stream ---------------------------------------------------------
	app.get('/sessions/:id/events', (c) => {
		const session = sessionManager.get(c.req.param('id'));
		if (!session) {
			return c.json({ error: 'session not found' }, 404);
		}

		let unsubscribe: (() => void) | undefined;

		const stream = new ReadableStream({
			start(controller) {
				const encoder = new TextEncoder();
				unsubscribe = session.eventBus.subscribeAll((type, payload) => {
					const data = JSON.stringify({ type, payload });
					controller.enqueue(encoder.encode(`data: ${data}\n\n`));
				});
			},
			cancel() {
				unsubscribe?.();
			},
		});

		return new Response(stream, {
			headers: {
				'Content-Type': 'text/event-stream',
				'Cache-Control': 'no-cache',
				Connection: 'keep-alive',
			},
		});
	});

	// ---- Tools --------------------------------------------------------------
	app.get('/tools', (c) => c.json({ tools: [] }, 200));

	// ---- Agents -------------------------------------------------------------
	app.get('/agents', (c) =>
		c.json(
			{
				agents: config.acpServers.map((s) => ({
					name: s.name,
					command: s.command,
					args: s.args,
				})),
			},
			200,
		),
	);

	// ---- Server lifecycle ---------------------------------------------------
	let server: ReturnType<typeof Bun.serve> | undefined;
	let resolvedPort = 0;

	const start = async (): Promise<void> => {
		server = Bun.serve({
			port: requestedPort,
			hostname: host,
			fetch: app.fetch,
		});
		resolvedPort = server.port;
	};

	const stop = async (): Promise<void> => {
		if (server) {
			server.stop(true);
			server = undefined;
		}
	};

	return Object.freeze({
		start,
		stop,
		get port() {
			return resolvedPort;
		},
		get url() {
			return `http://${host}:${resolvedPort}`;
		},
	});
}
