import { Hono } from 'hono';
import { createApiKey } from '../lib/api-key';
import { createApiKeySchema } from '../schemas';
import type { AuthContext, Env } from '../types';

const apiKeys = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// POST /api-keys
apiKeys.post('/', async (c) => {
	const auth = c.get('auth');
	const body = await c.req.json();
	const parsed = createApiKeySchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const result = await createApiKey(c.env.DB, auth.userId, parsed.data.name);

	// Return the raw key only once — it's hashed in storage
	return c.json({
		data: {
			id: result.id,
			key: result.key,
			prefix: result.prefix,
			name: parsed.data.name,
		},
	}, 201);
});

// GET /api-keys
apiKeys.get('/', async (c) => {
	const auth = c.get('auth');

	const keys = await c.env.DB
		.prepare(
			'SELECT id, name, key_prefix, last_used_at, created_at FROM api_keys WHERE user_id = ? ORDER BY created_at DESC',
		)
		.bind(auth.userId)
		.all<{
			id: string;
			name: string;
			key_prefix: string;
			last_used_at: string | null;
			created_at: string;
		}>();

	return c.json({ data: keys.results });
});

// DELETE /api-keys/:id
apiKeys.delete('/:id', async (c) => {
	const auth = c.get('auth');
	const id = c.req.param('id');

	await c.env.DB
		.prepare('DELETE FROM api_keys WHERE id = ? AND user_id = ?')
		.bind(id, auth.userId)
		.run();

	return c.json({ data: { ok: true } });
});

export default apiKeys;
