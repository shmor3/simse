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
