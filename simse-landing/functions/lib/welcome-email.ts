import { render } from '@react-email/render';
import WelcomeEmail from '../emails/welcome';

export async function sendWelcomeEmail(
	email: string,
	apiKey: string,
	from: string,
	unsubscribeUrl: string,
): Promise<void> {
	const html = await render(WelcomeEmail({ unsubscribeUrl }));

	await fetch('https://api.resend.com/emails', {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${apiKey}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({
			from,
			to: email,
			subject: "You're on the simse waitlist",
			html,
			headers: {
				'List-Unsubscribe': `<${unsubscribeUrl}>`,
			},
		}),
	});
}
