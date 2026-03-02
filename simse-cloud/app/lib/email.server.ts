interface SendEmailOptions {
	template: string;
	to: string;
	props?: Record<string, unknown>;
}

export async function sendEmail(
	env: { EMAIL_API_URL: string; EMAIL_API_SECRET: string },
	{ template, to, props }: SendEmailOptions,
): Promise<void> {
	const res = await fetch(`${env.EMAIL_API_URL}/send`, {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${env.EMAIL_API_SECRET}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({ template, to, props }),
	});

	if (!res.ok) {
		const body = await res.text();
		throw new Error(`Email API error (${res.status}): ${body}`);
	}
}
