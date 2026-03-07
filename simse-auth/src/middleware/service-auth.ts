import { createMiddleware } from 'hono/factory';
import { timingSafeEqual } from '../lib/timing-safe';
import type { Env } from '../types';

/**
 * Validates requests came from the API gateway by checking a shared secret.
 * Prevents direct access to protected routes on the public auth.simse.dev domain.
 */
export const serviceAuthMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	const secret = await c.env.SECRETS.get('AUTH_API_SECRET');
	if (!secret) {
		// Fail closed — deny access when secret is not configured
		return c.json(
			{
				error: {
					code: 'MISCONFIGURED',
					message: 'Service not configured',
				},
			},
			500,
		);
	}

	const authHeader = c.req.header('Authorization') ?? '';
	const expected = `Bearer ${secret}`;
	if (!timingSafeEqual(authHeader, expected)) {
		return c.json(
			{
				error: {
					code: 'FORBIDDEN',
					message: 'Direct access not allowed',
				},
			},
			403,
		);
	}
	await next();
});
