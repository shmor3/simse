import type { Context, Next } from 'hono';
import { validateApiKey } from '../lib/api-key';
import { validateSession } from '../lib/session';
import type { AuthContext, Env } from '../types';

export async function authMiddleware(
	c: Context<{ Bindings: Env; Variables: { auth: AuthContext } }>,
	next: Next,
) {
	const authHeader = c.req.header('Authorization');
	if (!authHeader?.startsWith('Bearer ')) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Missing authorization' } }, 401);
	}

	const token = authHeader.slice(7);
	const db = c.env.DB;

	if (token.startsWith('session_')) {
		const userId = await validateSession(db, token);
		if (!userId) {
			return c.json({ error: { code: 'SESSION_EXPIRED', message: 'Session expired or invalid' } }, 401);
		}
		c.set('auth', { userId, sessionId: token });
	} else if (token.startsWith('sk_')) {
		const userId = await validateApiKey(db, token);
		if (!userId) {
			return c.json({ error: { code: 'INVALID_API_KEY', message: 'Invalid API key' } }, 401);
		}
		c.set('auth', { userId });
	} else {
		return c.json({ error: { code: 'INVALID_TOKEN', message: 'Unrecognized token format' } }, 401);
	}

	await next();
}
