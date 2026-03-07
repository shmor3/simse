import { Hono } from 'hono';
import { sendAuditEvent } from '../lib/audit';
import { hashPassword, verifyPassword } from '../lib/password';
import { checkRateLimit } from '../lib/rate-limit';
import { revokeAllUserTokens } from '../lib/refresh-token';
import { deleteAllUserSessions } from '../lib/session';
import {
	changePasswordSchema,
	deleteAccountSchema,
	updateNameSchema,
} from '../schemas';
import type { Env } from '../types';

const users = new Hono<{ Bindings: Env }>();

// PUT /users/me/name
users.put('/me/name', async (c) => {
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
	const parsed = updateNameSchema.safeParse(body);
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

	await c.env.DB.prepare(
		"UPDATE users SET name = ?, updated_at = datetime('now') WHERE id = ?",
	)
		.bind(parsed.data.name, userId)
		.run();
	return c.json({ data: { ok: true } });
});

// PUT /users/me/password
users.put('/me/password', async (c) => {
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
	const parsed = changePasswordSchema.safeParse(body);
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

	const db = c.env.DB;

	// Rate limit password change — 5 attempts per 15 minutes per user
	const rl = await checkRateLimit(db, `chpw:${userId}`, 900, 5);
	if (!rl.allowed) {
		return c.json(
			{
				error: {
					code: 'RATE_LIMITED',
					message: 'Too many attempts. Please try again later.',
				},
			},
			429,
		);
	}

	const user = await db
		.prepare('SELECT password_hash FROM users WHERE id = ?')
		.bind(userId)
		.first<{ password_hash: string }>();
	if (
		!user ||
		!(await verifyPassword(parsed.data.currentPassword, user.password_hash))
	) {
		return c.json(
			{
				error: {
					code: 'INVALID_PASSWORD',
					message: 'Current password is incorrect',
				},
			},
			400,
		);
	}

	const newHash = await hashPassword(parsed.data.newPassword);
	await db
		.prepare(
			"UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE id = ?",
		)
		.bind(newHash, userId)
		.run();

	// Invalidate all sessions and refresh tokens — user must re-authenticate
	await deleteAllUserSessions(db, userId);
	await revokeAllUserTokens(db, userId);

	sendAuditEvent(c.env.ANALYTICS_QUEUE, 'password.changed', userId);

	return c.json({ data: { ok: true } });
});

// DELETE /users/me
users.delete('/me', async (c) => {
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
	const parsed = deleteAccountSchema.safeParse(body);
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

	const db = c.env.DB;
	const user = await db
		.prepare('SELECT email, password_hash FROM users WHERE id = ?')
		.bind(userId)
		.first<{ email: string; password_hash: string }>();
	if (
		!user ||
		parsed.data.confirmEmail.toLowerCase() !== user.email.toLowerCase()
	) {
		return c.json(
			{ error: { code: 'EMAIL_MISMATCH', message: 'Email does not match' } },
			400,
		);
	}

	// Verify password to prevent deletion via stolen session
	if (!(await verifyPassword(parsed.data.password, user.password_hash))) {
		return c.json(
			{
				error: {
					code: 'INVALID_PASSWORD',
					message: 'Password is incorrect',
				},
			},
			400,
		);
	}

	// Check if user is the sole owner of any team
	const soleOwnership = await db
		.prepare(
			"SELECT tm.team_id FROM team_members tm WHERE tm.user_id = ? AND tm.role = 'owner' AND NOT EXISTS (SELECT 1 FROM team_members tm2 WHERE tm2.team_id = tm.team_id AND tm2.role = 'owner' AND tm2.user_id != ?)",
		)
		.bind(userId, userId)
		.first<{ team_id: string }>();

	if (soleOwnership) {
		return c.json(
			{
				error: {
					code: 'SOLE_OWNER',
					message: 'Transfer team ownership before deleting your account',
				},
			},
			400,
		);
	}

	// Find teams where this user is the only member (will be orphaned)
	const orphanedTeams = await db
		.prepare(
			'SELECT team_id FROM team_members WHERE team_id IN (SELECT team_id FROM team_members WHERE user_id = ?) GROUP BY team_id HAVING COUNT(*) = 1',
		)
		.bind(userId)
		.all<{ team_id: string }>();

	const batch = [
		db.prepare('DELETE FROM sessions WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM tokens WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM refresh_tokens WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM api_keys WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM team_invites WHERE invited_by = ?').bind(userId),
		db.prepare('DELETE FROM team_members WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM users WHERE id = ?').bind(userId),
	];

	// Clean up orphaned teams and their invites
	for (const t of orphanedTeams.results) {
		batch.push(
			db.prepare('DELETE FROM team_invites WHERE team_id = ?').bind(t.team_id),
		);
		batch.push(db.prepare('DELETE FROM teams WHERE id = ?').bind(t.team_id));
	}

	await db.batch(batch);

	sendAuditEvent(c.env.ANALYTICS_QUEUE, 'account.deleted', userId);

	return c.json({ data: { ok: true } });
});

export default users;
