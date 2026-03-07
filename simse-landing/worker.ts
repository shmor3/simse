import { createRequestHandler } from 'react-router';

interface Env {
	DB: D1Database;
	COMMS_QUEUE: Queue;
	ANALYTICS: AnalyticsEngineDataset;
}

const requestHandler = createRequestHandler(
	() => import('virtual:react-router/server-build'),
	import.meta.env.MODE,
);

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
					indexes: ['simse-landing'],
					blobs: [
						request.method,
						url.pathname,
						String(response.status),
						'simse-landing',
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
} satisfies ExportedHandler<Env>;
