import { Hono } from 'hono';
import type { Env, ValidateResponse } from '../types';

const gateway = new Hono<{ Bindings: Env }>();

// Public auth routes — proxy directly without validation
const PUBLIC_AUTH_PATHS = ['/register', '/login', '/2fa', '/reset-password', '/new-password', '/verify-email'];

gateway.all('/auth/*', async (c) => {
	const subpath = c.req.path.replace('/auth', '');
	const isPublic = PUBLIC_AUTH_PATHS.some((p) => subpath === p);

	const headers = new Headers();
	headers.set('Content-Type', 'application/json');

	// For protected auth routes, validate first
	if (!isPublic) {
		const auth = await validateToken(c);
		if (!auth) {
			return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
		}
		headers.set('X-User-Id', auth.userId);
		if (auth.sessionId) headers.set('X-Session-Id', auth.sessionId);
		if (auth.teamId) headers.set('X-Team-Id', auth.teamId);
		if (auth.role) headers.set('X-Role', auth.role);
	}

	return proxyTo(c, `${c.env.AUTH_API_URL}${c.req.path}`, headers);
});

// Protected service routes
for (const prefix of ['/users', '/teams', '/api-keys']) {
	gateway.all(`${prefix}/*`, async (c) => {
		const auth = await validateToken(c);
		if (!auth) {
			return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
		}

		const headers = serviceHeaders(auth);
		return proxyTo(c, `${c.env.AUTH_API_URL}${c.req.path}`, headers);
	});
}

// Payments proxy
gateway.all('/payments/*', async (c) => {
	const auth = await validateToken(c);
	if (!auth) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	const path = c.req.path.replace('/payments', '');
	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.env.PAYMENTS_API_SECRET}`);
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);
	if (auth.teamId) headers.set('X-Team-Id', auth.teamId);

	return proxyTo(c, `${c.env.PAYMENTS_API_URL}${path}`, headers);
});

// Notifications proxy (to mailer)
gateway.all('/notifications', async (c) => {
	return proxyNotifications(c);
});
gateway.all('/notifications/*', async (c) => {
	return proxyNotifications(c);
});

async function proxyNotifications(c: any) {
	const auth = await validateToken(c);
	if (!auth) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	// POST /notifications → enqueue (fire-and-forget)
	if (c.req.method === 'POST') {
		const body = await c.req.json();
		await c.env.COMMS_QUEUE.send({
			type: 'notification',
			userId: auth.userId,
			...body,
		});
		return c.json({ data: { ok: true } });
	}

	// GET/PUT → proxy to mailer HTTP (needs response)
	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);

	return proxyTo(c, `${c.env.MAILER_API_URL}${c.req.path}`, headers);
}

// --- Helpers ---

async function validateToken(c: any): Promise<ValidateResponse['data'] | null> {
	const authHeader = c.req.header('Authorization');
	if (!authHeader?.startsWith('Bearer ')) return null;

	const token = authHeader.slice(7);

	const res = await fetch(`${c.env.AUTH_API_URL}/auth/validate`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	});

	if (!res.ok) return null;

	const json = await res.json() as ValidateResponse;
	return json.data;
}

function serviceHeaders(auth: ValidateResponse['data']): Headers {
	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);
	if (auth.sessionId) headers.set('X-Session-Id', auth.sessionId);
	if (auth.teamId) headers.set('X-Team-Id', auth.teamId);
	if (auth.role) headers.set('X-Role', auth.role);
	return headers;
}

async function proxyTo(c: any, url: string, headers: Headers): Promise<Response> {
	const init: RequestInit = {
		method: c.req.method,
		headers,
	};

	if (!['GET', 'HEAD'].includes(c.req.method)) {
		init.body = await c.req.text();
	}

	const res = await fetch(url, init);
	const body = await res.text();

	return new Response(body, {
		status: res.status,
		headers: { 'Content-Type': 'application/json' },
	});
}

export default gateway;
