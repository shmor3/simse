import Stripe from 'stripe';

export function createStripe(secretKey: string): Stripe {
	return new Stripe(secretKey);
}

export async function createCheckoutSession(
	stripe: Stripe,
	customerId: string,
	priceId: string,
	appUrl: string,
): Promise<string> {
	const session = await stripe.checkout.sessions.create({
		customer: customerId,
		mode: 'subscription',
		line_items: [{ price: priceId, quantity: 1 }],
		success_url: `${appUrl}/dashboard/billing?success=true`,
		cancel_url: `${appUrl}/dashboard/billing?canceled=true`,
	});
	return session.url!;
}

export async function createBillingPortalSession(
	stripe: Stripe,
	customerId: string,
	appUrl: string,
): Promise<string> {
	const session = await stripe.billingPortal.sessions.create({
		customer: customerId,
		return_url: `${appUrl}/dashboard/billing`,
	});
	return session.url;
}

export async function getOrCreateCustomer(
	stripe: Stripe,
	db: D1Database,
	teamId: string,
	email: string,
	name: string,
): Promise<string> {
	const team = await db
		.prepare('SELECT stripe_customer_id FROM teams WHERE id = ?')
		.bind(teamId)
		.first<{ stripe_customer_id: string | null }>();

	if (team?.stripe_customer_id) return team.stripe_customer_id;

	const customer = await stripe.customers.create({ email, name });
	await db
		.prepare('UPDATE teams SET stripe_customer_id = ? WHERE id = ?')
		.bind(customer.id, teamId)
		.run();

	return customer.id;
}

export async function verifyWebhookSignature(
	stripe: Stripe,
	body: string,
	signature: string,
	secret: string,
): Promise<Stripe.Event> {
	return stripe.webhooks.constructEventAsync(body, signature, secret);
}
