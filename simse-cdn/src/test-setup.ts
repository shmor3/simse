import { env } from 'cloudflare:test';
import { beforeAll } from 'vitest';

beforeAll(async () => {
	// Seed R2: media file
	await env.CDN_BUCKET.put('media/hero.png', new Uint8Array([1, 2, 3]), {
		httpMetadata: { contentType: 'image/png' },
	});

	// Seed R2: versioned binary
	await env.CDN_BUCKET.put(
		'releases/linux/x64/1.2.3/simse-linux-x64.tar.gz',
		new Uint8Array([4, 5, 6]),
	);

	// Seed KV: version manifest
	await env.VERSION_STORE.put('latest:linux-x64', '1.2.3');
	await env.VERSION_STORE.put('latest:darwin-arm64', '1.2.3');
	await env.VERSION_STORE.put('latest:windows-x64', '1.2.3');
});
