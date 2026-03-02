import { waitlistSchema } from '../../src/lib/schema';
import { validateEmail } from '../lib/validate-email';
import { sendWelcomeEmail } from '../lib/welcome-email';

interface Env {
	simse_waitlist: D1Database;   // matches binding name in wrangler.toml
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

	try {
		await context.env.simse_waitlist.prepare(
			'INSERT INTO waitlist (email) VALUES (?) ON CONFLICT (email) DO NOTHING',
		)
			.bind(email)
			.run();
	} catch (err) {
		// log full error for debugging; keep response generic to avoid info leak
		console.error('D1 insert failed', err);
		return Response.json({ error: 'Database error' }, { status: 500 });
	}

	// Build an unsubscribe link and fire-and-forget the email send
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

	return Response.json({ success: true });
};
