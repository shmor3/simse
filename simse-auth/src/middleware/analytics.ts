import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const analyticsMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	// biome-ignore lint/suspicious/noExplicitAny: cf properties not in Request type
	const cf = (c.req.raw as any).cf;

	try {
		c.env.ANALYTICS_QUEUE.send({
			type: 'datapoint',
			service: 'simse-auth',
			method: c.req.method,
			path: c.req.path,
			status: c.res.status,
			userId: c.req.header('X-User-Id') ?? '',
			teamId: c.req.header('X-Team-Id') ?? '',
			country: cf?.country ?? '',
			city: cf?.city ?? '',
			continent: cf?.continent ?? '',
			userAgent: (c.req.header('User-Agent') ?? '').slice(0, 256),
			referer: (c.req.header('Referer') ?? '').split('?')[0],
			contentType: c.res.headers.get('Content-Type') ?? '',
			cfRay: c.req.header('Cf-Ray') ?? '',
			latencyMs,
			requestSize: Number(c.req.header('Content-Length') ?? 0),
			responseSize: Number(c.res.headers.get('Content-Length') ?? 0),
			colo: Number(cf?.colo ?? 0),
		});
	} catch {
		// Analytics should never block requests
	}
});
