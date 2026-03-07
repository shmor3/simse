import { Link, useRouteLoaderData } from 'react-router';
import RemotesWidget from '~/components/dashboard/RemotesWidget';
import WelcomeHero from '~/components/dashboard/WelcomeHero';
import ActivityItem from '~/components/ui/ActivityItem';
import Card from '~/components/ui/Card';
import EmptyState from '~/components/ui/EmptyState';
import StatCard from '~/components/ui/StatCard';
import type { Route } from './+types/dashboard._index';

export async function loader(_args: Route.LoaderArgs) {
	return {
		stats: {
			sessions: 12,
			sessionsHistory: [2, 4, 3, 5, 4, 6, 8],
			tokens: '48.2k',
			tokensHistory: [12000, 18000, 14000, 22000, 19000, 28000, 32000],
			libraryItems: 37,
			libraryHistory: [28, 30, 31, 33, 34, 36, 37],
			creditBalance: '$24.50',
			creditHistory: [50, 48, 45, 40, 36, 30, 24.5],
		},
		checklist: {
			hasRemote: false,
			hasSession: true,
			hasLibraryItem: true,
			hasTeammate: false,
		},
		recentActivity: [
			{
				id: '1',
				type: 'session' as const,
				title: 'Session completed',
				description: 'Refactored authentication module — 4,200 tokens',
				time: '2 hours ago',
			},
			{
				id: '2',
				type: 'library' as const,
				title: 'Added to library',
				description: 'Saved "API rate limiting patterns" to knowledge base',
				time: '5 hours ago',
			},
			{
				id: '3',
				type: 'remote' as const,
				title: 'Remote disconnected',
				description: 'dev-server-01 went offline',
				time: 'Yesterday',
			},
			{
				id: '4',
				type: 'session' as const,
				title: 'Session started',
				description: 'Working on database migration scripts',
				time: 'Yesterday',
			},
			{
				id: '5',
				type: 'team' as const,
				title: 'Team invite accepted',
				description: 'Alex joined the workspace',
				time: '2 days ago',
			},
		],
	};
}

