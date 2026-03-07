async function hashToken(token: string): Promise<string> {
	const encoder = new TextEncoder();
	const data = encoder.encode(token);
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = new Uint8Array(hashBuffer);
	return btoa(String.fromCharCode(...hashArray));
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
