import { Form, Link, redirect, useNavigation } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import Input from '~/components/ui/Input';
import { authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard.team.invite';

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const raw = Object.fromEntries(formData);

	const email = raw.email as string;
	const role = raw.role as string;

	if (!email) {
		return { errors: { email: 'Email is required' }, values: raw };
	}

	const res = await authenticatedApi(request, '/teams/me/invite', {
		method: 'POST',
		body: JSON.stringify({ email, role }),
	});

	if (!res.ok) {
		const json = await res.json() as any;
		const message = json.error?.message ?? 'Failed to send invite';
		return { errors: { email: message }, values: raw };
	}

	return redirect('/dashboard/team');
}

export default function TeamInvite({ actionData }: Route.ComponentProps) {
	const navigation = useNavigation();
	const isSubmitting = navigation.state === 'submitting';
	const data = actionData as
		| { errors?: Record<string, string>; values?: Record<string, string> }
		| undefined;

	return (
		<>
			<PageHeader
				title="Invite member"
				description="Send an invitation to join your team."
				action={
					<Link to="/dashboard/team">
						<Button variant="ghost">Back</Button>
					</Link>
				}
			/>

			<Card className="mt-8 p-6">
				<Form method="post" className="space-y-5">
					<Input
						name="email"
						type="email"
						label="Email"
						placeholder="colleague@company.com"
						defaultValue={data?.values?.email}
						error={data?.errors?.email}
					/>

					<div className="space-y-1.5">
						<label
							htmlFor="role"
							className="block font-mono text-[11px] font-bold uppercase tracking-[0.15em] text-zinc-500"
						>
							Role
						</label>
						<select
							id="role"
							name="role"
							defaultValue={data?.values?.role ?? 'member'}
							className="w-full rounded-lg border border-zinc-800 bg-zinc-900 px-3 py-2.5 text-sm text-zinc-100 transition-colors hover:border-zinc-700 focus:border-emerald-400/50 focus:outline-none focus:ring-1 focus:ring-emerald-400/25"
						>
							<option value="member">Member</option>
							<option value="admin">Admin</option>
						</select>
					</div>

					<Button type="submit" className="w-full" loading={isSubmitting}>
						Send invite
					</Button>
				</Form>
			</Card>
		</>
	);
}
