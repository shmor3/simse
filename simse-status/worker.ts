import { createRequestHandler } from 'react-router';

declare module 'react-router' {
	export interface AppLoadContext {
		cloudflare: {
			env: Env;
			ctx: ExecutionContext;
		};
	}
}

interface Env {
	DB: D1Database;
	ANALYTICS: AnalyticsEngineDataset;
}

const requestHandler = createRequestHandler(
	() => import('virtual:react-router/server-build'),
	import.meta.env.MODE,
);

const SERVICES = [
	{ id: 'api', name: 'API Gateway', url: 'https://api.simse.dev/health' },
	{ id: 'auth', name: 'Auth', url: 'https://auth.simse.dev/health' },
	{ id: 'cdn', name: 'CDN', url: 'https://cdn.simse.dev/health' },
	{ id: 'cloud', name: 'Cloud App', url: 'https://app.simse.dev/health' },
	{ id: 'landing', name: 'Landing', url: 'https://simse.dev/health' },
];

async function checkService(
	service: { id: string; url: string },
	db: D1Database,
): Promise<void> {
	const start = Date.now();
	let status: 'up' | 'degraded' | 'down' = 'down';
	let statusCode: number | null = null;
	let error: string | null = null;

	try {
		const controller = new AbortController();
		const timeout = setTimeout(() => controller.abort(), 10_000);
		const res = await fetch(service.url, { signal: controller.signal });
		clearTimeout(timeout);
		statusCode = res.status;
		const elapsed = Date.now() - start;

		if (res.ok) {
			status = elapsed > 5000 ? 'degraded' : 'up';
		}
	} catch (err) {
		error = err instanceof Error ? err.message : 'Unknown error';
	}

	const responseTimeMs = Date.now() - start;

	await db
		.prepare(
			'INSERT INTO checks (service_id, status, response_time_ms, status_code, error) VALUES (?, ?, ?, ?, ?)',
		)
		.bind(service.id, status, responseTimeMs, statusCode, error)
		.run();
}

export default {
	async fetch(request: Request, env: Env, ctx: ExecutionContext) {
		const url = new URL(request.url);
		if (url.pathname === '/health') {
			return new Response(JSON.stringify({ ok: true }), {
				headers: { 'Content-Type': 'application/json' },
			});
		}

		const start = Date.now();
		const response = await requestHandler(request, {
			cloudflare: { env, ctx },
		});
		const latencyMs = Date.now() - start;

		// biome-ignore lint/suspicious/noExplicitAny: Cloudflare cf object not typed on Request
		const cf = (request as any).cf;

		ctx.waitUntil(
			Promise.resolve(
				env.ANALYTICS.writeDataPoint({
					indexes: ['simse-status'],
					blobs: [
						request.method,
						url.pathname,
						String(response.status),
						'simse-status',
						'',
						'',
						cf?.country ?? '',
						cf?.city ?? '',
						cf?.continent ?? '',
						(request.headers.get('User-Agent') ?? '').slice(0, 256),
						request.headers.get('Referer') ?? '',
						response.headers.get('Content-Type') ?? '',
						request.headers.get('Cf-Ray') ?? '',
					],
					doubles: [
						latencyMs,
						response.status,
						Number(request.headers.get('Content-Length') ?? 0),
						Number(response.headers.get('Content-Length') ?? 0),
						Number(cf?.colo ?? 0),
					],
				}),
			),
		);

		return response;
	},

	async scheduled(
		_event: ScheduledController,
		env: Env,
		ctx: ExecutionContext,
	) {
		const checks = SERVICES.map((s) => checkService(s, env.DB));
		ctx.waitUntil(
			Promise.allSettled(checks).then(() =>
				env.DB.prepare(
					"DELETE FROM checks WHERE checked_at < datetime('now', '-90 days')",
				).run(),
			),
		);
	},
} satisfies ExportedHandler<Env>;
