import { Form, Link, redirect, useNavigation } from 'react-router';
import Button from '~/components/ui/Button';
import Input from '~/components/ui/Input';
import { api } from '~/lib/api.server';
import { setSessionCookie } from '~/lib/session.server';
import type { Route } from './+types/auth.register';

export async function loader({ request }: Route.LoaderArgs) {
	const cookie = request.headers.get('Cookie') ?? '';
	if (cookie.includes('simse_session=')) throw redirect('/dashboard');
	return null;
}

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const raw = Object.fromEntries(formData);

	const res = await api('/auth/register', {
		method: 'POST',
		body: JSON.stringify({ name: raw.name, email: raw.email, password: raw.password }),
	});

	const json = await res.json() as any;

	if (!res.ok) {
		const message = json.error?.message ?? 'Registration failed';
		const code = json.error?.code;
		if (code === 'EMAIL_EXISTS') {
			return { errors: { email: 'An account with this email already exists' }, values: raw };
		}
		return { errors: { email: message }, values: raw };
	}

	return redirect('/dashboard', {
		headers: { 'Set-Cookie': setSessionCookie(json.data.token) },
	});
}

export default function Register({ actionData }: Route.ComponentProps) {
	const navigation = useNavigation();
	const isSubmitting = navigation.state === 'submitting';
	const errors = actionData?.errors as Record<string, string> | undefined;
	const values = actionData?.values as Record<string, string> | undefined;

	return (
		<>
			<h1 className="text-2xl font-bold tracking-tight text-white">
				Create <span className="text-emerald-400">account</span>.
			</h1>
			<p className="mt-2 text-sm text-zinc-500">Get started with simse.</p>

			<Form method="post" className="mt-8 space-y-5">
				<Input
					name="name"
					label="Name"
					placeholder="Jane Smith"
					autoComplete="name"
					defaultValue={values?.name}
					error={errors?.name}
				/>
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
					autoComplete="new-password"
					error={errors?.password}
				/>

				<Button type="submit" className="w-full" loading={isSubmitting}>
					Create account
				</Button>
			</Form>

			<p className="mt-6 text-center text-sm text-zinc-600">
				Already have an account?{' '}
				<Link
					to="/auth/login"
					className="text-emerald-400 transition-colors hover:text-emerald-300"
				>
					Sign in
				</Link>
			</p>
		</>
	);
}
