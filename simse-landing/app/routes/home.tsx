import type { Route } from './+types/home';
import { waitlistSchema } from '~/lib/schema';
import { validateEmail } from '~/lib/validate-email.server';
import Footer from '~/components/Footer';
import Hero from '~/components/Hero';

export function meta(): Route.MetaDescriptors {
	return [
		{ title: 'simse — The assistant that evolves with you' },
	];
}

export async function action({ request, context }: Route.ActionArgs) {
	let body: unknown;
	try {
		body = await request.json();
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

	const db = context.cloudflare.env.DB;

	let shouldEmail = false;
	try {
		const result = await db
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
		const origin = new URL(request.url).origin;
		const unsubscribeUrl = `${origin}/unsubscribe?email=${encodeURIComponent(email)}`;

		// Enqueue welcome email to simse-mailer via Cloudflare Queue
		context.cloudflare.ctx.waitUntil(
			context.cloudflare.env.COMMS_QUEUE.send({
				type: 'email',
				template: 'waitlist-welcome',
				to: email,
				props: { unsubscribeUrl },
			}).catch(() => {}),
		);
	}

	return Response.json({ success: true });
}

export default function Home() {
	return (
		<>
			<Hero />
			<Footer />
		</>
	);
}
