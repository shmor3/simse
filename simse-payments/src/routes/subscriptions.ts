import { Hono } from 'hono';
import type { Env } from '../types';

const subscriptions = new Hono<{ Bindings: Env }>();

// GET /subscriptions/:teamId — get current plan
subscriptions.get('/:teamId', async (c) => {
	const teamId = c.req.param('teamId');
	const db = c.env.DB;

	const sub = await db
		.prepare(
			'SELECT team_id, stripe_subscription_id, plan, status FROM subscriptions WHERE team_id = ?',
		)
		.bind(teamId)
		.first<{
			team_id: string;
			stripe_subscription_id: string | null;
			plan: string;
			status: string;
		}>();

	if (!sub) {
		return c.json({
			teamId,
			plan: 'free',
			status: 'active',
			stripeSubscriptionId: null,
		});
	}

	return c.json({
		teamId: sub.team_id,
		plan: sub.plan,
		status: sub.status,
		stripeSubscriptionId: sub.stripe_subscription_id,
	});
});

export default subscriptions;
