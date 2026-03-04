import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { inviteSchema } from '../schemas';
import type { AuthContext, Env } from '../types';

const teams = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// GET /teams/me
teams.get('/me', async (c) => {
	const auth = c.get('auth');
	const db = c.env.DB;

	const team = await db
		.prepare(
			'SELECT t.id, t.name, t.plan FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1',
		)
		.bind(auth.userId)
		.first<{ id: string; name: string; plan: string }>();

	if (!team) {
		return c.json({ error: { code: 'NOT_FOUND', message: 'No team found' } }, 404);
	}

	const members = await db
		.prepare(
			'SELECT u.id, u.name, u.email, tm.role, tm.joined_at FROM team_members tm JOIN users u ON tm.user_id = u.id WHERE tm.team_id = ?',
		)
		.bind(team.id)
		.all<{
			id: string;
			name: string;
			email: string;
			role: string;
			joined_at: string;
		}>();

	const invites = await db
		.prepare(
			"SELECT id, email, role, created_at FROM team_invites WHERE team_id = ? AND expires_at > datetime('now')",
		)
		.bind(team.id)
		.all<{
			id: string;
			email: string;
			role: string;
			created_at: string;
		}>();

	return c.json({
		data: {
			id: team.id,
			name: team.name,
			plan: team.plan,
			members: members.results,
			invites: invites.results,
		},
	});
});

// POST /teams/me/invite
teams.post('/me/invite', async (c) => {
	const auth = c.get('auth');
	const body = await c.req.json();
	const parsed = inviteSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;

	const membership = await db
		.prepare(
			"SELECT t.id as team_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1",
		)
		.bind(auth.userId)
		.first<{ team_id: string }>();

	if (!membership) {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Only owners and admins can invite' } }, 403);
	}

	const existingMember = await db
		.prepare(
			'SELECT 1 FROM team_members tm JOIN users u ON tm.user_id = u.id WHERE tm.team_id = ? AND LOWER(u.email) = ?',
		)
		.bind(membership.team_id, parsed.data.email.toLowerCase())
		.first();

	if (existingMember) {
		return c.json({ error: { code: 'ALREADY_MEMBER', message: 'User is already a team member' } }, 409);
	}

	const inviteId = generateId();
	const expiresAt = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString();

	await db
		.prepare(
			'INSERT INTO team_invites (id, team_id, email, role, invited_by, expires_at) VALUES (?, ?, ?, ?, ?, ?)',
		)
		.bind(inviteId, membership.team_id, parsed.data.email.toLowerCase(), parsed.data.role, auth.userId, expiresAt)
		.run();

	return c.json({ data: { id: inviteId } }, 201);
});

// PUT /teams/me/members/:userId/role
teams.put('/me/members/:userId/role', async (c) => {
	const auth = c.get('auth');
	const targetUserId = c.req.param('userId');
	const body = await c.req.json<{ role: string }>();

	if (!body.role || !['admin', 'member'].includes(body.role)) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: 'Role must be admin or member' } }, 400);
	}

	const db = c.env.DB;

	const membership = await db
		.prepare(
			"SELECT t.id as team_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1",
		)
		.bind(auth.userId)
		.first<{ team_id: string }>();

	if (!membership) {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Insufficient permissions' } }, 403);
	}

	// Prevent self-demotion
	if (targetUserId === auth.userId) {
		return c.json({ error: { code: 'INVALID_OPERATION', message: 'Cannot change your own role' } }, 400);
	}

	// Prevent demoting the owner
	const targetMember = await db
		.prepare('SELECT role FROM team_members WHERE team_id = ? AND user_id = ?')
		.bind(membership.team_id, targetUserId)
		.first<{ role: string }>();

	if (targetMember?.role === 'owner') {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Cannot change the owner role' } }, 403);
	}

	await db
		.prepare('UPDATE team_members SET role = ? WHERE team_id = ? AND user_id = ?')
		.bind(body.role, membership.team_id, targetUserId)
		.run();

	return c.json({ data: { ok: true } });
});

// DELETE /teams/me/invites/:inviteId
teams.delete('/me/invites/:inviteId', async (c) => {
	const auth = c.get('auth');
	const inviteId = c.req.param('inviteId');
	const db = c.env.DB;

	const membership = await db
		.prepare(
			"SELECT t.id as team_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1",
		)
		.bind(auth.userId)
		.first<{ team_id: string }>();

	if (!membership) {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Insufficient permissions' } }, 403);
	}

	await db
		.prepare('DELETE FROM team_invites WHERE id = ? AND team_id = ?')
		.bind(inviteId, membership.team_id)
		.run();

	return c.json({ data: { ok: true } });
});

export default teams;
