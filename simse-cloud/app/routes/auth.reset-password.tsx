import { Form, Link, useNavigation } from 'react-router';
import Button from '~/components/ui/Button';
import Input from '~/components/ui/Input';
import { generateCode } from '~/lib/auth.server';
import { generateId } from '~/lib/db.server';
import { resetPasswordSchema } from '~/lib/schemas';
import type { Route } from './+types/auth.reset-password';

export async function action({ request, context }: Route.ActionArgs) {
	const formData = await request.formData();
	const raw = Object.fromEntries(formData);
	const parsed = resetPasswordSchema.safeParse(raw);

	if (!parsed.success) {
		const errors: Record<string, string> = {};
		for (const issue of parsed.error.issues) {
			errors[String(issue.path[0])] = issue.message;
		}
		return { errors, values: raw };
	}

	const db = context.cloudflare.env.DB;
	const user = await db
		.prepare('SELECT id FROM users WHERE email = ?')
		.bind(parsed.data.email.toLowerCase())
		.first<{ id: string }>();

	// Always show success to prevent email enumeration
	if (user) {
		const tokenId = generateId();
		const code = generateCode();
		await db
			.prepare(
				"INSERT INTO tokens (id, user_id, type, code, expires_at) VALUES (?, ?, 'password_reset', ?, datetime('now', '+1 hour'))",
			)
			.bind(tokenId, user.id, code)
			.run();

		// TODO: Send reset password email via Resend with resetUrl
	}

	return { success: true };
}

export default function ResetPassword({ actionData }: Route.ComponentProps) {
	const navigation = useNavigation();
	const isSubmitting = navigation.state === 'submitting';
	const data = actionData as
		| {
				errors?: Record<string, string>;
				values?: Record<string, string>;
				success?: boolean;
		  }
		| undefined;

	if (data?.success) {
		return (
			<>
				<h1 className="text-2xl font-bold tracking-tight text-white">
					Check your <span className="text-emerald-400">email</span>.
				</h1>
				<p className="mt-4 text-sm leading-relaxed text-zinc-400">
					If an account exists with that email, we've sent a link to reset your
					password. The link expires in 1 hour.
				</p>
				<Link
					to="/auth/login"
					className="mt-8 block text-center text-sm text-emerald-400 transition-colors hover:text-emerald-300"
				>
					Back to sign in
				</Link>
			</>
		);
	}

	return (
		<>
			<h1 className="text-2xl font-bold tracking-tight text-white">
				Reset <span className="text-emerald-400">password</span>.
			</h1>
			<p className="mt-2 text-sm text-zinc-500">
				We'll email you a link to reset your password.
			</p>

			<Form method="post" className="mt-8 space-y-5">
				<Input
					name="email"
					type="email"
					label="Email"
					placeholder="you@example.com"
					autoComplete="email"
					defaultValue={data?.values?.email}
					error={data?.errors?.email}
				/>

				<Button type="submit" className="w-full" loading={isSubmitting}>
					Send reset link
				</Button>
			</Form>

			<Link
				to="/auth/login"
				className="mt-6 block text-center text-sm text-zinc-600 transition-colors hover:text-zinc-400"
			>
				Back to sign in
			</Link>
		</>
	);
}
