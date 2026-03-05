import { Hono } from 'hono';
import notificationsRoute from './routes/notifications';
import { renderTemplate } from './render';
import { sendEmail } from './send';

type CommsMessage =
	| { type: 'email'; template: string; to: string; props?: Record<string, string> }
	| { type: 'notification'; userId: string; kind: string; title: string; body: string; link?: string };

interface Env {
	RESEND_API_KEY: string;
	API_SECRET: string;
	DB: D1Database;
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
		props?: Record<string, string>;
	}>();

	if (!body.to || !body.template) {
		return c.json({ error: 'Missing required fields: to, template' }, 400);
	}

	const { subject, html } = await renderTemplate(body.template, body.props ?? {});

	await sendEmail(c.env.RESEND_API_KEY, { to: body.to, subject, html });
	return c.json({ success: true });
});

app.route('/notifications', notificationsRoute);

export default {
	async fetch(request: Request, env: Env): Promise<Response> {
		return app.fetch(request, env);
	},
	async queue(batch: MessageBatch<CommsMessage>, env: Env): Promise<void> {
		for (const message of batch.messages) {
			const msg = message.body;
			try {
				if (msg.type === 'email') {
					const { subject, html } = await renderTemplate(msg.template, msg.props ?? {});
					await sendEmail(env.RESEND_API_KEY, { to: msg.to, subject, html });
				} else if (msg.type === 'notification') {
					const id = crypto.randomUUID();
					await env.DB.prepare(
						'INSERT INTO notifications (id, user_id, type, title, body, link) VALUES (?, ?, ?, ?, ?, ?)',
					)
						.bind(id, msg.userId, msg.kind ?? 'info', msg.title, msg.body, msg.link ?? null)
						.run();
				}
				message.ack();
			} catch (e) {
				console.error('Queue processing error:', e);
				message.retry();
			}
		}
	},
};
