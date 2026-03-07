import { Form, redirect, useNavigation } from 'react-router';
import Button from '~/components/ui/Button';
import CodeInput from '~/components/ui/CodeInput';
import { type ApiResponse, api } from '~/lib/api.server';
import { setSessionCookie } from '~/lib/session.server';
import type { Route } from './+types/auth.2fa';

export async function loader({ request }: Route.LoaderArgs) {
	const cookie = request.headers.get('Cookie') ?? '';
	const match = cookie.match(/simse_2fa_pending=([^;]+)/);
	if (!match) throw redirect('/auth/login');
	return null;
}

export async function action({ request }: Route.ActionArgs) {
	const cookie = request.headers.get('Cookie') ?? '';
	const match = cookie.match(/simse_2fa_pending=([^;]+)/);
	if (!match) throw redirect('/auth/login');

	const pendingToken = match[1];
	const formData = await request.formData();
	const code = formData.get('code') as string;

	if (!code || code.length !== 6) {
		return { error: 'Please enter a valid 6-digit code' };
	}

	const res = await api('/auth/2fa', {
		method: 'POST',
		body: JSON.stringify({ code, pendingToken }),
	});

	const json = (await res.json()) as ApiResponse<{ token: string }>;

	if (!res.ok) {
		return { error: json.error?.message ?? 'Invalid or expired code' };
	}

	return redirect('/dashboard', {
		headers: { 'Set-Cookie': setSessionCookie(json.data!.token) },
	});
}

export default function TwoFactor({ actionData }: Route.ComponentProps) {
	const navigation = useNavigation();
	const isSubmitting = navigation.state === 'submitting';
	const error = (actionData as { error?: string } | undefined)?.error;

	return (
		<div className="animate-fade-in">
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
		</div>
	);
}
