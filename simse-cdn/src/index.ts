import type { Env } from './types';

const BINARY_FILENAMES: Record<string, string> = {
	'linux/x64': 'simse-linux-x64.tar.gz',
	'darwin/arm64': 'simse-darwin-arm64.tar.gz',
	'darwin/x64': 'simse-darwin-x64.tar.gz',
	'windows/x64': 'simse-windows-x64.zip',
};

const IMMUTABLE_CACHE = 'public, max-age=31536000, immutable';

export default {
	async fetch(request: Request, env: Env): Promise<Response> {
		const start = Date.now();
		const response = await handleRequest(request, env);
		const latencyMs = Date.now() - start;

		// biome-ignore lint/suspicious/noExplicitAny: Cloudflare cf object not typed on Request
		const cf = (request as any).cf;
		const url = new URL(request.url);

		env.ANALYTICS_QUEUE.send({
			type: 'datapoint',
			service: 'simse-cdn',
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
			latencyMs,
			requestSize: Number(request.headers.get('Content-Length') ?? 0),
			responseSize: Number(response.headers.get('Content-Length') ?? 0),
			colo: Number(cf?.colo ?? 0),
		}).catch(() => {});

		return response;
	},
} satisfies ExportedHandler<Env>;

async function handleRequest(request: Request, env: Env): Promise<Response> {
	const url = new URL(request.url);
	const path = url.pathname;

	if (path === '/health') {
		return new Response(JSON.stringify({ ok: true }), {
			status: 200,
			headers: { 'Content-Type': 'application/json' },
		});
	}

	const mediaMatch = path.match(/^\/media\/(.+)$/);
	if (mediaMatch) {
		return serveR2(env.CDN_BUCKET, `media/${mediaMatch[1]}`, {
			immutable: true,
		});
	}

	const versionedMatch = path.match(/^\/download\/([^/]+)\/([^/]+)\/([^/]+)$/);
	if (versionedMatch && versionedMatch[1] !== 'latest') {
		const [, version, os, arch] = versionedMatch;
		const platform = `${os}/${arch}`;
		const filename = BINARY_FILENAMES[platform];
		if (!filename) {
			return new Response('unknown platform', { status: 404 });
		}
		const key = `releases/${os}/${arch}/${version}/${filename}`;
		return serveR2(env.CDN_BUCKET, key, {
			immutable: true,
			binary: true,
			filename,
		});
	}

	const latestMatch = path.match(/^\/download\/latest\/([^/]+)\/([^/]+)$/);
	if (latestMatch) {
		const [, os, arch] = latestMatch;
		const kvKey = `latest:${os}-${arch}`;
		const version = await env.VERSION_STORE.get(kvKey);
		if (!version) {
			return new Response('unknown platform', { status: 404 });
		}
		return new Response(null, {
			status: 301,
			headers: {
				Location: `/download/${version}/${os}/${arch}`,
				'Cache-Control': 'no-store',
			},
		});
	}

	return new Response('not found', { status: 404 });
}

async function serveR2(
	bucket: R2Bucket,
	key: string,
	opts: { immutable?: boolean; binary?: boolean; filename?: string },
): Promise<Response> {
	let object: R2ObjectBody | null;
	try {
		object = await bucket.get(key);
	} catch {
		return new Response('upstream error', { status: 502 });
	}

	if (!object) {
		return new Response('not found', { status: 404 });
	}

	const headers = new Headers();
	if (opts.immutable) {
		headers.set('Cache-Control', IMMUTABLE_CACHE);
	}
	if (opts.binary) {
		headers.set('Content-Type', 'application/octet-stream');
		if (opts.filename) {
			headers.set(
				'Content-Disposition',
				`attachment; filename="${opts.filename}"`,
			);
		}
	} else {
		const ct = object.httpMetadata?.contentType;
		if (ct) headers.set('Content-Type', ct);
	}

	return new Response(object.body, { headers });
}
