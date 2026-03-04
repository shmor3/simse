import { Hono } from 'hono';
import { generateId } from '../lib/db';
import type { Env } from '../types';

const credits = new Hono<{ Bindings: Env }>();

// GET /credits/:userId/usage — last 7 days usage (for dashboard.usage)
// Must be registered before /:userId to avoid param capture
credits.get('/:userId/usage', async (c) => {
	const userId = c.req.param('userId');
	const db = c.env.DB;

	const balance = await db
		.prepare(
			'SELECT COALESCE(SUM(amount), 0) as total FROM credit_ledger WHERE user_id = ?',
		)
		.bind(userId)
		.first<{ total: number }>();

	const recentUsage = await db
		.prepare(
			"SELECT date(created_at) as day, SUM(ABS(amount)) as tokens FROM credit_ledger WHERE user_id = ? AND amount < 0 AND created_at > datetime('now', '-7 days') GROUP BY date(created_at) ORDER BY day",
		)
		.bind(userId)
		.all<{ day: string; tokens: number }>();

	return c.json({
		balance: balance?.total ?? 0,
		recentUsage: recentUsage.results,
	});
});

// GET /credits/:userId — balance + recent history
credits.get('/:userId', async (c) => {
	const userId = c.req.param('userId');
	const db = c.env.DB;

	const balance = await db
		.prepare(
			'SELECT COALESCE(SUM(amount), 0) as total FROM credit_ledger WHERE user_id = ?',
		)
		.bind(userId)
		.first<{ total: number }>();

	const history = await db
		.prepare(
			'SELECT id, amount, description, created_at FROM credit_ledger WHERE user_id = ? ORDER BY created_at DESC LIMIT 50',
		)
		.bind(userId)
		.all<{
			id: string;
			amount: number;
			description: string;
			created_at: string;
		}>();

	return c.json({
		balance: balance?.total ?? 0,
		history: history.results,
	});
});

// POST /credits — add/deduct credit
credits.post('/', async (c) => {
	const body = await c.req.json<{
		userId: string;
		amount: number;
		description: string;
	}>();

	if (!body.userId || body.amount === undefined || !body.description) {
		return c.json(
			{ error: 'Missing required fields: userId, amount, description' },
			400,
		);
	}

	const db = c.env.DB;
	const id = generateId();

	await db
		.prepare(
			'INSERT INTO credit_ledger (id, user_id, amount, description) VALUES (?, ?, ?, ?)',
		)
		.bind(id, body.userId, body.amount, body.description)
		.run();

	const balance = await db
		.prepare(
			'SELECT COALESCE(SUM(amount), 0) as total FROM credit_ledger WHERE user_id = ?',
		)
		.bind(body.userId)
		.first<{ total: number }>();

	return c.json({ id, balance: balance?.total ?? 0 });
});

export default credits;
