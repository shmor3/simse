import { Hono } from 'hono';
import { hashPassword, verifyPassword } from '../lib/password';
import { changePasswordSchema, deleteAccountSchema, updateNameSchema } from '../schemas';
import type { Env } from '../types';

const users = new Hono<{ Bindings: Env }>();

// PUT /users/me/name
users.put('/me/name', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const body = await c.req.json();
	const parsed = updateNameSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	await c.env.DB.prepare('UPDATE users SET name = ? WHERE id = ?').bind(parsed.data.name, userId).run();
	return c.json({ data: { ok: true } });
});

// PUT /users/me/password
users.put('/me/password', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const body = await c.req.json();
	const parsed = changePasswordSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const user = await db.prepare('SELECT password_hash FROM users WHERE id = ?').bind(userId).first<{ password_hash: string }>();
	if (!user || !(await verifyPassword(parsed.data.currentPassword, user.password_hash))) {
		return c.json({ error: { code: 'INVALID_PASSWORD', message: 'Current password is incorrect' } }, 400);
	}

	const newHash = await hashPassword(parsed.data.newPassword);
	await db.prepare('UPDATE users SET password_hash = ? WHERE id = ?').bind(newHash, userId).run();
	return c.json({ data: { ok: true } });
});

// DELETE /users/me
users.delete('/me', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const body = await c.req.json();
	const parsed = deleteAccountSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const user = await db.prepare('SELECT email FROM users WHERE id = ?').bind(userId).first<{ email: string }>();
	if (!user || parsed.data.confirmEmail.toLowerCase() !== user.email.toLowerCase()) {
		return c.json({ error: { code: 'EMAIL_MISMATCH', message: 'Email does not match' } }, 400);
	}

	await db.batch([
		db.prepare('DELETE FROM sessions WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM tokens WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM api_keys WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM team_invites WHERE invited_by = ?').bind(userId),
		db.prepare('DELETE FROM team_members WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM users WHERE id = ?').bind(userId),
	]);

	return c.json({ data: { ok: true } });
});

export default users;
