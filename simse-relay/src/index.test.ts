import { SELF } from 'cloudflare:test';
import { describe, expect, it } from 'vitest';

describe('GET /health', () => {
	it('returns 200', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/health');
		expect(res.status).toBe(200);
		const body = await res.json();
		expect(body).toEqual({ ok: true });
	});
});

describe('GET /ws/tunnel', () => {
	it('returns 401 without token', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/ws/tunnel');
		expect(res.status).toBe(401);
		const body = (await res.json()) as { error: { code: string } };
		expect(body.error.code).toBe('MISSING_TOKEN');
	});
});

describe('GET /ws/client', () => {
	it('returns 401 without token', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/ws/client');
		expect(res.status).toBe(401);
		const body = (await res.json()) as { error: { code: string } };
		expect(body.error.code).toBe('MISSING_TOKEN');
	});
});

describe('GET /tunnels', () => {
	it('returns 401 without auth header', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/tunnels');
		expect(res.status).toBe(401);
		const body = (await res.json()) as { error: { code: string } };
		expect(body.error.code).toBe('UNAUTHORIZED');
	});
});

describe('unknown routes', () => {
	it('returns 404', async () => {
		const res = await SELF.fetch('https://relay.simse.dev/unknown');
		expect(res.status).toBe(404);
	});
});
