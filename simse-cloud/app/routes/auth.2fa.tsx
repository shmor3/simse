import { Form, redirect, useNavigation } from 'react-router';
import Button from '~/components/ui/Button';
import CodeInput from '~/components/ui/CodeInput';
import { createSession } from '~/lib/auth.server';
import { twoFactorSchema } from '~/lib/schemas';
import { setSessionCookie } from '~/lib/session.server';
import type { Route } from './+types/auth.2fa';

export async function loader({ request }: Route.LoaderArgs) {
	const cookie = request.headers.get('Cookie') ?? '';
	const match = cookie.match(/simse_2fa_pending=([^;]+)/);
	if (!match) throw redirect('/auth/login');
	return null;
}

export async function action({ request, context }: Route.ActionArgs) {
	const cookie = request.headers.get('Cookie') ?? '';
	const match = cookie.match(/simse_2fa_pending=([^;]+)/);
	if (!match) throw redirect('/auth/login');

	const tokenId = match[1];
	const db = context.cloudflare.env.DB;

	const token = await db
		.prepare(
			"SELECT user_id FROM tokens WHERE id = ? AND type = '2fa' AND used = 0 AND expires_at > datetime('now')",
		)
		.bind(tokenId)
		.first<{ user_id: string }>();

	if (!token) throw redirect('/auth/login');

	const formData = await request.formData();
	const raw = Object.fromEntries(formData);
	const parsed = twoFactorSchema.safeParse(raw);

	if (!parsed.success) {
		return { error: 'Please enter a valid 6-digit code' };
	}

	// Verify the code against stored TOTP secret
	// For now, verify against a code token we'd have sent via email
	const codeToken = await db
		.prepare(
			"SELECT id FROM tokens WHERE user_id = ? AND type = '2fa' AND code = ? AND used = 0 AND expires_at > datetime('now')",
		)
		.bind(token.user_id, parsed.data.code)
		.first<{ id: string }>();

	if (!codeToken) {
		return { error: 'Invalid or expired code' };
	}

	// Mark tokens as used
	await db.batch([
		db.prepare('UPDATE tokens SET used = 1 WHERE id = ?').bind(tokenId),
		db.prepare('UPDATE tokens SET used = 1 WHERE id = ?').bind(codeToken.id),
	]);

	const sessionId = await createSession(db, token.user_id);

	return redirect('/dashboard', {
		headers: {
			'Set-Cookie': setSessionCookie(sessionId),
		},
	});
}

export default function TwoFactor({ actionData }: Route.ComponentProps) {
	const navigation = useNavigation();
	const isSubmitting = navigation.state === 'submitting';
	const error = (actionData as { error?: string } | undefined)?.error;

	return (
		<>
			<h1 className="text-2xl font-bold tracking-tight text-white">
				Two-factor <span className="text-emerald-400">auth</span>.
			</h1>
			<p className="mt-2 text-sm text-zinc-500">
				Enter the 6-digit code from your authenticator app.
			</p>

			<Form method="post" className="mt-8 space-y-6">
				<CodeInput name="code" error={error} />

				<Button type="submit" className="w-full" loading={isSubmitting}>
					Verify
				</Button>
			</Form>

			<p className="mt-6 text-center text-[13px] text-zinc-600">
				Didn't receive a code? Check your authenticator app or{' '}
				<a
					href="/auth/login"
					className="text-emerald-400 transition-colors hover:text-emerald-300"
				>
					try again
				</a>
			</p>
		</>
	);
}
