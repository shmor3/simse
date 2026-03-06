import { Hono } from 'hono';
import { createMiddleware } from 'hono/factory';
import tunnels from './routes/tunnels';
import ws from './routes/ws';
import type { ApiSecrets, Env } from './types';

export { TunnelSession } from './tunnel';

const app = new Hono<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>();

// Analytics middleware
app.use('*', async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	// biome-ignore lint/suspicious/noExplicitAny: Cloudflare cf object not typed on Request
	const cf = (c.req.raw as any).cf;
	const url = new URL(c.req.url);

	c.env.ANALYTICS?.writeDataPoint({
		indexes: ['simse-relay'],
		blobs: [
			c.req.method,
			url.pathname,
			String(c.res.status),
			'simse-relay',
			'',
			'',
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
		],
	});
});

// Health check (before secrets middleware)
app.get('/health', (c) => c.json({ ok: true }));

// Secrets middleware
const secretsMiddleware = createMiddleware<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>(async (c, next) => {
	// Try Secrets Store first, fall back to env var (for tests / dev)
	const authApiUrl =
		(await c.env.SECRETS?.get('AUTH_API_URL')) ||
		(c.env as unknown as Record<string, string>).AUTH_API_URL;

	if (!authApiUrl) {
		return c.json(
			{
				error: {
					code: 'MISCONFIGURED',
					message: 'Service misconfigured',
				},
			},
			500,
		);
	}

	c.set('secrets', { authApiUrl });
	await next();
});

app.use('*', secretsMiddleware);

// Routes
app.route('', ws);
app.route('', tunnels);

export default app;
