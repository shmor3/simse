import { Hono } from 'hono';
import type { ApiSecrets, Env, ValidateResponse } from '../types';

const tunnels = new Hono<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>();

tunnels.get('/tunnels', async (c) => {
	const authHeader = c.req.header('Authorization');
	if (!authHeader?.startsWith('Bearer ')) {
		return c.json(
			{
				error: {
					code: 'UNAUTHORIZED',
					message: 'Bearer token required',
				},
			},
			401,
		);
	}

	const token = authHeader.slice(7);
	const res = await fetch(`${c.var.secrets.authApiUrl}/auth/validate`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	});

	if (!res.ok) {
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
			401,
		);
	}

	const auth = (await res.json()) as ValidateResponse;
	const userId = auth.data.userId;

	// Check if user has an active tunnel
	const id = c.env.TUNNEL_SESSION.idFromName(userId);
	const stub = c.env.TUNNEL_SESSION.get(id);
	const statusRes = await stub.fetch(new Request('https://internal/status'));
	const status = (await statusRes.json()) as {
		hasSession: boolean;
		hasTunnel: boolean;
		hasClient: boolean;
	};

	return c.json({
		data: {
			tunnels: status.hasTunnel
				? [{ userId, hasTunnel: true, hasClient: status.hasClient }]
				: [],
		},
	});
});

export default tunnels;
