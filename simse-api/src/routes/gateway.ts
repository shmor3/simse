import { Hono } from 'hono';
import type { AuthContext, Env } from '../types';

const gateway = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// Proxy to simse-payments: /payments/* → simse-payments/*
gateway.all('/payments/*', async (c) => {
	const path = c.req.path.replace('/payments', '');
	const url = `${c.env.PAYMENTS_API_URL}${path}`;

	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.env.PAYMENTS_API_SECRET}`);
	headers.set('Content-Type', 'application/json');

	const init: RequestInit = {
		method: c.req.method,
		headers,
	};

	if (!['GET', 'HEAD'].includes(c.req.method)) {
		init.body = await c.req.text();
	}

	const res = await fetch(url, init);
	const body = await res.text();

	return new Response(body, {
		status: res.status,
		headers: { 'Content-Type': 'application/json' },
	});
});

// Proxy to simse-mailer: /emails/send → simse-mailer/send
gateway.post('/emails/send', async (c) => {
	const body = await c.req.text();

	const res = await fetch(`${c.env.MAILER_API_URL}/send`, {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${c.env.MAILER_API_SECRET}`,
			'Content-Type': 'application/json',
		},
		body,
	});

	const responseBody = await res.text();

	return new Response(responseBody, {
		status: res.status,
		headers: { 'Content-Type': 'application/json' },
	});
});

export default gateway;
