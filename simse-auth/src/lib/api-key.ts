import { generateId } from './db';

export async function createApiKey(
	db: D1Database,
	userId: string,
	name: string,
): Promise<{ id: string; key: string; prefix: string }> {
	const id = generateId();
	const rawKey = `sk_${generateId().replace(/-/g, '')}`;
	const prefix = rawKey.slice(0, 7);

	// Hash the key for storage
	const encoder = new TextEncoder();
	const data = encoder.encode(rawKey);
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = new Uint8Array(hashBuffer);
	const keyHash = btoa(String.fromCharCode(...hashArray));

	await db
		.prepare(
			'INSERT INTO api_keys (id, user_id, name, key_hash, key_prefix) VALUES (?, ?, ?, ?, ?)',
		)
		.bind(id, userId, name, keyHash, prefix)
		.run();

	return { id, key: rawKey, prefix };
}

export async function validateApiKey(
	db: D1Database,
	rawKey: string,
): Promise<string | null> {
	const encoder = new TextEncoder();
	const data = encoder.encode(rawKey);
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = new Uint8Array(hashBuffer);
	const keyHash = btoa(String.fromCharCode(...hashArray));

	const row = await db
		.prepare('SELECT user_id FROM api_keys WHERE key_hash = ?')
		.bind(keyHash)
		.first<{ user_id: string }>();

	if (!row) return null;

	// Update last_used_at
	await db
		.prepare(
			"UPDATE api_keys SET last_used_at = datetime('now') WHERE key_hash = ?",
		)
		.bind(keyHash)
		.run();

	return row.user_id;
}
