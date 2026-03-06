import { Link } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Badge from '~/components/ui/Badge';
import Card from '~/components/ui/Card';
import StatCard from '~/components/ui/StatCard';
import type { Route } from './+types/dashboard._index';

export async function loader(_args: Route.LoaderArgs) {
	// This route inherits the auth check from the dashboard layout loader.
	// For now, return placeholder stats. These will be populated
	// once the user has real usage data.
	return {
		stats: {
			sessions: 0,
			tokens: '0',
			libraryItems: 0,
			creditBalance: '$0.00',
		},
		recentSessions: [] as Array<{
			id: string;
			name: string;
			status: string;
			tokens: number;
			createdAt: string;
		}>,
	};
}

export default function DashboardIndex({ loaderData }: Route.ComponentProps) {
	const { stats, recentSessions } = loaderData;

	return (
		<>
			<PageHeader
				title="Overview"
				description="Your simse workspace at a glance."
			/>

			{/* Stats grid */}
			<div className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
				<StatCard label="Sessions" value={stats.sessions} />
				<StatCard label="Tokens used" value={stats.tokens} />
				<StatCard label="Library items" value={stats.libraryItems} />
				<StatCard label="Credit balance" value={stats.creditBalance} />
			</div>

			{/* Quick actions */}
			<div className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-3">
				<Link to="#" className="group">
					<Card className="p-5 transition-all group-hover:-translate-y-0.5 group-hover:border-zinc-700">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							Quick action
						</p>
						<p className="mt-3 text-sm font-semibold text-white">New session</p>
						<p className="mt-1 text-[13px] text-zinc-500">
							Start a fresh AI session with your context.
						</p>
					</Card>
				</Link>
				<Link to="#" className="group">
					<Card className="p-5 transition-all group-hover:-translate-y-0.5 group-hover:border-zinc-700">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							Quick action
						</p>
						<p className="mt-3 text-sm font-semibold text-white">
							Browse library
						</p>
						<p className="mt-1 text-[13px] text-zinc-500">
							Search and manage your knowledge base.
						</p>
					</Card>
				</Link>
				<Link to="/dashboard/settings/team/invite" className="group">
					<Card className="p-5 transition-all group-hover:-translate-y-0.5 group-hover:border-zinc-700">
						<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
							Quick action
						</p>
						<p className="mt-3 text-sm font-semibold text-white">
							Invite teammate
						</p>
						<p className="mt-1 text-[13px] text-zinc-500">
							Add someone to your team workspace.
						</p>
					</Card>
				</Link>
			</div>

			{/* Recent sessions */}
			<div className="mt-8">
				<h2 className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Recent sessions
				</h2>

				{recentSessions.length === 0 ? (
					<Card className="mt-4 p-10 text-center">
						<div className="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-zinc-800">
							<svg
								className="h-6 w-6 text-zinc-600"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={1.5}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
								/>
							</svg>
						</div>
						<p className="mt-4 text-sm font-medium text-zinc-400">
							No sessions yet
						</p>
						<p className="mt-1 text-[13px] text-zinc-600">
							Start your first session to see activity here.
						</p>
					</Card>
				) : (
					<Card className="mt-4 overflow-hidden">
						<table className="w-full">
							<thead>
								<tr className="border-b border-zinc-800">
									<th className="px-5 py-3 text-left font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">
										Session
									</th>
									<th className="px-5 py-3 text-left font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">
										Status
									</th>
									<th className="px-5 py-3 text-right font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">
										Tokens
									</th>
									<th className="px-5 py-3 text-right font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">
										Date
									</th>
								</tr>
							</thead>
							<tbody>
								{recentSessions.map((s) => (
									<tr
										key={s.id}
										className="border-b border-zinc-800/50 last:border-0"
									>
										<td className="px-5 py-3 text-sm text-zinc-200">
											{s.name}
										</td>
										<td className="px-5 py-3">
											<Badge
												variant={s.status === 'active' ? 'emerald' : 'default'}
											>
												{s.status}
											</Badge>
										</td>
										<td className="px-5 py-3 text-right font-mono text-sm text-zinc-400">
											{s.tokens.toLocaleString()}
										</td>
										<td className="px-5 py-3 text-right text-sm text-zinc-500">
											{s.createdAt}
										</td>
									</tr>
								))}
							</tbody>
						</table>
					</Card>
				)}
			</div>
		</>
	);
}
