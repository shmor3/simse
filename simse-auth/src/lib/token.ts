import { generateId } from './db';

export function generateCode(): string {
	const array = new Uint32Array(1);
	crypto.getRandomValues(array);
	return String(array[0] % 1_000_000).padStart(6, '0');
}

export async function createToken(
	db: D1Database,
	userId: string,
	type: string,
	expiresInMinutes: number,
): Promise<{ id: string; code: string }> {
	const id = generateId();
	const code = generateCode();
	const expiresAt = new Date(
		Date.now() + expiresInMinutes * 60 * 1000,
	).toISOString();

	await db
		.prepare(
			'INSERT INTO tokens (id, user_id, type, code, expires_at) VALUES (?, ?, ?, ?, ?)',
		)
		.bind(id, userId, type, code, expiresAt)
		.run();

	return { id, code };
}

// Atomic validate + mark used — prevents race where two concurrent requests
// both validate the same token before either marks it used
export async function consumeToken(
	db: D1Database,
	code: string,
	type: string,
): Promise<{ id: string; userId: string } | null> {
	const token = await db
		.prepare(
			"UPDATE tokens SET used = 1 WHERE id = (SELECT id FROM tokens WHERE code = ? AND type = ? AND used = 0 AND expires_at > datetime('now') LIMIT 1) AND used = 0 RETURNING id, user_id",
		)
		.bind(code, type)
		.first<{ id: string; user_id: string }>();

	if (!token) return null;
	return { id: token.id, userId: token.user_id };
}

export async function markTokenUsed(db: D1Database, id: string): Promise<void> {
	await db.prepare('UPDATE tokens SET used = 1 WHERE id = ?').bind(id).run();
}
