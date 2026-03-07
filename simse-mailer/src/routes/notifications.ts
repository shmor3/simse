import { Hono } from 'hono';
import type { SecretsStoreNamespace } from '../types';

interface Env {
	DB: D1Database;
	SECRETS: SecretsStoreNamespace;
}

interface MailerSecrets {
	mailerApiSecret: string;
}

const notifications = new Hono<{
	Bindings: Env;
	Variables: { secrets: MailerSecrets };
}>();

// GET /notifications/:userId — list (last 100)
notifications.get('/:userId', async (c) => {
	const userId = c.req.header('X-User-Id');
	const authHeader = c.req.header('Authorization');
	const paramUserId = c.req.param('userId');

	// Must be accessed by the user themselves (via gateway X-User-Id) or internal API
	if (
		userId !== paramUserId &&
		authHeader !== `Bearer ${c.var.secrets.mailerApiSecret}`
	) {
		return c.json(
			{ error: { code: 'FORBIDDEN', message: 'Access denied' } },
			403,
		);
	}

	const rows = await c.env.DB.prepare(
		'SELECT id, type, title, body, read, link, created_at FROM notifications WHERE user_id = ? ORDER BY created_at DESC LIMIT 100',
	)
		.bind(paramUserId)
		.all<{
			id: string;
			type: string;
			title: string;
			body: string;
			read: number;
			link: string | null;
			created_at: string;
		}>();

	return c.json({ data: rows.results });
});

// POST /notifications — create (internal API only)
notifications.post('/', async (c) => {
	const authHeader = c.req.header('Authorization');
	if (authHeader !== `Bearer ${c.var.secrets.mailerApiSecret}`) {
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Unauthorized' } },
			401,
		);
	}

	const body = await c.req.json<{
		userId: string;
		type: string;
		title: string;
		body: string;
		link?: string;
	}>();

	if (!body.userId || !body.type || !body.title || !body.body) {
		return c.json(
			{
				error: { code: 'VALIDATION_ERROR', message: 'Missing required fields' },
			},
			400,
		);
	}

	const id = crypto.randomUUID();
	await c.env.DB.prepare(
		'INSERT INTO notifications (id, user_id, type, title, body, link) VALUES (?, ?, ?, ?, ?, ?)',
	)
		.bind(id, body.userId, body.type, body.title, body.body, body.link ?? null)
		.run();

	return c.json({ data: { id } }, 201);
});

// PUT /notifications/:id/read — mark read
notifications.put('/:id/read', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId)
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } },
			401,
		);

	const id = c.req.param('id');
	await c.env.DB.prepare(
		'UPDATE notifications SET read = 1 WHERE id = ? AND user_id = ?',
	)
		.bind(id, userId)
		.run();

	return c.json({ data: { ok: true } });
});

// PUT /notifications/:userId/read-all — mark all read
notifications.put('/:userId/read-all', async (c) => {
	const userId = c.req.header('X-User-Id');
	const paramUserId = c.req.param('userId');

	if (!userId || userId !== paramUserId) {
		return c.json(
			{ error: { code: 'FORBIDDEN', message: 'Access denied' } },
			403,
		);
	}

	await c.env.DB.prepare(
		'UPDATE notifications SET read = 1 WHERE user_id = ? AND read = 0',
	)
		.bind(userId)
		.run();

	return c.json({ data: { ok: true } });
});

export default notifications;
