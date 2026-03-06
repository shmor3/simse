import { Hono } from 'hono';
import { renderTemplate } from './render';
import notificationsRoute from './routes/notifications';
import { sendEmail } from './send';

type CommsMessage =
	| {
			type: 'email';
			template: string;
			to: string;
			props?: Record<string, string>;
	  }
	| {
			type: 'notification';
			userId: string;
			kind: string;
			title: string;
			body: string;
			link?: string;
	  };

export interface Env {
	DB: D1Database;
	SECRETS: SecretsStoreNamespace;
	ANALYTICS: AnalyticsEngineDataset;
}

interface MailerSecrets {
	resendApiKey: string;
	mailerApiSecret: string;
}

const app = new Hono<{
	Bindings: Env;
	Variables: { secrets: MailerSecrets };
}>();

// Analytics middleware
app.use('*', async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	// biome-ignore lint/suspicious/noExplicitAny: Cloudflare cf object not typed on Request
	const cf = (c.req.raw as any).cf;

	c.env.ANALYTICS.writeDataPoint({
		indexes: ['simse-mailer'],
		blobs: [
			c.req.method,
			c.req.path,
			String(c.res.status),
			'simse-mailer',
			c.req.header('X-User-Id') ?? '',
			'',
			cf?.country ?? '',
			cf?.city ?? '',
			cf?.continent ?? '',
			(c.req.header('User-Agent') ?? '').slice(0, 256),
			c.req.header('Referer') ?? '',
			c.res.headers.get('Content-Type') ?? '',
			c.req.header('Cf-Ray') ?? '',
		],
		doubles: [
			latencyMs,
			c.res.status,
			Number(c.req.header('Content-Length') ?? 0),
			Number(c.res.headers.get('Content-Length') ?? 0),
			Number(cf?.colo ?? 0),
		],
	});
});

// Secrets middleware
app.use('*', async (c, next) => {
	const [resendApiKey, mailerApiSecret] = await Promise.all([
		c.env.SECRETS.get('RESEND_API_KEY'),
		c.env.SECRETS.get('MAILER_API_SECRET'),
	]);
	if (!resendApiKey || !mailerApiSecret) {
		return c.json({ error: 'Service misconfigured' }, 500);
	}
	c.set('secrets', { resendApiKey, mailerApiSecret });
	await next();
});

app.get('/health', (c) => c.json({ ok: true }));

app.post('/send', async (c) => {
	const authHeader = c.req.header('Authorization');
	if (authHeader !== `Bearer ${c.var.secrets.mailerApiSecret}`) {
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

	const { subject, html } = await renderTemplate(
		body.template,
		body.props ?? {},
	);

	await sendEmail(c.var.secrets.resendApiKey, { to: body.to, subject, html });
	return c.json({ success: true });
});

app.route('/notifications', notificationsRoute);

export default {
	async fetch(request: Request, env: Env): Promise<Response> {
		return app.fetch(request, env);
	},
	async queue(batch: MessageBatch<CommsMessage>, env: Env): Promise<void> {
		// Fetch secrets once for the whole batch
		const [resendApiKey] = await Promise.all([
			env.SECRETS.get('RESEND_API_KEY'),
		]);
		if (!resendApiKey) {
			console.error(
				'RESEND_API_KEY not configured — acking all messages to avoid poison pill',
			);
			for (const message of batch.messages) message.ack();
			return;
		}

		for (const message of batch.messages) {
			const msg = message.body;
			try {
				if (msg.type === 'email') {
					const { subject, html } = await renderTemplate(
						msg.template,
						msg.props ?? {},
					);
					await sendEmail(resendApiKey, { to: msg.to, subject, html });
				} else if (msg.type === 'notification') {
					const id = crypto.randomUUID();
					await env.DB.prepare(
						'INSERT INTO notifications (id, user_id, type, title, body, link) VALUES (?, ?, ?, ?, ?, ?)',
					)
						.bind(
							id,
							msg.userId,
							msg.kind ?? 'info',
							msg.title,
							msg.body,
							msg.link ?? null,
						)
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
