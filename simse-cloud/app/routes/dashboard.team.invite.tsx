import { Form, Link, redirect, useNavigation } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import Input from '~/components/ui/Input';
import { generateId } from '~/lib/db.server';
import { inviteSchema } from '~/lib/schemas';
import { getSession } from '~/lib/session.server';
import type { Route } from './+types/dashboard.team.invite';

export async function action({ request, context }: Route.ActionArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) throw redirect('/auth/login');

	const formData = await request.formData();
	const raw = Object.fromEntries(formData);
	const parsed = inviteSchema.safeParse(raw);

	if (!parsed.success) {
		const errors: Record<string, string> = {};
		for (const issue of parsed.error.issues) {
			errors[String(issue.path[0])] = issue.message;
		}
		return { errors, values: raw };
	}

	const db = context.cloudflare.env.DB;
	const { email, role } = parsed.data;

	// Get the user's team
	const team = await db
		.prepare(
			"SELECT t.id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND (tm.role = 'owner' OR tm.role = 'admin') LIMIT 1",
		)
		.bind(session.userId)
		.first<{ id: string }>();

	if (!team) {
		return {
			errors: { email: 'You do not have permission to invite members' },
		};
	}

	// Check if already a member
	const existing = await db
		.prepare(
			'SELECT 1 FROM team_members tm JOIN users u ON tm.user_id = u.id WHERE tm.team_id = ? AND u.email = ?',
		)
		.bind(team.id, email.toLowerCase())
		.first();

	if (existing) {
		return {
			errors: { email: 'This person is already a team member' },
			values: raw,
		};
	}

	// Create invite
	const inviteId = generateId();
	await db
		.prepare(
			"INSERT INTO team_invites (id, team_id, email, role, invited_by, expires_at) VALUES (?, ?, ?, ?, ?, datetime('now', '+7 days'))",
		)
		.bind(inviteId, team.id, email.toLowerCase(), role, session.userId)
		.run();

	// TODO: Send team invite email via email API

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
