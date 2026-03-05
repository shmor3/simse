import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { sendEmail } from '../lib/comms';
import { hashPassword, verifyPassword } from '../lib/password';
import { createSession, deleteSession } from '../lib/session';
import { createToken, generateCode, markTokenUsed, validateToken } from '../lib/token';
import {
	loginSchema,
	newPasswordSchema,
	registerSchema,
	resetPasswordSchema,
	twoFactorSchema,
} from '../schemas';
import type { AuthContext, Env } from '../types';

const auth = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// POST /auth/register
auth.post('/register', async (c) => {
	const body = await c.req.json();
	const parsed = registerSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const { name, email, password } = parsed.data;
	const normalizedEmail = email.toLowerCase();
	const db = c.env.DB;

	const existing = await db
		.prepare('SELECT id FROM users WHERE LOWER(email) = ?')
		.bind(normalizedEmail)
		.first();

	if (existing) {
		return c.json({ error: { code: 'EMAIL_EXISTS', message: 'Email already registered' } }, 409);
	}

	const userId = generateId();
	const passwordHash = await hashPassword(password);
	const teamId = generateId();
	const tokenId = generateId();
	const verifyCode = generateCode();
	const tokenExpires = new Date(Date.now() + 15 * 60 * 1000).toISOString();

	await db.batch([
		db.prepare('INSERT INTO users (id, email, name, password_hash) VALUES (?, ?, ?, ?)').bind(userId, normalizedEmail, name, passwordHash),
		db.prepare('INSERT INTO teams (id, name) VALUES (?, ?)').bind(teamId, `${name}'s Team`),
		db.prepare("INSERT INTO team_members (team_id, user_id, role) VALUES (?, ?, 'owner')").bind(teamId, userId),
		db.prepare('INSERT INTO tokens (id, user_id, type, code, expires_at) VALUES (?, ?, ?, ?, ?)').bind(tokenId, userId, 'email_verify', verifyCode, tokenExpires),
	]);

	const token = await createSession(db, userId);

	// Send verification email via queue
	await sendEmail(c.env.COMMS_QUEUE, 'verify-email', normalizedEmail, { code: verifyCode });

	return c.json({
		data: {
			token,
			user: { id: userId, email: normalizedEmail, name },
		},
	}, 201);
});

// POST /auth/login
auth.post('/login', async (c) => {
	const body = await c.req.json();
	const parsed = loginSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const { email, password } = parsed.data;
	const db = c.env.DB;

	const user = await db
		.prepare('SELECT id, email, password_hash, two_factor_enabled FROM users WHERE LOWER(email) = ?')
		.bind(email.toLowerCase())
		.first<{ id: string; email: string; password_hash: string; two_factor_enabled: number }>();

	if (!user || !(await verifyPassword(password, user.password_hash))) {
		return c.json({ error: { code: 'INVALID_CREDENTIALS', message: 'Invalid email or password' } }, 401);
	}

	if (user.two_factor_enabled) {
		const { id, code } = await createToken(db, user.id, '2fa', 10);
		// Send 2FA code via queue
		await sendEmail(c.env.COMMS_QUEUE, 'two-factor', user.email, { code });
		return c.json({ data: { requires2fa: true, pendingToken: id } });
	}

	const token = await createSession(db, user.id);
	return c.json({ data: { token, user: { id: user.id } } });
});

// POST /auth/2fa
auth.post('/2fa', async (c) => {
	const body = await c.req.json();
	const parsed = twoFactorSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const { code, pendingToken } = parsed.data;
	const db = c.env.DB;

	const pending = await db
		.prepare("SELECT user_id FROM tokens WHERE id = ? AND type = '2fa' AND used = 0 AND expires_at > datetime('now')")
		.bind(pendingToken)
		.first<{ user_id: string }>();

	if (!pending) {
		return c.json({ error: { code: 'INVALID_TOKEN', message: '2FA session expired' } }, 401);
	}

	const codeToken = await validateToken(db, code, '2fa');
	if (!codeToken || codeToken.userId !== pending.user_id) {
		return c.json({ error: { code: 'INVALID_CODE', message: 'Invalid 2FA code' } }, 401);
	}

	await markTokenUsed(db, pendingToken);
	await markTokenUsed(db, codeToken.id);

	const token = await createSession(db, pending.user_id);
	return c.json({ data: { token, user: { id: pending.user_id } } });
});

// POST /auth/logout (requires auth — called via gateway with X-User-Id)
auth.post('/logout', async (c) => {
	const sessionId = c.req.header('X-Session-Id');
	if (sessionId) {
		await deleteSession(c.env.DB, sessionId);
	}
	return c.json({ data: { ok: true } });
});

