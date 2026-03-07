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
	ANALYTICS_QUEUE: Queue;
}

interface MailerSecrets {
	resendApiKey: string;
	mailerApiSecret: string;
}

const app = new Hono<{
	Bindings: Env;
	Variables: { secrets: MailerSecrets };
}>();

// Health check — before any middleware
app.get('/health', (c) => c.json({ ok: true }));

// Analytics middleware
app.use('*', async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	// biome-ignore lint/suspicious/noExplicitAny: Cloudflare cf object not typed on Request
	const cf = (c.req.raw as any).cf;

	c.env.ANALYTICS_QUEUE.send({
		type: 'datapoint',
		service: 'simse-mailer',
		method: c.req.method,
		path: c.req.path,
		status: c.res.status,
		userId: c.req.header('X-User-Id') ?? '',
		country: cf?.country ?? '',
		city: cf?.city ?? '',
		continent: cf?.continent ?? '',
		userAgent: (c.req.header('User-Agent') ?? '').slice(0, 256),
		referer: (c.req.header('Referer') ?? '').split('?')[0],
		contentType: c.res.headers.get('Content-Type') ?? '',
		cfRay: c.req.header('Cf-Ray') ?? '',
		latencyMs,
		requestSize: Number(c.req.header('Content-Length') ?? 0),
		responseSize: Number(c.res.headers.get('Content-Length') ?? 0),
		colo: Number(cf?.colo ?? 0),
	}).catch(() => {});
});

// Secrets middleware
app.use('*', async (c, next) => {
	const secrets = c.env.SECRETS;
	if (!secrets) {
		return c.json({ error: 'Service misconfigured' }, 500);
	}
	const [resendApiKey, mailerApiSecret] = await Promise.all([
		secrets.get('RESEND_API_KEY'),
		secrets.get('MAILER_API_SECRET'),
	]);
	if (!resendApiKey || !mailerApiSecret) {
		return c.json({ error: 'Service misconfigured' }, 500);
	}
	c.set('secrets', { resendApiKey, mailerApiSecret });
	await next();
});

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
		const batchStart = Date.now();
		const resendApiKey = await env.SECRETS.get('RESEND_API_KEY');
		if (!resendApiKey) {
			console.error(
				'RESEND_API_KEY not configured — acking all messages to avoid poison pill',
			);
			for (const message of batch.messages) message.ack();
			return;
		}

		for (const message of batch.messages) {
			const msg = message.body;
			const msgStart = Date.now();
			try {
				if (msg.type === 'email') {
					const { subject, html } = await renderTemplate(
						msg.template,
						msg.props ?? {},
					);
					await sendEmail(resendApiKey, { to: msg.to, subject, html });

					env.ANALYTICS_QUEUE.send({
						type: 'datapoint',
						service: 'simse-mailer',
						method: 'queue',
						path: msg.template,
						status: 200,
						latencyMs: Date.now() - msgStart,
						requestSize: 0,
						responseSize: 0,
					}).catch(() => {});
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

					env.ANALYTICS_QUEUE.send({
						type: 'datapoint',
						service: 'simse-mailer',
						method: 'queue',
						path: msg.kind ?? 'info',
						status: 200,
						latencyMs: Date.now() - msgStart,
						requestSize: 0,
						responseSize: 0,
					}).catch(() => {});
				}
				message.ack();
			} catch (e) {
				const errorMsg = e instanceof Error ? e.message : 'Unknown error';
				const label = msg.type === 'email' ? msg.template : msg.type;

				// Permanent failures: don't retry, just ack and log
				if (errorMsg.includes('Unknown email template')) {
					console.error(
						`Permanent failure for ${msg.type} (${label}): ${errorMsg}`,
					);
					message.ack();
				} else {
					console.error(
						`Transient failure for ${msg.type} (${label}): ${errorMsg}`,
					);
					message.retry();
				}

				env.ANALYTICS_QUEUE.send({
					type: 'datapoint',
					service: 'simse-mailer',
					method: 'queue',
					path: label,
					status: 500,
					latencyMs: Date.now() - msgStart,
					requestSize: 0,
					responseSize: 0,
				}).catch(() => {});
			}
		}

		env.ANALYTICS_QUEUE.send({
			type: 'datapoint',
			service: 'simse-mailer',
			method: 'queue',
			path: 'batch',
			status: 200,
			latencyMs: Date.now() - batchStart,
			requestSize: batch.messages.length,
			responseSize: 0,
		}).catch(() => {});
	},
};
