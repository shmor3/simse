import { waitlistSchema } from '../../src/lib/schema';
import { validateEmail } from '../lib/validate-email';
import { sendWelcomeEmail } from '../lib/welcome-email';

interface Env {
	simse_waitlist: D1Database; // matches binding name in wrangler.toml
	RESEND_API_KEY: string;
	FROM_EMAIL: string;
}

export const onRequestPost: PagesFunction<Env> = async (context) => {
	let body: unknown;
	try {
		body = await context.request.json();
	} catch {
		return Response.json({ error: 'Invalid JSON' }, { status: 400 });
	}

	const parsed = waitlistSchema.safeParse(body);
	if (!parsed.success) {
		return Response.json(
			{ error: parsed.error.issues[0].message },
			{ status: 400 },
		);
	}

	const email = parsed.data.email.trim().toLowerCase();

	const validation = await validateEmail(email);
	if (!validation.valid) {
		return Response.json({ error: validation.reason }, { status: 422 });
	}

	let shouldEmail = false;
	try {
		// Insert new row, or re-subscribe if previously unsubscribed with a 24h cooldown.
		// The WHERE clause prevents re-subscribing within 24h of unsubscribing,
		// blocking subscribe/unsubscribe cycling that wastes emails.
		const result = await context.env.simse_waitlist
			.prepare(
				`INSERT INTO waitlist (email, subscribed, updated_at) VALUES (?, 1, datetime('now'))
				ON CONFLICT (email) DO UPDATE SET subscribed = 1, updated_at = datetime('now')
				WHERE subscribed = 0 AND updated_at < datetime('now', '-1 day')`,
			)
			.bind(email)
			.run();
		shouldEmail = (result.meta?.changes ?? 0) > 0;
	} catch (err) {
		console.error('D1 insert failed', err);
		return Response.json({ error: 'Database error' }, { status: 500 });
	}

	if (shouldEmail) {
		const origin = new URL(context.request.url).origin;
		const unsubscribeUrl = `${origin}/unsubscribe?email=${encodeURIComponent(email)}`;

		context.waitUntil(
			sendWelcomeEmail(
				email,
				context.env.RESEND_API_KEY,
				context.env.FROM_EMAIL,
				unsubscribeUrl,
			).catch(() => {}),
		);
	}

	// Always return success to avoid leaking whether the email is already on the list
	return Response.json({ success: true });
};
