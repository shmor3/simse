import { Hono } from 'hono';
import { sendAuditEvent } from '../lib/audit';
import { sendEmail } from '../lib/comms';
import { generateId } from '../lib/db';
import { checkRateLimit } from '../lib/rate-limit';
import { inviteSchema, updateRoleSchema } from '../schemas';
import type { Env } from '../types';

const teams = new Hono<{ Bindings: Env }>();

// GET /teams/me
teams.get('/me', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId)
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } },
			401,
		);

	const db = c.env.DB;
	const team = await db
		.prepare(
			'SELECT t.id, t.name, t.plan FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1',
		)
		.bind(userId)
		.first<{ id: string; name: string; plan: string }>();

	if (!team) {
		return c.json(
			{ error: { code: 'NOT_FOUND', message: 'No team found' } },
			404,
		);
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
		.all<{ id: string; email: string; role: string; created_at: string }>();

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
	const parsed = inviteSchema.safeParse(body);
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
	const normalizedEmail = parsed.data.email.toLowerCase();

	// Rate limit invites — 10 per hour per user
	const rl = await checkRateLimit(db, `invite:${userId}`, 3600, 10);
	if (!rl.allowed) {
		return c.json(
			{
				error: {
					code: 'RATE_LIMITED',
					message: 'Too many invites. Please try again later.',
				},
			},
			429,
		);
	}

	const membership = await db
		.prepare(
			"SELECT t.id as team_id, t.name as team_name, tm.role as caller_role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1",
		)
		.bind(userId)
		.first<{ team_id: string; team_name: string; caller_role: string }>();

	if (!membership) {
		return c.json(
			{
				error: {
					code: 'FORBIDDEN',
					message: 'Only owners and admins can invite',
				},
			},
			403,
		);
	}

	const existingMember = await db
		.prepare(
			'SELECT 1 FROM team_members tm JOIN users u ON tm.user_id = u.id WHERE tm.team_id = ? AND LOWER(u.email) = ?',
		)
		.bind(membership.team_id, normalizedEmail)
		.first();

	if (existingMember) {
		return c.json(
			{
				error: {
					code: 'ALREADY_MEMBER',
					message: 'User is already a team member',
				},
			},
			409,
		);
	}

	// Check for existing pending invite to prevent duplicates
	const existingInvite = await db
		.prepare(
			"SELECT id FROM team_invites WHERE team_id = ? AND LOWER(email) = ? AND expires_at > datetime('now')",
		)
		.bind(membership.team_id, normalizedEmail)
		.first();

	if (existingInvite) {
		return c.json(
			{
				error: {
					code: 'ALREADY_INVITED',
					message: 'An invite is already pending for this email',
				},
			},
			409,
		);
	}

	// Only owners can invite as admin (consistent with role update rules)
	if (parsed.data.role === 'admin' && membership.caller_role !== 'owner') {
		return c.json(
			{
				error: {
					code: 'FORBIDDEN',
					message: 'Only the team owner can invite as admin',
				},
			},
			403,
		);
	}

	const inviteId = generateId();
	const expiresAt = new Date(
		Date.now() + 7 * 24 * 60 * 60 * 1000,
	).toISOString();

	await db
		.prepare(
			'INSERT INTO team_invites (id, team_id, email, role, invited_by, expires_at) VALUES (?, ?, ?, ?, ?, ?)',
		)
		.bind(
			inviteId,
			membership.team_id,
			normalizedEmail,
			parsed.data.role,
			userId,
			expiresAt,
		)
		.run();

	const inviter = await db
		.prepare('SELECT name FROM users WHERE id = ?')
		.bind(userId)
		.first<{ name: string }>();

	await sendEmail(c.env.ANALYTICS_QUEUE, 'team-invite', normalizedEmail, {
		inviterName: inviter?.name ?? 'A team member',
		teamName: membership.team_name,
		inviteUrl: `https://app.simse.dev/invite/${inviteId}`,
	});

	sendAuditEvent(c.env.ANALYTICS_QUEUE, 'team.invited', userId, {
		email: normalizedEmail,
		teamId: membership.team_id,
	});

	return c.json({ data: { id: inviteId } }, 201);
});

// PUT /teams/me/members/:userId/role
teams.put('/me/members/:userId/role', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId)
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } },
			401,
		);

	const targetUserId = c.req.param('userId');
	let body: unknown;
	try {
		body = await c.req.json();
	} catch {
		return c.json(
			{ error: { code: 'INVALID_BODY', message: 'Invalid JSON body' } },
			400,
		);
	}
	const parsed = updateRoleSchema.safeParse(body);

	if (!parsed.success) {
		return c.json(
			{
				error: {
					code: 'VALIDATION_ERROR',
					message: 'Role must be admin or member',
				},
			},
			400,
		);
	}

	const db = c.env.DB;

	const membership = await db
		.prepare(
			"SELECT t.id as team_id, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1",
		)
		.bind(userId)
		.first<{ team_id: string; role: string }>();

	if (!membership) {
		return c.json(
			{ error: { code: 'FORBIDDEN', message: 'Insufficient permissions' } },
			403,
		);
	}

	// Only owners can promote to admin
	if (parsed.data.role === 'admin' && membership.role !== 'owner') {
		return c.json(
			{
				error: {
					code: 'FORBIDDEN',
					message: 'Only the team owner can promote members to admin',
				},
			},
			403,
		);
	}

	if (targetUserId === userId) {
		return c.json(
			{
				error: {
					code: 'INVALID_OPERATION',
					message: 'Cannot change your own role',
				},
			},
			400,
		);
	}

	const targetMember = await db
		.prepare('SELECT role FROM team_members WHERE team_id = ? AND user_id = ?')
		.bind(membership.team_id, targetUserId)
		.first<{ role: string }>();

	if (!targetMember) {
		return c.json(
			{
				error: {
					code: 'NOT_FOUND',
					message: 'User is not a member of this team',
				},
			},
			404,
		);
	}

	if (targetMember.role === 'owner') {
		return c.json(
			{ error: { code: 'FORBIDDEN', message: 'Cannot change the owner role' } },
			403,
		);
	}

	await db
		.prepare(
			'UPDATE team_members SET role = ? WHERE team_id = ? AND user_id = ?',
		)
		.bind(parsed.data.role, membership.team_id, targetUserId)
		.run();

	sendAuditEvent(c.env.ANALYTICS_QUEUE, 'role.changed', userId, {
		targetUserId,
		newRole: parsed.data.role,
		teamId: membership.team_id,
	});

	return c.json({ data: { ok: true } });
});

// DELETE /teams/me/invites/:inviteId
teams.delete('/me/invites/:inviteId', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId)
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } },
			401,
		);

	const inviteId = c.req.param('inviteId');
	const db = c.env.DB;

	const membership = await db
		.prepare(
			"SELECT t.id as team_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1",
		)
		.bind(userId)
		.first<{ team_id: string }>();

	if (!membership) {
		return c.json(
			{ error: { code: 'FORBIDDEN', message: 'Insufficient permissions' } },
			403,
		);
	}

	await db
		.prepare('DELETE FROM team_invites WHERE id = ? AND team_id = ?')
		.bind(inviteId, membership.team_id)
		.run();

	sendAuditEvent(c.env.ANALYTICS_QUEUE, 'team.invite_deleted', userId, {
		inviteId,
		teamId: membership.team_id,
	});

	return c.json({ data: { ok: true } });
});

export default teams;