export default function DashboardIndex({ loaderData }: Route.ComponentProps) {
	const { stats, checklist, recentActivity } = loaderData;

	const dashboardData = useRouteLoaderData('routes/dashboard') as
		| {
				userName: string;
				remotes: Array<{
					id: string;
					name: string;
					status: 'connected' | 'offline';
				}>;
		  }
		| undefined;

	const userName = dashboardData?.userName ?? '';
	const remotes = dashboardData?.remotes ?? [];

	const checklistItems = [
		{
			label: 'Connect a remote machine',
			done: checklist.hasRemote,
			to: '/dashboard/settings/remotes',
		},
		{
			label: 'Start your first session',
			done: checklist.hasSession,
			to: '#',
		},
		{
			label: 'Add to your library',
			done: checklist.hasLibraryItem,
			to: '/dashboard/library',
		},
		{
			label: 'Invite a teammate',
			done: checklist.hasTeammate,
			to: '/dashboard/settings/team/invite',
		},
	];

	return (
		<>
			{/* Welcome hero */}
			<div className="animate-fade-in-up">
				<WelcomeHero userName={userName} checklist={checklistItems} />
			</div>

			{/* Stats grid */}
			<div className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
				<div className="animate-stagger-1">
					<StatCard
						label="Sessions"
						value={stats.sessions}
						sparklineData={stats.sessionsHistory}
						change="33%"
						positive
					/>
				</div>
				<div className="animate-stagger-2">
					<StatCard
						label="Tokens used"
						value={stats.tokens}
						sparklineData={stats.tokensHistory}
						change="14%"
						positive
					/>
				</div>
				<div className="animate-stagger-3">
					<StatCard
						label="Library items"
						value={stats.libraryItems}
						sparklineData={stats.libraryHistory}
						change="3"
						positive
					/>
				</div>
				<div className="animate-stagger-4">
					<StatCard
						label="Credit balance"
						value={stats.creditBalance}
						sparklineData={stats.creditHistory}
					/>
				</div>
			</div>

			{/* Quick actions */}
			<div className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-3 animate-stagger-5">
				<Link to="#" className="group">
					<Card className="card-hover p-5">
						<div className="flex items-start gap-4">
							<div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-emerald-400/10">
								<svg
									className="h-5 w-5 text-emerald-400"
									fill="none"
									viewBox="0 0 24 24"
									stroke="currentColor"
									strokeWidth={2}
								>
									<path
										strokeLinecap="round"
										strokeLinejoin="round"
										d="M12 4v16m8-8H4"
									/>
								</svg>
							</div>
							<div className="min-w-0 flex-1">
								<p className="text-sm font-semibold text-white">New session</p>
								<p className="mt-1 text-[13px] text-zinc-500">
									Start a fresh AI session with your context.
								</p>
							</div>
							<svg
								className="h-4 w-4 shrink-0 self-center text-zinc-700 transition-all group-hover:translate-x-0.5 group-hover:text-zinc-500"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={2}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M9 5l7 7-7 7"
								/>
							</svg>
						</div>
					</Card>
				</Link>
				<Link to="/dashboard/library" className="group">
					<Card className="card-hover p-5">
						<div className="flex items-start gap-4">
							<div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-blue-400/10">
								<svg
									className="h-5 w-5 text-blue-400"
									fill="none"
									viewBox="0 0 24 24"
									stroke="currentColor"
									strokeWidth={2}
								>
									<path
										strokeLinecap="round"
										strokeLinejoin="round"
										d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
									/>
								</svg>
							</div>
							<div className="min-w-0 flex-1">
								<p className="text-sm font-semibold text-white">
									Browse library
								</p>
								<p className="mt-1 text-[13px] text-zinc-500">
									Search and manage your knowledge base.
								</p>
							</div>
							<svg
								className="h-4 w-4 shrink-0 self-center text-zinc-700 transition-all group-hover:translate-x-0.5 group-hover:text-zinc-500"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={2}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M9 5l7 7-7 7"
								/>
							</svg>
						</div>
					</Card>
				</Link>
				<Link to="/dashboard/settings/team/invite" className="group">
					<Card className="card-hover p-5">
						<div className="flex items-start gap-4">
							<div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-violet-400/10">
								<svg
									className="h-5 w-5 text-violet-400"
									fill="none"
									viewBox="0 0 24 24"
									stroke="currentColor"
									strokeWidth={2}
								>
									<path
										strokeLinecap="round"
										strokeLinejoin="round"
										d="M18 9v3m0 0v3m0-3h3m-3 0h-3m-2-5a4 4 0 11-8 0 4 4 0 018 0zM3 20a6 6 0 0112 0v1H3v-1z"
									/>
								</svg>
							</div>
							<div className="min-w-0 flex-1">
								<p className="text-sm font-semibold text-white">
									Invite teammate
								</p>
								<p className="mt-1 text-[13px] text-zinc-500">
									Add someone to your team workspace.
								</p>
							</div>
							<svg
								className="h-4 w-4 shrink-0 self-center text-zinc-700 transition-all group-hover:translate-x-0.5 group-hover:text-zinc-500"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={2}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M9 5l7 7-7 7"
								/>
							</svg>
						</div>
					</Card>
				</Link>
			</div>

			{/* Bottom row: Activity feed + Remotes widget */}
			<div className="mt-8 grid grid-cols-1 gap-6 lg:grid-cols-3 animate-stagger-6">
				{/* Activity feed */}
				<div className="lg:col-span-2">
					<Card className="p-6">
						<div className="flex items-center justify-between">
							<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
								Recent activity
							</p>
							<Link
								to="/dashboard/notifications"
								className="font-mono text-[11px] text-zinc-600 transition-colors hover:text-zinc-400"
							>
								View all
							</Link>
						</div>

						{recentActivity.length === 0 ? (
							<div className="mt-6">
								<EmptyState
									type="activity"
									title="No activity yet"
									description="Your recent sessions, library updates, and team events will show up here."
								/>
							</div>
						) : (
							<div className="mt-6">
								{recentActivity.map((item, i) => (
									<ActivityItem
										key={item.id}
										type={item.type}
										title={item.title}
										description={item.description}
										time={item.time}
										isLast={i === recentActivity.length - 1}
									/>
								))}
							</div>
						)}
					</Card>
				</div>

				{/* Remotes widget */}
				<div>
					<RemotesWidget remotes={remotes} onConnect={() => {}} />
				</div>
			</div>
		</>
	);
}
