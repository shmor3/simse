import { validateEmail } from '../lib/validate-email';
import { sendWelcomeEmail } from '../lib/welcome-email';

interface Env {
	DB: D1Database;
	RESEND_API_KEY: string;
	FROM_EMAIL: string;
	UNSUBSCRIBE_URL: string;
}

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export const onRequestPost: PagesFunction<Env> = async (context) => {
	let body: { email?: string };
	try {
		body = await context.request.json();
	} catch {
		return Response.json({ error: 'Invalid JSON' }, { status: 400 });
	}

	const email = body.email?.trim().toLowerCase();
	if (!email || !EMAIL_RE.test(email)) {
		return Response.json({ error: 'Invalid email' }, { status: 400 });
	}

	const validation = await validateEmail(email);
	if (!validation.valid) {
		return Response.json({ error: validation.reason }, { status: 422 });
	}

	try {
		await context.env.DB.prepare(
			'INSERT INTO waitlist (email) VALUES (?) ON CONFLICT (email) DO NOTHING',
		)
			.bind(email)
			.run();
	} catch (err) {
		return Response.json({ error: 'Database error' }, { status: 500 });
	}

	// Fire-and-forget: don't block signup on email delivery
	context.waitUntil(
		sendWelcomeEmail(
			email,
			context.env.RESEND_API_KEY,
			context.env.FROM_EMAIL,
			context.env.UNSUBSCRIBE_URL,
		).catch(() => {}),
	);

	return Response.json({ success: true });
};
