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

	// /auth/validate is internal-only (gateway calls it for token validation)
	if (subpath === '/validate') {
		return c.json(
			{ error: { code: 'NOT_FOUND', message: 'Route not found' } },
			404,
		);
	}

	const isPublic = PUBLIC_AUTH_PATHS.some((p) => subpath === p);

	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	headers.set('X-Request-Id', c.get('requestId') ?? '');
	setClientIp(headers, c);

	if (!isPublic) {
		const auth = await authenticateRequest(c);
		if (!auth) {
			return c.json(
				{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
				401,
			);
		}
		headers.set('Authorization', `Bearer ${c.var.secrets.authApiSecret}`);
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
	setClientIp(headers, c);
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
				title:
					typeof body.title === 'string' ? body.title.slice(0, 500) : undefined,
				message:
					typeof body.message === 'string'
						? body.message.slice(0, 5000)
						: undefined,
				kind:
					typeof body.kind === 'string' ? body.kind.slice(0, 50) : undefined,
				link:
					typeof body.link === 'string' ? body.link.slice(0, 2000) : undefined,
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
	setClientIp(headers, c);

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

		// Only forward WebSocket upgrade headers — don't pass raw client headers
		// which could include spoofed X-User-Id or Authorization
		const wsHeaders = new Headers();
		const upgrade = c.req.header('Upgrade');
		if (upgrade) wsHeaders.set('Upgrade', upgrade);
		const connection = c.req.header('Connection');
		if (connection) wsHeaders.set('Connection', connection);
		const wsKey = c.req.header('Sec-WebSocket-Key');
		if (wsKey) wsHeaders.set('Sec-WebSocket-Key', wsKey);
		const wsVersion = c.req.header('Sec-WebSocket-Version');
		if (wsVersion) wsHeaders.set('Sec-WebSocket-Version', wsVersion);
		const wsProtocol = c.req.header('Sec-WebSocket-Protocol');
		if (wsProtocol) wsHeaders.set('Sec-WebSocket-Protocol', wsProtocol);
		const wsExtensions = c.req.header('Sec-WebSocket-Extensions');
		if (wsExtensions) wsHeaders.set('Sec-WebSocket-Extensions', wsExtensions);

		return c.env.CLOUD_SERVICE.fetch(
			new Request(url.toString(), { headers: wsHeaders }),
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
		headers: {
			'Content-Type': 'application/json',
			Authorization: `Bearer ${c.var.secrets.authApiSecret}`,
		},
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

function setClientIp(headers: Headers, c: GatewayContext): void {
	const clientIp = c.req.header('CF-Connecting-IP');
	if (clientIp) headers.set('X-Forwarded-For', clientIp);
}

function serviceHeaders(auth: AuthResult, c: GatewayContext): Headers {
	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	headers.set('Authorization', `Bearer ${c.var.secrets.authApiSecret}`);
	headers.set('X-Request-Id', c.get('requestId') ?? '');
	setClientIp(headers, c);
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

	// Only allow safe Content-Types — prevent XSS via compromised backend
	const rawCt = res.headers.get('Content-Type') ?? 'application/json';
	const contentType = rawCt.startsWith('application/json')
		? rawCt
		: 'application/json';
	return new Response(res.body, {
		status: res.status,
		headers: { 'Content-Type': contentType },
	});
}

export default gateway;
