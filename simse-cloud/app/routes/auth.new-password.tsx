import { Form, Link, redirect, useNavigation } from 'react-router';
import Button from '~/components/ui/Button';
import Input from '~/components/ui/Input';
import { hashPassword } from '~/lib/auth.server';
import { newPasswordSchema } from '~/lib/schemas';
import type { Route } from './+types/auth.new-password';

export async function loader({ request }: Route.LoaderArgs) {
	const url = new URL(request.url);
	const token = url.searchParams.get('token');
	if (!token) throw redirect('/auth/login');
	return { token };
}

export async function action({ request, context }: Route.ActionArgs) {
	const formData = await request.formData();
	const raw = Object.fromEntries(formData);
	const parsed = newPasswordSchema.safeParse(raw);

	if (!parsed.success) {
		const errors: Record<string, string> = {};
		for (const issue of parsed.error.issues) {
			errors[String(issue.path[0])] = issue.message;
		}
		return { errors };
	}

	const db = context.cloudflare.env.DB;
	const token = await db
		.prepare(
			"SELECT id, user_id FROM tokens WHERE code = ? AND type = 'password_reset' AND used = 0 AND expires_at > datetime('now')",
		)
		.bind(parsed.data.token)
		.first<{ id: string; user_id: string }>();

	if (!token) {
		return { errors: { token: 'Invalid or expired reset link' } };
	}

	const passwordHash = await hashPassword(parsed.data.password);

	await db.batch([
		db
			.prepare(
				"UPDATE users SET password_hash = ?, updated_at = datetime('now') WHERE id = ?",
			)
			.bind(passwordHash, token.user_id),
		db.prepare('UPDATE tokens SET used = 1 WHERE id = ?').bind(token.id),
	]);

	return { success: true };
}

export default function NewPassword({
	loaderData,
	actionData,
}: Route.ComponentProps) {
	const navigation = useNavigation();
	const isSubmitting = navigation.state === 'submitting';
	const data = actionData as
		| { errors?: Record<string, string>; success?: boolean }
		| undefined;

	if (data?.success) {
		return (
			<>
				<h1 className="text-2xl font-bold tracking-tight text-white">
					Password <span className="text-emerald-400">updated</span>.
				</h1>
				<p className="mt-4 text-sm leading-relaxed text-zinc-400">
					Your password has been reset. You can now sign in with your new
					password.
				</p>
				<Link
					to="/auth/login"
					className="mt-8 block text-center font-mono text-sm font-bold text-emerald-400 transition-colors hover:text-emerald-300"
				>
					Sign in
				</Link>
			</>
		);
	}

	return (
		<>
			<h1 className="text-2xl font-bold tracking-tight text-white">
				New <span className="text-emerald-400">password</span>.
			</h1>
			<p className="mt-2 text-sm text-zinc-500">
				Choose a new password for your account.
			</p>

			{data?.errors?.token && (
				<div className="mt-4 rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-sm text-red-400">
					{data.errors.token}
				</div>
			)}

			<Form method="post" className="mt-8 space-y-5">
				<input type="hidden" name="token" value={loaderData.token} />
				<Input
					name="password"
					type="password"
					label="New password"
					placeholder="••••••••"
					autoComplete="new-password"
					error={data?.errors?.password}
				/>

				<Button type="submit" className="w-full" loading={isSubmitting}>
					Reset password
				</Button>
			</Form>
		</>
	);
}
