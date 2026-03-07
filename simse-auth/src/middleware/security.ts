import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const securityHeadersMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	await next();

	c.header('X-Content-Type-Options', 'nosniff');
	c.header('X-Frame-Options', 'DENY');
	c.header('Strict-Transport-Security', 'max-age=31536000; includeSubDomains');
});
