import { render } from '@react-email/render';
import { Hono } from 'hono';
import { createElement } from 'react';
import { sendEmail } from './send';
import { templates } from './templates';

interface Env {
	RESEND_API_KEY: string;
	API_SECRET: string;
}

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));

app.post('/send', async (c) => {
	const authHeader = c.req.header('Authorization');
	if (authHeader !== `Bearer ${c.env.API_SECRET}`) {
		return c.json({ error: 'Unauthorized' }, 401);
	}

	const body = await c.req.json<{
		template: string;
		to: string;
		props?: Record<string, unknown>;
	}>();

	if (!body.template || !body.to) {
		return c.json({ error: 'Missing required fields: template, to' }, 400);
	}

	const entry = templates[body.template];
	if (!entry) {
		return c.json({ error: `Unknown template: ${body.template}` }, 400);
	}

	const props = body.props ?? {};
	const subject = entry.subject(props);
	const html = await render(createElement(entry.component, props));

	await sendEmail(c.env.RESEND_API_KEY, { to: body.to, subject, html });

	return c.json({ success: true });
});

export default app;
