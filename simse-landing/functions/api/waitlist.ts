interface Env {
	DB: D1Database;
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

	try {
		await context.env.DB.prepare(
			'INSERT INTO waitlist (email) VALUES (?) ON CONFLICT (email) DO NOTHING',
		)
			.bind(email)
			.run();
	} catch (err) {
		return Response.json({ error: 'Database error' }, { status: 500 });
	}

	return Response.json({ success: true });
};
