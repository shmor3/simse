interface SendEmailOptions {
	to: string;
	subject: string;
	html: string;
}

export async function sendEmail(
	apiKey: string,
	{ to, subject, html }: SendEmailOptions,
): Promise<void> {
	const res = await fetch('https://api.resend.com/emails', {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${apiKey}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({
			from: 'simse <noreply@simse.dev>',
			to,
			subject,
			html,
		}),
	});

	if (!res.ok) {
		const body = await res.text();
		throw new Error(`Resend API error (${res.status}): ${body}`);
	}
}
