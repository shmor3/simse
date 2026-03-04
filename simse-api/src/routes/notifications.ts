import { Hono } from 'hono';
import type { AuthContext, Env } from '../types';

const notifications = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// GET /notifications
notifications.get('/', async (c) => {
	const auth = c.get('auth');
	const db = c.env.DB;

	const results = await db
		.prepare(
			'SELECT id, type, title, body, read, link, created_at FROM notifications WHERE user_id = ? ORDER BY created_at DESC LIMIT 100',
		)
		.bind(auth.userId)
		.all<{
			id: string;
			type: string;
			title: string;
			body: string;
			read: number;
			link: string | null;
			created_at: string;
		}>();

	return c.json({ data: results.results });
});

// PUT /notifications/:id/read
notifications.put('/:id/read', async (c) => {
	const auth = c.get('auth');
	const id = c.req.param('id');

	await c.env.DB
		.prepare('UPDATE notifications SET read = 1 WHERE id = ? AND user_id = ?')
		.bind(id, auth.userId)
		.run();

	return c.json({ data: { ok: true } });
});

// PUT /notifications/read-all
notifications.put('/read-all', async (c) => {
	const auth = c.get('auth');

	await c.env.DB
		.prepare('UPDATE notifications SET read = 1 WHERE user_id = ? AND read = 0')
		.bind(auth.userId)
		.run();

	return c.json({ data: { ok: true } });
});

export default notifications;
