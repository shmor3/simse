import { Hono } from 'hono';
import { createApiKey } from '../lib/api-key';
import { sendAuditEvent } from '../lib/audit';
import { createApiKeySchema } from '../schemas';
import type { Env } from '../types';

const MAX_API_KEYS_PER_USER = 25;

const apiKeys = new Hono<{ Bindings: Env }>();

// POST /api-keys
apiKeys.post('/', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId)
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } },
			401,
		);

	let body: unknown;
	try {
		body = await c.req.json();
	} catch {
		return c.json(
			{ error: { code: 'INVALID_BODY', message: 'Invalid JSON body' } },
			400,
		);
	}
	const parsed = createApiKeySchema.safeParse(body);
	if (!parsed.success) {
		return c.json(
			{
				error: {
					code: 'VALIDATION_ERROR',
					message: parsed.error.issues[0].message,
				},
			},
			400,
		);
	}

	// Enforce max API keys per user
	const count = await c.env.DB.prepare(
		'SELECT COUNT(*) as cnt FROM api_keys WHERE user_id = ?',
	)
		.bind(userId)
		.first<{ cnt: number }>();

	if (count && count.cnt >= MAX_API_KEYS_PER_USER) {
		return c.json(
			{
				error: {
					code: 'LIMIT_EXCEEDED',
					message: `Maximum of ${MAX_API_KEYS_PER_USER} API keys allowed`,
				},
			},
			400,
		);
	}

	const result = await createApiKey(c.env.DB, userId, parsed.data.name);

	sendAuditEvent(c.env.ANALYTICS_QUEUE, 'api_key.created', userId, {
		keyId: result.id,
		name: parsed.data.name,
	});

	return c.json(
		{
			data: {
				id: result.id,
				key: result.key,
				prefix: result.prefix,
				name: parsed.data.name,
			},
		},
		201,
	);
});

// GET /api-keys
apiKeys.get('/', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId)
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } },
			401,
		);

	const keys = await c.env.DB.prepare(
		'SELECT id, name, key_prefix, last_used_at, created_at FROM api_keys WHERE user_id = ? ORDER BY created_at DESC',
	)
		.bind(userId)
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
	const userId = c.req.header('X-User-Id');
	if (!userId)
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } },
			401,
		);

	const id = c.req.param('id');
	await c.env.DB.prepare('DELETE FROM api_keys WHERE id = ? AND user_id = ?')
		.bind(id, userId)
		.run();

	sendAuditEvent(c.env.ANALYTICS_QUEUE, 'api_key.deleted', userId, {
		keyId: id,
	});

	return c.json({ data: { ok: true } });
});

export default apiKeys;
