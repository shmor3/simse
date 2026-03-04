import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { sendEmail } from '../lib/mailer';
import { createStripe, verifyWebhookSignature } from '../lib/stripe';
import type { Env } from '../types';

const webhooks = new Hono<{ Bindings: Env }>();

webhooks.post('/stripe', async (c) => {
	const stripe = createStripe(c.env.STRIPE_SECRET_KEY);
	const body = await c.req.text();
	const signature = c.req.header('Stripe-Signature');

	if (!signature) {
		return c.json({ error: 'Missing signature' }, 400);
	}

	let event: Awaited<ReturnType<typeof verifyWebhookSignature>>;
	try {
		event = await verifyWebhookSignature(
			stripe,
			body,
			signature,
			c.env.STRIPE_WEBHOOK_SECRET,
		);
	} catch {
		return c.json({ error: 'Invalid signature' }, 400);
	}

	const db = c.env.DB;

	switch (event.type) {
		case 'customer.subscription.created':
		case 'customer.subscription.updated': {
			const sub = event.data.object;
			const customerId =
				typeof sub.customer === 'string' ? sub.customer : sub.customer.id;

			const plan =
				sub.status === 'active'
					? (sub.items.data[0]?.price?.lookup_key ?? 'pro')
					: 'free';

			const status = sub.status === 'active' ? 'active' : 'inactive';

			// Find team by stripe customer ID
			const customer = await db
				.prepare('SELECT team_id FROM customers WHERE stripe_customer_id = ?')
				.bind(customerId)
				.first<{ team_id: string }>();

			if (customer) {
				// Upsert subscription
				const existing = await db
					.prepare('SELECT id FROM subscriptions WHERE team_id = ?')
					.bind(customer.team_id)
					.first<{ id: string }>();

				if (existing) {
					await db
						.prepare(
							"UPDATE subscriptions SET plan = ?, status = ?, stripe_subscription_id = ?, updated_at = datetime('now') WHERE team_id = ?",
						)
						.bind(plan, status, sub.id, customer.team_id)
						.run();
				} else {
					await db
						.prepare(
							'INSERT INTO subscriptions (id, team_id, stripe_subscription_id, plan, status) VALUES (?, ?, ?, ?, ?)',
						)
						.bind(generateId(), customer.team_id, sub.id, plan, status)
						.run();
				}
			}
			break;
		}

		case 'customer.subscription.deleted': {
			const sub = event.data.object;
			const customerId =
				typeof sub.customer === 'string' ? sub.customer : sub.customer.id;

			const customer = await db
				.prepare('SELECT team_id FROM customers WHERE stripe_customer_id = ?')
				.bind(customerId)
				.first<{ team_id: string }>();

			if (customer) {
				await db
					.prepare(
						"UPDATE subscriptions SET plan = 'free', status = 'canceled', stripe_subscription_id = NULL, updated_at = datetime('now') WHERE team_id = ?",
					)
					.bind(customer.team_id)
					.run();
			}
			break;
		}

		case 'invoice.payment_succeeded': {
			const invoice = event.data.object;
			const customerId =
				typeof invoice.customer === 'string'
					? invoice.customer
					: invoice.customer?.id;

			if (customerId) {
				const customer = await db
					.prepare(
						'SELECT email, name FROM customers WHERE stripe_customer_id = ?',
					)
					.bind(customerId)
					.first<{ email: string; name: string }>();

				if (customer) {
					const amount = `$${((invoice.amount_paid ?? 0) / 100).toFixed(2)}`;
					await sendEmail(
						c.env.MAILER_API_URL,
						c.env.MAILER_API_SECRET,
						customer.email,
						`Receipt for your simse payment — ${amount}`,
						`<p>Payment of ${amount} received. Thank you!</p>`,
					);
				}
			}
			break;
		}

		case 'invoice.payment_failed': {
			const invoice = event.data.object;
			const customerId =
				typeof invoice.customer === 'string'
					? invoice.customer
					: invoice.customer?.id;

			if (customerId) {
				const customer = await db
					.prepare(
						'SELECT email, name FROM customers WHERE stripe_customer_id = ?',
					)
					.bind(customerId)
					.first<{ email: string; name: string }>();

				if (customer) {
					const amount = `$${((invoice.amount_due ?? 0) / 100).toFixed(2)}`;
					await sendEmail(
						c.env.MAILER_API_URL,
						c.env.MAILER_API_SECRET,
						customer.email,
						`Your simse payment of ${amount} didn't go through`,
						`<p>We couldn't process your payment of ${amount}. Please update your payment method.</p>`,
					);
				}
			}
			break;
		}
	}

	return c.json({ received: true });
});

export default webhooks;
