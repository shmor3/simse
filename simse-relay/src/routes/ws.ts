import { Hono } from 'hono';
import type { ApiSecrets, Env, ValidateResponse } from '../types';

const ws = new Hono<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>();

ws.get('/ws/tunnel', async (c) => {
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

	const auth = await validateToken(c.var.secrets.authApiUrl, token);
	if (!auth) {
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
			401,
		);
	}

	// Route to Durable Object keyed by userId
	const id = c.env.TUNNEL_SESSION.idFromName(auth.userId);
	const stub = c.env.TUNNEL_SESSION.get(id);

	const url = new URL(c.req.url);
	url.searchParams.set('userId', auth.userId);
	const doRequest = new Request(url.toString(), {
		headers: c.req.raw.headers,
	});

	return stub.fetch(doRequest);
});

ws.get('/ws/client', async (c) => {
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

	const auth = await validateToken(c.var.secrets.authApiUrl, token);
	if (!auth) {
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
			401,
		);
	}

	// Route to same Durable Object as the user's tunnel
	const id = c.env.TUNNEL_SESSION.idFromName(auth.userId);
	const stub = c.env.TUNNEL_SESSION.get(id);

	const url = new URL(c.req.url);
	url.searchParams.set('userId', auth.userId);
	const doRequest = new Request(url.toString(), {
		headers: c.req.raw.headers,
	});

	return stub.fetch(doRequest);
});

async function validateToken(
	authApiUrl: string,
	token: string,
): Promise<ValidateResponse['data'] | null> {
	const res = await fetch(`${authApiUrl}/auth/validate`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	});

	if (!res.ok) return null;

	const json = (await res.json()) as ValidateResponse;
	return json.data;
}

export default ws;
