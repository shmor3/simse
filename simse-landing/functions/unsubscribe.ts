interface Env {
	simse_waitlist: D1Database;
}

const html = (message: string, success: boolean) => `<!DOCTYPE html>
<html lang="en">
<head>
	<meta charset="utf-8" />
	<meta name="viewport" content="width=device-width, initial-scale=1" />
	<title>Unsubscribe – simse</title>
	<style>
		* { margin: 0; padding: 0; box-sizing: border-box; }
		body {
			font-family: system-ui, -apple-system, sans-serif;
			background: #0a0a0b;
			color: #a1a1aa;
			display: flex;
			align-items: center;
			justify-content: center;
			min-height: 100vh;
			padding: 2rem;
		}
		.card {
			text-align: center;
			max-width: 420px;
		}
		.icon {
			font-size: 2rem;
			margin-bottom: 1rem;
		}
		h1 {
			font-size: 1.25rem;
			font-weight: 600;
			color: ${success ? '#34d399' : '#f87171'};
			margin-bottom: 0.75rem;
		}
		p {
			font-size: 0.875rem;
			line-height: 1.6;
			color: #71717a;
		}
		a {
			color: #a1a1aa;
			text-decoration: underline;
			text-underline-offset: 2px;
		}
		a:hover { color: #d4d4d8; }
	</style>
</head>
<body>
	<div class="card">
		<div class="icon">${success ? '&#10003;' : '&#10007;'}</div>
		<h1>${message}</h1>
		<p>
			${success ? "You've been removed from our mailing list and won't receive any more emails from us." : 'Please try again or contact us if the issue persists.'}
		</p>
		<p style="margin-top: 1.5rem;">
			<a href="/">Back to simse.dev</a>
		</p>
	</div>
</body>
</html>`;

export const onRequestGet: PagesFunction<Env> = async (context) => {
	const url = new URL(context.request.url);
	const email = url.searchParams.get('email')?.trim().toLowerCase();

	if (!email) {
		return new Response(html('Invalid unsubscribe link', false), {
			status: 400,
			headers: { 'Content-Type': 'text/html;charset=utf-8' },
		});
	}

	try {
		await context.env.simse_waitlist
			.prepare(
				"UPDATE waitlist SET subscribed = 0, updated_at = datetime('now') WHERE email = ? AND subscribed = 1",
			)
			.bind(email)
			.run();
	} catch (err) {
		console.error('D1 delete failed', err);
		return new Response(html('Something went wrong', false), {
			status: 500,
			headers: { 'Content-Type': 'text/html;charset=utf-8' },
		});
	}

	return new Response(html('Unsubscribed', true), {
		status: 200,
		headers: { 'Content-Type': 'text/html;charset=utf-8' },
	});
};
