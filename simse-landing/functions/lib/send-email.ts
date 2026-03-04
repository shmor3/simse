import { render } from '@react-email/render';
import EarlyPreviewEmail from '../emails/early-preview';
import InviteEmail from '../emails/invite';
import WelcomeEmail from '../emails/welcome';

const FROM = 'Simse <hello@simse.dev>';

interface SendOptions {
	to: string;
	apiKey: string;
	subject: string;
	html: string;
	unsubscribeUrl: string;
}

async function send({
	to,
	apiKey,
	subject,
	html,
	unsubscribeUrl,
}: SendOptions) {
	const res = await fetch('https://api.resend.com/emails', {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${apiKey}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({
			from: FROM,
			to,
			subject,
			html,
			headers: {
				'List-Unsubscribe': `<${unsubscribeUrl}>`,
			},
		}),
	});

	if (!res.ok) {
		const body = await res.text().catch(() => 'unknown');
		throw new Error(`Resend API error ${res.status}: ${body}`);
	}
}

export async function sendWelcomeEmail(
	email: string,
	apiKey: string,
	unsubscribeUrl: string,
) {
	const html = await render(WelcomeEmail({ unsubscribeUrl }));
	await send({
		to: email,
		apiKey,
		subject: "You're on the simse waitlist",
		html,
		unsubscribeUrl,
	});
}

export async function sendEarlyPreviewEmail(
	email: string,
	apiKey: string,
	previewUrl: string,
	unsubscribeUrl: string,
) {
	const html = await render(EarlyPreviewEmail({ previewUrl, unsubscribeUrl }));
	await send({
		to: email,
		apiKey,
		subject: 'Your early preview of simse is ready',
		html,
		unsubscribeUrl,
	});
}

export async function sendInviteEmail(
	email: string,
	apiKey: string,
	inviteUrl: string,
	unsubscribeUrl: string,
) {
	const html = await render(InviteEmail({ inviteUrl, unsubscribeUrl }));
	await send({
		to: email,
		apiKey,
		subject: 'Your simse early access is ready',
		html,
		unsubscribeUrl,
	});
}
