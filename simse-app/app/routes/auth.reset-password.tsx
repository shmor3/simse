import { Form, Link, useNavigation } from 'react-router';
import Button from '~/components/ui/Button';
import Input from '~/components/ui/Input';
import { api } from '~/lib/api.server';
import type { Route } from './+types/auth.reset-password';

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const email = formData.get('email') as string;

	if (!email) {
		return { errors: { email: 'Email is required' } };
	}

	await api('/auth/reset-password', {
		method: 'POST',
		body: JSON.stringify({ email }),
	});

	// Always show success to prevent email enumeration
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
			<div className="animate-fade-in">
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
			</div>
		);
	}

	return (
		<div className="animate-fade-in">
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
		</div>
	);
}
