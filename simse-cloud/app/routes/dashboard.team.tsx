import { Form, Link } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Avatar from '~/components/ui/Avatar';
import Badge from '~/components/ui/Badge';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard.team';

export async function loader({ request }: Route.LoaderArgs) {
	try {
		const res = await authenticatedApi(request, '/teams/me');
		if (!res.ok) return { team: null, members: [], invites: [] };

		type Member = {
			id: string;
			name: string;
			email: string;
			role: string;
			joined_at: string;
		};
		type Invite = {
			id: string;
			email: string;
			role: string;
			created_at: string;
		};
		const json = (await res.json()) as ApiResponse<{
			id: string;
			name: string;
			plan: string;
			members: Member[];
			invites: Invite[];
		}>;
		const team = json.data;

		// Determine current user's role by looking at the session token
		// We get this from the /auth/me endpoint
		const meRes = await authenticatedApi(request, '/auth/me');
		const meJson = (await meRes.json()) as ApiResponse<{ id: string }>;
		const userId = meJson.data?.id;
		const currentMember = team?.members.find((m) => m.id === userId);

		return {
			team: team ? { id: team.id, name: team.name, plan: team.plan } : null,
			members: team?.members ?? [],
			invites: team?.invites ?? [],
			myRole: currentMember?.role ?? 'member',
		};
	} catch {
		return { team: null, members: [], invites: [] };
	}
}

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const intent = formData.get('intent');

	if (intent === 'change-role') {
		const memberId = formData.get('memberId') as string;
		const newRole = formData.get('role') as string;

		if (!['admin', 'member'].includes(newRole)) return null;

		await authenticatedApi(request, `/teams/me/members/${memberId}/role`, {
			method: 'PUT',
			body: JSON.stringify({ role: newRole }),
		});
	}

	if (intent === 'revoke-invite') {
		const inviteId = formData.get('inviteId') as string;
		await authenticatedApi(request, `/teams/me/invites/${inviteId}`, {
			method: 'DELETE',
		});
	}

	return null;
}

const roleBadge = (role: string) => {
	switch (role) {
		case 'owner':
			return <Badge variant="emerald">Owner</Badge>;
		case 'admin':
			return <Badge variant="info">Admin</Badge>;
		default:
			return <Badge>Member</Badge>;
	}
};

export default function Team({ loaderData }: Route.ComponentProps) {
	const { team, members, invites, myRole } = loaderData as {
		team: { id: string; name: string; plan: string } | null;
		members: Array<{
			id: string;
			name: string;
			email: string;
			role: string;
			joined_at: string;
		}>;
		invites: Array<{
			id: string;
			email: string;
			role: string;
			created_at: string;
		}>;
		myRole: string;
	};
	const canManage = myRole === 'owner' || myRole === 'admin';

	return (
		<>
			<PageHeader
				title="Team"
				description={
					team ? `${team.name} \u2014 ${team.plan} plan` : 'Manage your team'
				}
				action={
					canManage ? (
						<div className="flex gap-3">
							<Link to="/dashboard/team/plans">
								<Button variant="ghost">Plans</Button>
							</Link>
							<Link to="/dashboard/team/invite">
								<Button>Invite member</Button>
							</Link>
						</div>
					) : undefined
				}
			/>

			{/* Members */}
			<Card className="mt-8 overflow-hidden">
				<div className="border-b border-zinc-800 px-6 py-4">
					<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
						Members ({members.length})
					</p>
				</div>
				<div className="divide-y divide-zinc-800/50">
					{members.map((m) => (
						<div
							key={m.id}
							className="flex items-center justify-between px-6 py-4"
						>
							<div className="flex items-center gap-3">
								<Avatar name={m.name} />
								<div>
									<p className="text-sm font-medium text-white">{m.name}</p>
									<p className="text-[13px] text-zinc-500">{m.email}</p>
								</div>
							</div>
							<div className="flex items-center gap-3">
								{roleBadge(m.role)}
								{canManage && m.role !== 'owner' && (
									<Form method="post">
										<input type="hidden" name="intent" value="change-role" />
										<input type="hidden" name="teamId" value={team?.id} />
										<input type="hidden" name="memberId" value={m.id} />
										<input
											type="hidden"
											name="role"
											value={m.role === 'admin' ? 'member' : 'admin'}
										/>
										<Button
											variant="ghost"
											type="submit"
											className="text-[12px]"
										>
											{m.role === 'admin' ? 'Demote' : 'Promote'}
										</Button>
									</Form>
								)}
							</div>
						</div>
					))}
				</div>
			</Card>

			{/* Pending invites */}
			{invites.length > 0 && (
				<Card className="mt-6 overflow-hidden">
					<div className="border-b border-zinc-800 px-6 py-4">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							Pending invites ({invites.length})
						</p>
					</div>
					<div className="divide-y divide-zinc-800/50">
						{invites.map((inv) => (
							<div
								key={inv.id}
								className="flex items-center justify-between px-6 py-4"
							>
								<div>
									<p className="text-sm text-zinc-300">{inv.email}</p>
									<p className="text-[12px] text-zinc-600">
										Invited {new Date(inv.created_at).toLocaleDateString()}
									</p>
								</div>
								<div className="flex items-center gap-3">
									<Badge>{inv.role}</Badge>
									{canManage && (
										<Form method="post">
											<input
												type="hidden"
												name="intent"
												value="revoke-invite"
											/>
											<input type="hidden" name="inviteId" value={inv.id} />
											<Button
												variant="danger"
												type="submit"
												className="text-[12px]"
											>
												Revoke
											</Button>
										</Form>
									)}
								</div>
							</div>
						))}
					</div>
				</Card>
			)}
		</>
	);
}
