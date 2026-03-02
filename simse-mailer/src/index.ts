import { Hono } from 'hono';
import { sendEmail } from './send';

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
		to: string;
		subject: string;
		html: string;
	}>();

	if (!body.to || !body.subject || !body.html) {
		return c.json({ error: 'Missing required fields: to, subject, html' }, 400);
	}

	await sendEmail(c.env.RESEND_API_KEY, {
		to: body.to,
		subject: body.subject,
		html: body.html,
	});

	return c.json({ success: true });
});

export default app;
