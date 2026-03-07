import { type Context, Hono } from 'hono';
import { breakers } from '../index';
import type { CircuitBreaker } from '../lib/circuit-breaker';
import { verifyJwt } from '../lib/jwt';
import { resilientFetch } from '../lib/resilient-fetch';
import type { AppVariables, Env, ValidateResponse } from '../types';

type GatewayContext = Context<{ Bindings: Env; Variables: AppVariables }>;

const gateway = new Hono<{
	Bindings: Env;
	Variables: AppVariables;
}>();

const PUBLIC_AUTH_PATHS = [
	'/register',
	'/login',
	'/2fa',
	'/reset-password',
	'/new-password',
	'/verify-email',
	'/refresh',
	'/revoke',
];

// --- Auth routes ---
gateway.all('/auth/*', async (c) => {
	const subpath = c.req.path.replace('/auth', '');
	const isPublic = PUBLIC_AUTH_PATHS.some((p) => subpath === p);

	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	headers.set('X-Request-Id', c.get('requestId') ?? '');

	if (!isPublic) {
		const auth = await authenticateRequest(c);
		if (!auth) {
			return c.json(
				{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
				401,
			);
		}
		setAuthHeaders(headers, auth);
	}

	return proxyTo(
		c,
		`${c.var.secrets.authApiUrl}${c.req.path}`,
		headers,
		breakers.auth,
	);
});

// --- Protected service routes ---
for (const prefix of ['/users', '/teams', '/api-keys']) {
	gateway.all(`${prefix}/*`, async (c) => {
		const auth = await authenticateRequest(c);
		if (!auth) {
			return c.json(
				{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
				401,
			);
		}

		const headers = serviceHeaders(auth, c);
		return proxyTo(
			c,
			`${c.var.secrets.authApiUrl}${c.req.path}`,
			headers,
			breakers.auth,
		);
	});
}

// --- Payments proxy ---
gateway.all('/payments/*', async (c) => {
	const auth = await authenticateRequest(c);
	if (!auth) {
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
			401,
		);
	}

	const path = c.req.path.replace('/payments', '');
	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.var.secrets.paymentsApiSecret}`);
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);
	headers.set('X-Request-Id', c.get('requestId') ?? '');
	if (auth.teamId) headers.set('X-Team-Id', auth.teamId);

	return proxyTo(
		c,
		`${c.var.secrets.paymentsApiUrl}${path}`,
		headers,
		breakers.payments,
	);
});

// --- Notifications proxy ---
gateway.all('/notifications', proxyNotifications);
gateway.all('/notifications/*', proxyNotifications);

async function proxyNotifications(c: GatewayContext) {
	const auth = await authenticateRequest(c);
	if (!auth) {
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
			401,
		);
	}

	if (c.req.method === 'POST') {
		let body: Record<string, unknown>;
		try {
			body = await c.req.json();
		} catch {
			return c.json(
				{ error: { code: 'INVALID_BODY', message: 'Invalid JSON body' } },
				400,
			);
		}

		try {
			await c.env.COMMS_QUEUE.send({
				type: 'notification',
				userId: auth.userId,
				...body,
			});
		} catch {
			return c.json(
				{
					error: {
						code: 'SERVICE_UNAVAILABLE',
						message: 'Notification service unavailable',
					},
				},
				503,
			);
		}
		return c.json({ data: { ok: true } });
	}

	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.var.secrets.mailerApiSecret}`);
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);
	headers.set('X-Request-Id', c.get('requestId') ?? '');

	return proxyTo(
		c,
		`${c.var.secrets.mailerApiUrl}${c.req.path}`,
		headers,
		breakers.mailer,
	);
}

