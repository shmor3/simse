import { Form, Link, redirect, useNavigation } from 'react-router';
import Button from '~/components/ui/Button';
import Input from '~/components/ui/Input';
import { createSession, generateCode, hashPassword } from '~/lib/auth.server';
import { generateId } from '~/lib/db.server';
import { registerSchema } from '~/lib/schemas';
import { getSession, setSessionCookie } from '~/lib/session.server';
import type { Route } from './+types/auth.register';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (session) throw redirect('/dashboard');
	return null;
}

export async function action({ request, context }: Route.ActionArgs) {
	const formData = await request.formData();
	const raw = Object.fromEntries(formData);
	const parsed = registerSchema.safeParse(raw);

	if (!parsed.success) {
		const errors: Record<string, string> = {};
		for (const issue of parsed.error.issues) {
			const key = String(issue.path[0]);
			errors[key] = issue.message;
		}
		return { errors, values: raw };
	}

	const { name, email, password } = parsed.data;
	const db = context.cloudflare.env.DB;

	// Check if email already exists
	const existing = await db
		.prepare('SELECT id FROM users WHERE email = ?')
		.bind(email.toLowerCase())
		.first();

	if (existing) {
		return {
			errors: { email: 'An account with this email already exists' },
			values: raw,
		};
	}

	const userId = generateId();
	const passwordHash = await hashPassword(password);

	await db
		.prepare(
			'INSERT INTO users (id, email, name, password_hash) VALUES (?, ?, ?, ?)',
		)
		.bind(userId, email.toLowerCase(), name, passwordHash)
		.run();

	// Create a default team for the user
	const teamId = generateId();
	await db.batch([
		db
			.prepare('INSERT INTO teams (id, name) VALUES (?, ?)')
			.bind(teamId, `${name}'s Team`),
		db
			.prepare(
				"INSERT INTO team_members (team_id, user_id, role) VALUES (?, ?, 'owner')",
			)
			.bind(teamId, userId),
	]);

	// Create email verification token
	const code = generateCode();
	const tokenId = generateId();
	await db
		.prepare(
			"INSERT INTO tokens (id, user_id, type, code, expires_at) VALUES (?, ?, 'email_verify', ?, datetime('now', '+15 minutes'))",
		)
		.bind(tokenId, userId, code)
		.run();

	// TODO: Send verification email via email API (Task 6)

	const sessionId = await createSession(db, userId);

	return redirect('/dashboard', {
		headers: {
			'Set-Cookie': setSessionCookie(sessionId),
		},
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
