const COOKIE_NAME = 'simse_session';

export interface Session {
	userId: string;
	sessionId: string;
}

export async function getSession(
	request: Request,
	env: Env,
): Promise<Session | null> {
	const cookie = request.headers.get('Cookie');
	if (!cookie) return null;

	const match = cookie.match(new RegExp(`${COOKIE_NAME}=([^;]+)`));
	if (!match) return null;

	const sessionId = match[1];

	const row = await env.DB.prepare(
		"SELECT id, user_id FROM sessions WHERE id = ? AND expires_at > datetime('now')",
	)
		.bind(sessionId)
		.first<{ id: string; user_id: string }>();

	if (!row) return null;

	return { userId: row.user_id, sessionId: row.id };
}

export function setSessionCookie(sessionId: string): string {
	return `${COOKIE_NAME}=${sessionId}; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=${30 * 24 * 60 * 60}`;
}

export function clearSessionCookie(): string {
	return `${COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=0`;
}