// --- Relay proxy (WebSocket tunnel + REST) ---
for (const path of ['/ws/tunnel', '/ws/client']) {
	gateway.get(path, async (c) => {
		// WebSocket clients pass token as query param (can't set headers)
		const token = c.req.query('token');
		if (!token) {
			return c.json(
				{
					error: {
						code: 'MISSING_TOKEN',
						message: 'token query param required',
					},
				},
				401,
			);
		}

		const auth = await authenticateToken(c, token);
		if (!auth) {
			return c.json(
				{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
				401,
			);
		}

		// Forward to simse-cloud with userId; strip token from query
		const url = new URL(c.req.url);
		url.searchParams.delete('token');
		url.searchParams.set('userId', auth.userId);

		return c.env.CLOUD_SERVICE.fetch(
			new Request(url.toString(), {
				headers: c.req.raw.headers,
			}),
		);
	});
}

gateway.get('/tunnels', async (c) => {
	const auth = await authenticateRequest(c);
	if (!auth) {
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
			401,
		);
	}

	const url = new URL(c.req.url);
	const headers = new Headers();
	headers.set('X-User-Id', auth.userId);
	headers.set('X-Request-Id', c.get('requestId') ?? '');

	return c.env.CLOUD_SERVICE.fetch(new Request(url.toString(), { headers }));
});

// --- Helpers ---

interface AuthResult {
	userId: string;
	sessionId?: string;
	teamId: string | null;
	role: string | null;
}

async function authenticateRequest(
	c: GatewayContext,
): Promise<AuthResult | null> {
	const authHeader = c.req.header('Authorization');
	if (!authHeader?.startsWith('Bearer ')) return null;

	return authenticateToken(c, authHeader.slice(7));
}

async function authenticateToken(
	c: GatewayContext,
	token: string,
): Promise<AuthResult | null> {
	// API keys — validate via auth service
	if (token.startsWith('sk_')) {
		return validateTokenViaService(c, token);
	}

	// JWT access token — validate locally
	const jwtSecret = c.var.secrets.jwtSecret;
	const result = await verifyJwt(token, jwtSecret);

	if (!result) {
		// Not a valid JWT — try legacy session token validation
		if (token.startsWith('session_')) {
			return validateTokenViaService(c, token);
		}
		return null;
	}

	if (result.expired) {
		return null;
	}

	return {
		userId: result.payload.sub,
		sessionId: result.payload.sid,
		teamId: result.payload.tid,
		role: result.payload.role,
	};
}

async function validateTokenViaService(
	c: GatewayContext,
	token: string,
): Promise<AuthResult | null> {
	const init: RequestInit = {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	};

	const res = await resilientFetch(
		`${c.var.secrets.authApiUrl}/auth/validate`,
		init,
		breakers.auth,
	);

	if (!res.ok) return null;

	try {
		const json = (await res.json()) as ValidateResponse;
		return json.data;
	} catch {
		return null;
	}
}

function setAuthHeaders(headers: Headers, auth: AuthResult): void {
	headers.set('X-User-Id', auth.userId);
	if (auth.sessionId) headers.set('X-Session-Id', auth.sessionId);
	if (auth.teamId) headers.set('X-Team-Id', auth.teamId);
	if (auth.role) headers.set('X-Role', auth.role);
}

function serviceHeaders(auth: AuthResult, c: GatewayContext): Headers {
	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	headers.set('X-Request-Id', c.get('requestId') ?? '');
	setAuthHeaders(headers, auth);
	return headers;
}

async function proxyTo(
	c: GatewayContext,
	url: string,
	headers: Headers,
	breaker: CircuitBreaker,
): Promise<Response> {
	const init: RequestInit = {
		method: c.req.method,
		headers,
	};

	if (!['GET', 'HEAD'].includes(c.req.method)) {
		init.body = await c.req.text();
	}

	const res = await resilientFetch(url, init, breaker);

	// Stream response, preserve original Content-Type
	const contentType = res.headers.get('Content-Type') ?? 'application/json';
	return new Response(res.body, {
		status: res.status,
		headers: { 'Content-Type': contentType },
	});
}

export default gateway;
