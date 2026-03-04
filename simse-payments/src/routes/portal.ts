import { Hono } from 'hono';
import { createBillingPortalSession, createStripe } from '../lib/stripe';
import type { Env } from '../types';

const portal = new Hono<{ Bindings: Env }>();

portal.post('/', async (c) => {
	const body = await c.req.json<{
		teamId: string;
		appUrl: string;
	}>();

	if (!body.teamId || !body.appUrl) {
		return c.json({ error: 'Missing required fields: teamId, appUrl' }, 400);
	}

	const db = c.env.DB;
	const customer = await db
		.prepare('SELECT stripe_customer_id FROM customers WHERE team_id = ?')
		.bind(body.teamId)
		.first<{ stripe_customer_id: string }>();

	if (!customer) {
		return c.json({ error: 'Customer not found' }, 404);
	}

	const stripe = createStripe(c.env.STRIPE_SECRET_KEY);
	const url = await createBillingPortalSession(
		stripe,
		customer.stripe_customer_id,
		body.appUrl,
	);

	return c.json({ url });
});

export default portal;
