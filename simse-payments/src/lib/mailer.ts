export async function sendEmail(
	mailerUrl: string,
	mailerSecret: string,
	to: string,
	subject: string,
	html: string,
): Promise<void> {
	const res = await fetch(`${mailerUrl}/send`, {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${mailerSecret}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({ to, subject, html }),
	});

	if (!res.ok) {
		const body = await res.text();
		console.error(`Mailer error (${res.status}): ${body}`);
	}
}
