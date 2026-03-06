import { generateId } from './db';

const SESSION_TTL_DAYS = 30;

export async function createSession(
	db: D1Database,
	userId: string,
): Promise<string> {
	const id = `session_${generateId()}`;
	const expiresAt = new Date(
		Date.now() + SESSION_TTL_DAYS * 24 * 60 * 60 * 1000,
	).toISOString();

	await db
		.prepare('INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, ?)')
		.bind(id, userId, expiresAt)
		.run();

	return id;
}

export async function validateSession(
	db: D1Database,
	sessionId: string,
): Promise<string | null> {
	const session = await db
		.prepare(
			"SELECT user_id FROM sessions WHERE id = ? AND expires_at > datetime('now')",
		)
		.bind(sessionId)
		.first<{ user_id: string }>();

	return session?.user_id ?? null;
}

export async function deleteSession(
	db: D1Database,
	sessionId: string,
): Promise<void> {
	await db.prepare('DELETE FROM sessions WHERE id = ?').bind(sessionId).run();
}
