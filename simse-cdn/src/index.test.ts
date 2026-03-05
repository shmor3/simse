import { SELF } from 'cloudflare:test';
import { describe, expect, it } from 'vitest';

describe('GET /health', () => {
	it('returns 200', async () => {
		const res = await SELF.fetch('https://cdn.simse.dev/health');
		expect(res.status).toBe(200);
		await res.text();
	});
});

describe('GET /media/{file}', () => {
	it('streams file from R2 with immutable cache header', async () => {
		const res = await SELF.fetch('https://cdn.simse.dev/media/hero.png');
		expect(res.status).toBe(200);
		expect(res.headers.get('Cache-Control')).toBe(
			'public, max-age=31536000, immutable',
		);
		expect(res.headers.get('Content-Type')).toBe('image/png');
		await res.arrayBuffer();
	});

	it('returns 404 for missing file', async () => {
		const res = await SELF.fetch('https://cdn.simse.dev/media/missing.png');
		expect(res.status).toBe(404);
		await res.text();
	});
});

describe('GET /download/{version}/{os}/{arch}', () => {
	it('streams binary from R2 with correct headers', async () => {
		const res = await SELF.fetch(
			'https://cdn.simse.dev/download/1.2.3/linux/x64',
		);
		expect(res.status).toBe(200);
		expect(res.headers.get('Cache-Control')).toBe(
			'public, max-age=31536000, immutable',
		);
		expect(res.headers.get('Content-Type')).toBe('application/octet-stream');
		expect(res.headers.get('Content-Disposition')).toBe(
			'attachment; filename="simse-linux-x64.tar.gz"',
		);
		await res.arrayBuffer();
	});

	it('returns 404 for missing version', async () => {
		const res = await SELF.fetch(
			'https://cdn.simse.dev/download/9.9.9/linux/x64',
		);
		expect(res.status).toBe(404);
		await res.text();
	});
});

describe('GET /download/latest/{os}/{arch}', () => {
	it('redirects to versioned URL via KV lookup', async () => {
		const res = await SELF.fetch(
			'https://cdn.simse.dev/download/latest/linux/x64',
			{ redirect: 'manual' },
		);
		expect(res.status).toBe(301);
		expect(res.headers.get('Location')).toBe('/download/1.2.3/linux/x64');
		expect(res.headers.get('Cache-Control')).toBe('no-store');
		await res.text();
	});

	it('returns 404 for unknown platform', async () => {
		const res = await SELF.fetch(
			'https://cdn.simse.dev/download/latest/freebsd/x64',
		);
		expect(res.status).toBe(404);
		await res.text();
	});
});

describe('unknown routes', () => {
	it('returns 404', async () => {
		const res = await SELF.fetch('https://cdn.simse.dev/unknown');
		expect(res.status).toBe(404);
		await res.text();
	});
});