// POST /auth/reset-password
auth.post('/reset-password', async (c) => {
	const body = await c.req.json();
	const parsed = resetPasswordSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const user = await db
		.prepare('SELECT id, email FROM users WHERE LOWER(email) = ?')
		.bind(parsed.data.email.toLowerCase())
		.first<{ id: string; email: string }>();

	if (user) {
		const { code } = await createToken(db, user.id, 'password_reset', 60);
		await sendEmail(c.env.COMMS_QUEUE, 'reset-password', user.email, { code });
	}

	return c.json({ data: { ok: true } });
});

// POST /auth/new-password
auth.post('/new-password', async (c) => {
	const body = await c.req.json();
	const parsed = newPasswordSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const token = await validateToken(db, parsed.data.token, 'password_reset');
	if (!token) {
		return c.json({ error: { code: 'INVALID_TOKEN', message: 'Invalid or expired reset token' } }, 400);
	}

	const passwordHash = await hashPassword(parsed.data.password);
	await db.prepare('UPDATE users SET password_hash = ? WHERE id = ?').bind(passwordHash, token.userId).run();
	await markTokenUsed(db, token.id);

	return c.json({ data: { ok: true } });
});

// POST /auth/verify-email
auth.post('/verify-email', async (c) => {
	const body = await c.req.json<{ code: string }>();
	if (!body.code) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: 'Code required' } }, 400);
	}

	const db = c.env.DB;
	const token = await validateToken(db, body.code, 'email_verify');
	if (!token) {
		return c.json({ error: { code: 'INVALID_TOKEN', message: 'Invalid or expired code' } }, 400);
	}

	await db.prepare('UPDATE users SET email_verified = 1 WHERE id = ?').bind(token.userId).run();
	await markTokenUsed(db, token.id);

	return c.json({ data: { ok: true } });
});

// POST /auth/validate — called by simse-api gateway
auth.post('/validate', async (c) => {
	const body = await c.req.json<{ token: string }>();
	if (!body.token) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: 'Token required' } }, 400);
	}

	const db = c.env.DB;
	const token = body.token;
	let userId: string | null = null;
	let sessionId: string | undefined;

	if (token.startsWith('session_')) {
		const row = await db
			.prepare("SELECT user_id FROM sessions WHERE id = ? AND expires_at > datetime('now')")
			.bind(token)
			.first<{ user_id: string }>();
		if (row) {
			userId = row.user_id;
			sessionId = token;
		}
	} else if (token.startsWith('sk_')) {
		const encoder = new TextEncoder();
		const data = encoder.encode(token);
		const hashBuffer = await crypto.subtle.digest('SHA-256', data);
		const hashArray = new Uint8Array(hashBuffer);
		const keyHash = btoa(String.fromCharCode(...hashArray));

		const row = await db
			.prepare('SELECT user_id FROM api_keys WHERE key_hash = ?')
			.bind(keyHash)
			.first<{ user_id: string }>();

		if (row) {
			userId = row.user_id;
			await db.prepare("UPDATE api_keys SET last_used_at = datetime('now') WHERE key_hash = ?").bind(keyHash).run();
		}
	}

	if (!userId) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	// Get team info for RBAC
	const team = await db
		.prepare('SELECT t.id, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1')
		.bind(userId)
		.first<{ id: string; role: string }>();

	return c.json({
		data: {
			userId,
			sessionId,
			teamId: team?.id ?? null,
			role: team?.role ?? null,
		},
	});
});

// GET /auth/me — requires auth headers from gateway
auth.get('/me', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);
	}

	const db = c.env.DB;
	const user = await db
		.prepare('SELECT id, email, name, email_verified, two_factor_enabled, created_at FROM users WHERE id = ?')
		.bind(userId)
		.first<{
			id: string;
			email: string;
			name: string;
			email_verified: number;
			two_factor_enabled: number;
			created_at: string;
		}>();

	if (!user) {
		return c.json({ error: { code: 'NOT_FOUND', message: 'User not found' } }, 404);
	}

	const team = await db
		.prepare('SELECT t.id, t.name, t.plan, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1')
		.bind(userId)
		.first<{ id: string; name: string; plan: string; role: string }>();

	return c.json({
		data: {
			id: user.id,
			email: user.email,
			name: user.name,
			emailVerified: !!user.email_verified,
			twoFactorEnabled: !!user.two_factor_enabled,
			createdAt: user.created_at,
			team: team ? { id: team.id, name: team.name, plan: team.plan, role: team.role } : null,
		},
	});
});

export default auth;
