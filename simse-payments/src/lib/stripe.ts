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
	return session.url ?? '';
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

export async function verifyWebhookSignature(
	stripe: Stripe,
	body: string,
	signature: string,
	secret: string,
): Promise<Stripe.Event> {
	return stripe.webhooks.constructEventAsync(body, signature, secret);
}
