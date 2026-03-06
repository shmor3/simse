import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const analyticsMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	const cf = (c.req.raw as Request & { cf?: IncomingRequestCfProperties }).cf;

	c.env.ANALYTICS.writeDataPoint({
		indexes: ['simse-api'],
		blobs: [
			c.req.method,
			c.req.path,
			String(c.res.status),
			'simse-api',
			c.req.header('X-User-Id') ?? '',
			c.req.header('X-Team-Id') ?? '',
			cf?.country ?? '',
			cf?.city ?? '',
			cf?.continent ?? '',
			(c.req.header('User-Agent') ?? '').slice(0, 256),
			c.req.header('Referer') ?? '',
			c.res.headers.get('Content-Type') ?? '',
			c.req.header('Cf-Ray') ?? '',
		],
		doubles: [
			latencyMs,
			c.res.status,
			Number(c.req.header('Content-Length') ?? 0),
			Number(c.res.headers.get('Content-Length') ?? 0),
			Number(cf?.colo ?? 0),
		],
	});
});
