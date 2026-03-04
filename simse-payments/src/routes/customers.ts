import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { createStripe } from '../lib/stripe';
import type { Env } from '../types';

const customers = new Hono<{ Bindings: Env }>();

// POST /customers — create or get Stripe customer
customers.post('/', async (c) => {
	const body = await c.req.json<{
		teamId: string;
		email: string;
		name: string;
	}>();

	if (!body.teamId || !body.email || !body.name) {
		return c.json(
			{ error: 'Missing required fields: teamId, email, name' },
			400,
		);
	}

	const db = c.env.DB;

	// Check if customer already exists
	const existing = await db
		.prepare('SELECT stripe_customer_id FROM customers WHERE team_id = ?')
		.bind(body.teamId)
		.first<{ stripe_customer_id: string }>();

	if (existing) {
		return c.json({ customerId: existing.stripe_customer_id });
	}

	// Create in Stripe
	const stripe = createStripe(c.env.STRIPE_SECRET_KEY);
	const customer = await stripe.customers.create({
		email: body.email,
		name: body.name,
		metadata: { teamId: body.teamId },
	});

	// Store locally
	await db
		.prepare(
			'INSERT INTO customers (team_id, stripe_customer_id, email, name) VALUES (?, ?, ?, ?)',
		)
		.bind(body.teamId, customer.id, body.email, body.name)
		.run();

	// Create default free subscription record
	await db
		.prepare(
			"INSERT INTO subscriptions (id, team_id, plan, status) VALUES (?, ?, 'free', 'active')",
		)
		.bind(generateId(), body.teamId)
		.run();

	return c.json({ customerId: customer.id });
});

export default customers;
