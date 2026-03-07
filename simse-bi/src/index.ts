import { Hono } from 'hono';
import type {
	AnalyticsMessage,
	AuditMessage,
	DatapointMessage,
	Env,
} from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));

app.get('/audit/:userId', async (c) => {
	const userId = c.req.param('userId');
	const limit = Math.min(Number(c.req.query('limit') ?? 50), 200);
	const offset = Number(c.req.query('offset') ?? 0);

	const rows = await c.env.DB.prepare(
		'SELECT id, action, user_id, metadata, created_at FROM audit_events WHERE user_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?',
	)
		.bind(userId, limit, offset)
		.all<{
			id: string;
			action: string;
			user_id: string;
			metadata: string | null;
			created_at: string;
		}>();

	return c.json({
		data: rows.results.map((r) => ({
			id: r.id,
			action: r.action,
			userId: r.user_id,
			metadata: r.metadata ? JSON.parse(r.metadata) : null,
			createdAt: r.created_at,
		})),
	});
});

function writeDatapoint(env: Env, msg: DatapointMessage): void {
	env.ANALYTICS.writeDataPoint({
		indexes: [msg.service],
		blobs: [
			msg.method,
			msg.path,
			String(msg.status),
			msg.service,
			msg.userId ?? '',
			msg.teamId ?? '',
			msg.country ?? '',
			msg.city ?? '',
			msg.continent ?? '',
			(msg.userAgent ?? '').slice(0, 256),
			(msg.referer ?? '').split('?')[0],
			msg.contentType ?? '',
			msg.cfRay ?? '',
		],
		doubles: [
			msg.latencyMs,
			msg.status,
			msg.requestSize,
			msg.responseSize,
			msg.colo ?? 0,
		],
	});
}

async function writeAudit(env: Env, msg: AuditMessage): Promise<void> {
	const { type, action, userId, timestamp, ...rest } = msg;
	const id = crypto.randomUUID();
	const metadata = Object.keys(rest).length > 0 ? JSON.stringify(rest) : null;

	await env.DB.prepare(
		'INSERT INTO audit_events (id, action, user_id, metadata, created_at) VALUES (?, ?, ?, ?, ?)',
	)
		.bind(id, action, userId, metadata, timestamp)
		.run();

	// Also write to Analytics Engine for dashboards
	env.ANALYTICS.writeDataPoint({
		indexes: ['audit'],
		blobs: [
			action,
			userId,
			timestamp,
			metadata ?? '',
			'',
			'',
			'',
			'',
			'',
			'',
			'',
			'',
			'',
		],
		doubles: [0, 0, 0, 0, 0],
	});
}

export default {
	fetch: app.fetch,

	async queue(batch: MessageBatch<AnalyticsMessage>, env: Env): Promise<void> {
		for (const message of batch.messages) {
			const msg = message.body;
			try {
				if (msg.type === 'datapoint') {
					writeDatapoint(env, msg);
				} else if (msg.type === 'audit') {
					await writeAudit(env, msg);
				}
				message.ack();
			} catch (e) {
				if (msg.type === 'audit') {
					// Audit failures should retry — data must not be lost
					console.error('Audit write failed', e);
					message.retry();
				} else {
					// Analytics datapoint failures are non-critical
					console.error('Datapoint write failed', e);
					message.ack();
				}
			}
		}
	},
};
