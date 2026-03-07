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
	ASSETS: Fetcher;
	DB: D1Database;
	COMMS_QUEUE: Queue;
	ANALYTICS_QUEUE: Queue;
}

// --- Thin API layer (BFF) ---

const DISPOSABLE_DOMAINS = new Set([
	'mailinator.com', 'guerrillamail.com', 'guerrillamail.de', 'grr.la',
	'tempmail.com', 'temp-mail.org', 'throwaway.email', 'yopmail.com',
	'yopmail.fr', 'sharklasers.com', 'dispostable.com', 'trashmail.com',
	'trashmail.me', 'trashmail.net', 'mailnesia.com', 'maildrop.cc',
	'discard.email', 'mailcatch.com', 'fakeinbox.com', 'mailnull.com',
	'10minutemail.com', 'burnermail.io', 'mailsac.com',
]);

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

async function handleWaitlist(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
	if (request.method !== 'POST') {
		return Response.json({ error: 'Method not allowed' }, { status: 405 });
	}

	let body: { email?: string };
	try {
		body = await request.json();
	} catch {
		return Response.json({ error: 'Invalid JSON' }, { status: 400 });
	}

	const email = (body.email ?? '').trim().toLowerCase();
	if (!email || !EMAIL_RE.test(email)) {
		return Response.json({ error: 'Please enter a valid email address' }, { status: 400 });
	}

	const domain = email.split('@')[1];
	if (DISPOSABLE_DOMAINS.has(domain)) {
		return Response.json({ error: 'Disposable email addresses are not allowed' }, { status: 422 });
	}

	try {
		const dns = await fetch(
			`https://cloudflare-dns.com/dns-query?name=${encodeURIComponent(domain)}&type=MX`,
			{ headers: { Accept: 'application/dns-json' } },
		);
		if (dns.ok) {
			const data: { Status: number; Answer?: { type: number }[] } = await dns.json();
			if (data.Status === 0 && !data.Answer?.some((a) => a.type === 15)) {
				return Response.json({ error: 'This email domain does not appear to accept mail' }, { status: 422 });
			}
		}
	} catch { /* fail open */ }

	let shouldEmail = false;
	try {
		const result = await env.DB.prepare(
			`INSERT INTO waitlist (email, subscribed, updated_at) VALUES (?, 1, datetime('now'))
			ON CONFLICT (email) DO UPDATE SET subscribed = 1, updated_at = datetime('now')
			WHERE subscribed = 0 AND updated_at < datetime('now', '-1 day')`,
		).bind(email).run();
		shouldEmail = (result.meta?.changes ?? 0) > 0;
	} catch (err) {
		console.error('D1 insert failed', err);
		return Response.json({ error: 'Database error' }, { status: 500 });
	}

	if (shouldEmail) {
		const origin = new URL(request.url).origin;
		const unsubscribeUrl = `${origin}/unsubscribe?email=${encodeURIComponent(email)}`;
		ctx.waitUntil(
			env.COMMS_QUEUE.send({
				type: 'email',
				template: 'waitlist-welcome',
				to: email,
				props: { unsubscribeUrl },
			}).catch((err) => console.error('Queue send failed:', err)),
		);
	}

	return Response.json({ success: true });
}

// --- SSR ---

const requestHandler = createRequestHandler(
	() => import('virtual:react-router/server-build'),
	import.meta.env.MODE,
);

export default {
	async fetch(request: Request, env: Env, ctx: ExecutionContext) {
		const url = new URL(request.url);
		const start = Date.now();

		// Health check
		if (url.pathname === '/health') {
			return Response.json({ ok: true });
		}

		// Static assets
		if (url.pathname.startsWith('/assets/') || url.pathname === '/site.webmanifest' || url.pathname === '/favicon.ico') {
			return env.ASSETS.fetch(request);
		}

		let response: Response;

		// BFF API
		if (url.pathname === '/api/waitlist') {
			response = await handleWaitlist(request, env, ctx);
		} else {
			// React Router SSR
			try {
				response = await requestHandler(request, {
					cloudflare: { env, ctx },
				});
			} catch (e) {
				console.error('Worker error:', e);
				response = new Response('Internal Server Error', { status: 500 });
			}
		}

		// biome-ignore lint/suspicious/noExplicitAny: Cloudflare cf object not typed on Request
		const cf = (request as any).cf;
		ctx.waitUntil(
			env.ANALYTICS_QUEUE.send({
				type: 'datapoint',
				service: 'simse-landing',
				method: request.method,
				path: url.pathname,
				status: response.status,
				country: cf?.country ?? '',
				city: cf?.city ?? '',
				continent: cf?.continent ?? '',
				userAgent: (request.headers.get('User-Agent') ?? '').slice(0, 256),
				referer: (request.headers.get('Referer') ?? '').split('?')[0],
				contentType: response.headers.get('Content-Type') ?? '',
				cfRay: request.headers.get('Cf-Ray') ?? '',
				latencyMs: Date.now() - start,
				requestSize: Number(request.headers.get('Content-Length') ?? 0),
				responseSize: Number(response.headers.get('Content-Length') ?? 0),
				colo: Number(cf?.colo ?? 0),
			}).catch(() => {}),
		);

		return response;
	},
} satisfies ExportedHandler<Env>;
