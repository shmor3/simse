import type { Context, Next } from 'hono';
import type { Env } from '../types';

export async function authMiddleware(
	c: Context<{ Bindings: Env }>,
	next: Next,
) {
	const authHeader = c.req.header('Authorization');
	if (authHeader !== `Bearer ${c.env.API_SECRET}`) {
		return c.json({ error: 'Unauthorized' }, 401);
	}
	await next();
}
