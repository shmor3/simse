import { generateId } from './db';

const REFRESH_TOKEN_TTL_DAYS = 30;

function generateRefreshToken(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(32));
	const hex = Array.from(bytes)
		.map((b) => b.toString(16).padStart(2, '0'))
		.join('');
	return `rt_${hex}`;
}

async function hashToken(token: string): Promise<string> {
	const encoder = new TextEncoder();
	const data = encoder.encode(token);
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = new Uint8Array(hashBuffer);
	return btoa(String.fromCharCode(...hashArray));
}

export async function createRefreshToken(
	db: D1Database,
	userId: string,
	familyId?: string,
): Promise<{ token: string; familyId: string }> {
	const id = generateId();
	const family = familyId ?? generateId();
	const rawToken = generateRefreshToken();
	const tokenHash = await hashToken(rawToken);
	const expiresAt = new Date(
		Date.now() + REFRESH_TOKEN_TTL_DAYS * 24 * 60 * 60 * 1000,
	).toISOString();

	await db
		.prepare(
			'INSERT INTO refresh_tokens (id, user_id, family_id, token_hash, expires_at) VALUES (?, ?, ?, ?, ?)',
		)
		.bind(id, userId, family, tokenHash, expiresAt)
		.run();

	return { token: rawToken, familyId: family };
}

export async function rotateRefreshToken(
	db: D1Database,
	rawToken: string,
): Promise<
	| {
			ok: true;
			userId: string;
			familyId: string;
			newToken: string;
	  }
	| {
			ok: false;
			code: 'INVALID_TOKEN' | 'TOKEN_REUSED';
	  }
> {
	const tokenHash = await hashToken(rawToken);

	// Look up the token
	const row = await db
		.prepare(
			'SELECT id, user_id, family_id, revoked, expires_at FROM refresh_tokens WHERE token_hash = ?',
		)
		.bind(tokenHash)
		.first<{
			id: string;
			user_id: string;
			family_id: string;
			revoked: number;
			expires_at: string;
		}>();

	if (!row) {
		return { ok: false, code: 'INVALID_TOKEN' };
	}

	// Reuse detection: if token was already revoked, revoke entire family
	if (row.revoked) {
		await db
			.prepare('UPDATE refresh_tokens SET revoked = 1 WHERE family_id = ?')
			.bind(row.family_id)
			.run();
		return { ok: false, code: 'TOKEN_REUSED' };
	}

	// Check expiry
	if (new Date(row.expires_at) <= new Date()) {
		return { ok: false, code: 'INVALID_TOKEN' };
	}

	// Atomic revoke — prevents TOCTOU race where two concurrent requests
	// both pass the revoked check and issue duplicate tokens
	const revoked = await db
		.prepare(
			'UPDATE refresh_tokens SET revoked = 1 WHERE id = ? AND revoked = 0 RETURNING id',
		)
		.bind(row.id)
		.first<{ id: string }>();

	if (!revoked) {
		// Another concurrent request already revoked this token
		await db
			.prepare('UPDATE refresh_tokens SET revoked = 1 WHERE family_id = ?')
			.bind(row.family_id)
			.run();
		return { ok: false, code: 'TOKEN_REUSED' };
	}

	// Issue new token in same family
	const result = await createRefreshToken(db, row.user_id, row.family_id);

	return {
		ok: true,
		userId: row.user_id,
		familyId: row.family_id,
		newToken: result.token,
	};
}

export async function revokeFamily(
	db: D1Database,
	familyId: string,
): Promise<void> {
	await db
		.prepare('UPDATE refresh_tokens SET revoked = 1 WHERE family_id = ?')
		.bind(familyId)
		.run();
}

export async function revokeAllUserTokens(
	db: D1Database,
	userId: string,
): Promise<void> {
	await db
		.prepare('UPDATE refresh_tokens SET revoked = 1 WHERE user_id = ?')
		.bind(userId)
		.run();
}
