import { Form, Link, redirect, useNavigation } from 'react-router';
import Button from '~/components/ui/Button';
import Input from '~/components/ui/Input';
import { type ApiResponse, api } from '~/lib/api.server';
import { setSessionCookie } from '~/lib/session.server';
import type { Route } from './+types/auth.login';

export async function loader({ request }: Route.LoaderArgs) {
	const cookie = request.headers.get('Cookie') ?? '';
	if (cookie.includes('simse_session=')) throw redirect('/dashboard');
	return null;
}

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const raw = Object.fromEntries(formData);

	const res = await api('/auth/login', {
		method: 'POST',
		body: JSON.stringify({ email: raw.email, password: raw.password }),
	});

	const json = (await res.json()) as ApiResponse<{
		token: string;
		requires2fa?: boolean;
		pendingToken?: string;
	}>;

	if (!res.ok) {
		return {
			errors: { email: json.error?.message ?? 'Invalid email or password' },
			values: raw,
		};
	}

	if (json.data?.requires2fa) {
		return redirect('/auth/2fa', {
			headers: {
				'Set-Cookie': `simse_2fa_pending=${json.data.pendingToken}; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=600`,
			},
		});
	}

	return redirect('/dashboard', {
		headers: { 'Set-Cookie': setSessionCookie(json.data!.token) },
	});
}

export default function Login({ actionData }: Route.ComponentProps) {
	const navigation = useNavigation();
	const isSubmitting = navigation.state === 'submitting';
	const errors = actionData?.errors as Record<string, string> | undefined;
	const values = actionData?.values as Record<string, string> | undefined;

	return (
		<>
			<h1 className="text-2xl font-bold tracking-tight text-white">
				Sign <span className="text-emerald-400">in</span>.
			</h1>
			<p className="mt-2 text-sm text-zinc-500">Welcome back to simse.</p>

			<Form method="post" className="mt-8 space-y-5">
				<Input
					name="email"
					type="email"
					label="Email"
					placeholder="you@example.com"
					autoComplete="email"
					defaultValue={values?.email}
					error={errors?.email}
				/>
				<Input
					name="password"
					type="password"
					label="Password"
					placeholder="••••••••"
					autoComplete="current-password"
					error={errors?.password}
				/>

				<div className="flex justify-end">
					<Link
						to="/auth/reset-password"
						className="text-[13px] text-zinc-500 transition-colors hover:text-emerald-400"
					>
						Forgot password?
					</Link>
				</div>

				<Button type="submit" className="w-full" loading={isSubmitting}>
					Sign in
				</Button>
			</Form>

			<p className="mt-6 text-center text-sm text-zinc-600">
				Don't have an account?{' '}
				<Link
					to="/auth/register"
					className="text-emerald-400 transition-colors hover:text-emerald-300"
				>
					Create one
				</Link>
			</p>
		</>
	);
}
