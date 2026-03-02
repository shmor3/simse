import { generateId } from '~/lib/db.server';
import { createStripe, verifyWebhookSignature } from '~/lib/stripe.server';
import type { Route } from './+types/api.stripe-webhook';

export async function action({ request, context }: Route.ActionArgs) {
	const env = context.cloudflare.env;
	const stripe = createStripe(env.STRIPE_SECRET_KEY);

	const body = await request.text();
	const signature = request.headers.get('Stripe-Signature');

	if (!signature) {
		return new Response('Missing signature', { status: 400 });
	}

	let event: Awaited<ReturnType<typeof verifyWebhookSignature>>;
	try {
		event = await verifyWebhookSignature(
			stripe,
			body,
			signature,
			env.STRIPE_WEBHOOK_SECRET,
		);
	} catch {
		return new Response('Invalid signature', { status: 400 });
	}

	const db = env.DB;

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

			await db
				.prepare(
					'UPDATE teams SET plan = ?, stripe_subscription_id = ? WHERE stripe_customer_id = ?',
				)
				.bind(plan, sub.id, customerId)
				.run();
			break;
		}

		case 'customer.subscription.deleted': {
			const sub = event.data.object;
			const customerId =
				typeof sub.customer === 'string' ? sub.customer : sub.customer.id;

			await db
				.prepare(
					"UPDATE teams SET plan = 'free', stripe_subscription_id = NULL WHERE stripe_customer_id = ?",
				)
				.bind(customerId)
				.run();
			break;
		}

		case 'invoice.payment_succeeded': {
			const invoice = event.data.object;
			const customerId =
				typeof invoice.customer === 'string'
					? invoice.customer
					: invoice.customer?.id;

			if (customerId) {
				// Find team owner to send notification
				const team = await db
					.prepare(
						"SELECT tm.user_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE t.stripe_customer_id = ? AND tm.role = 'owner' LIMIT 1",
					)
					.bind(customerId)
					.first<{ user_id: string }>();

				if (team) {
					await db
						.prepare(
							"INSERT INTO notifications (id, user_id, type, title, body, link) VALUES (?, ?, 'billing', 'Payment received', ?, '/dashboard/billing')",
						)
						.bind(
							generateId(),
							team.user_id,
							`$${((invoice.amount_paid ?? 0) / 100).toFixed(2)} payment processed.`,
						)
						.run();
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
				const team = await db
					.prepare(
						"SELECT tm.user_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE t.stripe_customer_id = ? AND tm.role = 'owner' LIMIT 1",
					)
					.bind(customerId)
					.first<{ user_id: string }>();

				if (team) {
					await db
						.prepare(
							"INSERT INTO notifications (id, user_id, type, title, body, link) VALUES (?, ?, 'warning', 'Payment failed', 'Your last payment could not be processed. Please update your payment method.', '/dashboard/billing')",
						)
						.bind(generateId(), team.user_id)
						.run();
				}
			}
			break;
		}
	}

	return new Response('ok', { status: 200 });
}
