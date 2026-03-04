import { Hono } from 'hono';
import { createCheckoutSession, createStripe } from '../lib/stripe';
import type { Env } from '../types';

const checkout = new Hono<{ Bindings: Env }>();

checkout.post('/', async (c) => {
	const body = await c.req.json<{
		teamId: string;
		priceId: string;
		appUrl: string;
	}>();

	if (!body.teamId || !body.priceId || !body.appUrl) {
		return c.json(
			{ error: 'Missing required fields: teamId, priceId, appUrl' },
			400,
		);
	}

	const db = c.env.DB;
	const customer = await db
		.prepare('SELECT stripe_customer_id FROM customers WHERE team_id = ?')
		.bind(body.teamId)
		.first<{ stripe_customer_id: string }>();

	if (!customer) {
		return c.json({ error: 'Customer not found. Create customer first.' }, 404);
	}

	const stripe = createStripe(c.env.STRIPE_SECRET_KEY);
	const url = await createCheckoutSession(
		stripe,
		customer.stripe_customer_id,
		body.priceId,
		body.appUrl,
	);

	return c.json({ url });
});

export default checkout;
