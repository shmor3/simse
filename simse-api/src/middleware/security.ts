import { createMiddleware } from 'hono/factory';
import type { AppVariables, Env } from '../types';

const MAX_BODY_SIZE = 1_048_576; // 1MB

export const requestValidationMiddleware = createMiddleware<{
	Bindings: Env;
	Variables: AppVariables;
}>(async (c, next) => {
	// Generate or pass through correlation ID
	const requestId = c.req.header('X-Request-Id') ?? crypto.randomUUID();
	c.set('requestId', requestId);

	// Validate Content-Type on request bodies
	if (['POST', 'PUT', 'PATCH'].includes(c.req.method)) {
		const contentType = c.req.header('Content-Type');
		if (contentType && !contentType.includes('application/json')) {
			return c.json(
				{
					error: {
						code: 'UNSUPPORTED_MEDIA_TYPE',
						message: 'Content-Type must be application/json',
					},
					requestId,
				},
				415,
			);
		}

		const contentLength = Number(c.req.header('Content-Length') ?? 0);
		if (contentLength > MAX_BODY_SIZE) {
			return c.json(
				{
					error: {
						code: 'PAYLOAD_TOO_LARGE',
						message: 'Request body exceeds 1MB limit',
					},
					requestId,
				},
				413,
			);
		}
	}

	await next();

	// Set security headers on all responses
	c.header('X-Request-Id', requestId);
	c.header('X-Content-Type-Options', 'nosniff');

	// Strip leaked backend headers
	c.res.headers.delete('Server');
	c.res.headers.delete('X-Powered-By');
});
