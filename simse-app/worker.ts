import { createRequestHandler } from 'react-router';
import { handleRelayRequest } from '~/relay/handler';

export { TunnelSession } from '~/relay/tunnel';

declare module 'react-router' {
	export interface AppLoadContext {
		cloudflare: {
			env: Env;
			ctx: ExecutionContext;
		};
	}
}

const requestHandler = createRequestHandler(
	() => import('virtual:react-router/server-build'),
	import.meta.env.MODE,
);

export default {
	async fetch(request, env, ctx) {
		const url = new URL(request.url);

		if (url.pathname === '/health') {
			return new Response(JSON.stringify({ ok: true }), {
				headers: { 'Content-Type': 'application/json' },
			});
		}

		// Relay routes (WebSocket tunnel + REST)
		const relayResponse = await handleRelayRequest(request, env);
		if (relayResponse) {
			return relayResponse;
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
					indexes: ['simse-app'],
					blobs: [
						request.method,
						url.pathname,
						String(response.status),
						'simse-app',
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
