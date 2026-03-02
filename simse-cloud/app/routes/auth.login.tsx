import { Form, Link, redirect, useNavigation } from 'react-router';
import Button from '~/components/ui/Button';
import Input from '~/components/ui/Input';
import { createSession, verifyPassword } from '~/lib/auth.server';
import { loginSchema } from '~/lib/schemas';
import { getSession, setSessionCookie } from '~/lib/session.server';
import type { Route } from './+types/auth.login';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (session) throw redirect('/dashboard');
	return null;
}

export async function action({ request, context }: Route.ActionArgs) {
	const formData = await request.formData();
	const raw = Object.fromEntries(formData);
	const parsed = loginSchema.safeParse(raw);

	if (!parsed.success) {
		const errors: Record<string, string> = {};
		for (const issue of parsed.error.issues) {
			const key = String(issue.path[0]);
			errors[key] = issue.message;
		}
		return { errors, values: raw };
	}

	const { email, password } = parsed.data;
	const db = context.cloudflare.env.DB;

	const user = await db
		.prepare(
			'SELECT id, password_hash, two_factor_enabled FROM users WHERE email = ?',
		)
		.bind(email.toLowerCase())
		.first<{ id: string; password_hash: string; two_factor_enabled: number }>();

	if (!user || !(await verifyPassword(password, user.password_hash))) {
		return {
			errors: { email: 'Invalid email or password' },
			values: raw,
		};
	}

	// If 2FA is enabled, redirect to 2FA page
	if (user.two_factor_enabled) {
		// Store pending user ID in a short-lived token
		const tokenId = crypto.randomUUID();
		await db
			.prepare(
				"INSERT INTO tokens (id, user_id, type, code, expires_at) VALUES (?, ?, '2fa', '', datetime('now', '+10 minutes'))",
			)
			.bind(tokenId, user.id)
			.run();

		return redirect('/auth/2fa', {
			headers: {
				'Set-Cookie': `simse_2fa_pending=${tokenId}; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=600`,
			},
		});
	}

	const sessionId = await createSession(db, user.id);

	return redirect('/dashboard', {
		headers: {
			'Set-Cookie': setSessionCookie(sessionId),
		},
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
