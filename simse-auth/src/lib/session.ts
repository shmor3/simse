import { generateId } from './db';

const SESSION_TTL_DAYS = 30;
const MAX_SESSIONS_PER_USER = 10;

async function hashToken(token: string): Promise<string> {
	const encoder = new TextEncoder();
	const data = encoder.encode(token);
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = new Uint8Array(hashBuffer);
	return btoa(String.fromCharCode(...hashArray));
}

export async function createSession(
	db: D1Database,
	userId: string,
): Promise<string> {
	const rawToken = `session_${generateId()}`;
	const tokenHash = await hashToken(rawToken);
	const expiresAt = new Date(
		Date.now() + SESSION_TTL_DAYS * 24 * 60 * 60 * 1000,
	).toISOString();

	// Enforce max sessions — delete oldest if at limit
	const count = await db
		.prepare(
			"SELECT COUNT(*) as cnt FROM sessions WHERE user_id = ? AND expires_at > datetime('now')",
		)
		.bind(userId)
		.first<{ cnt: number }>();

	if (count && count.cnt >= MAX_SESSIONS_PER_USER) {
		await db
			.prepare(
				'DELETE FROM sessions WHERE id IN (SELECT id FROM sessions WHERE user_id = ? ORDER BY created_at ASC LIMIT ?)',
			)
			.bind(userId, count.cnt - MAX_SESSIONS_PER_USER + 1)
			.run();
	}

	await db
		.prepare('INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, ?)')
		.bind(tokenHash, userId, expiresAt)
		.run();

	return rawToken;
}

export async function validateSession(
	db: D1Database,
	sessionId: string,
): Promise<string | null> {
	const tokenHash = await hashToken(sessionId);
	const session = await db
		.prepare(
			"SELECT user_id FROM sessions WHERE id = ? AND expires_at > datetime('now')",
		)
		.bind(tokenHash)
		.first<{ user_id: string }>();

	return session?.user_id ?? null;
}

export async function deleteSession(
	db: D1Database,
	sessionId: string,
): Promise<void> {
	const tokenHash = await hashToken(sessionId);
	await db.prepare('DELETE FROM sessions WHERE id = ?').bind(tokenHash).run();
}

export async function deleteAllUserSessions(
	db: D1Database,
	userId: string,
): Promise<void> {
	await db.prepare('DELETE FROM sessions WHERE user_id = ?').bind(userId).run();
}
