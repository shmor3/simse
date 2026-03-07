import { Form } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';

import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import EmptyState from '~/components/ui/EmptyState';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';

function relativeTime(dateStr: string): string {
	const now = Date.now();
	const then = new Date(dateStr).getTime();
	const diff = Math.max(0, now - then);
	const mins = Math.floor(diff / 60000);
	if (mins < 1) return 'just now';
	if (mins < 60) return `${mins}m ago`;
	const hrs = Math.floor(mins / 60);
	if (hrs < 24) return `${hrs}h ago`;
	const days = Math.floor(hrs / 24);
	if (days < 7) return `${days}d ago`;
	return new Date(dateStr).toLocaleDateString();
}

import type { Route } from './+types/dashboard.notifications';

export async function loader({ request }: Route.LoaderArgs) {
	try {
		const res = await authenticatedApi(request, '/notifications');
		if (!res.ok) return { notifications: [] };

		type Notif = {
			id: string;
			type: string;
			title: string;
			body: string;
			read: boolean;
			created_at: string;
			link?: string;
		};
		const json = (await res.json()) as ApiResponse<Notif[]>;
		return { notifications: json.data ?? [] };
	} catch {
		return { notifications: [] };
	}
}

export async function action({ request }: Route.ActionArgs) {
	const formData = await request.formData();
	const intent = formData.get('intent');

	if (intent === 'mark-read') {
		const notifId = formData.get('id') as string;
		await authenticatedApi(request, `/notifications/${notifId}/read`, {
			method: 'PUT',
		});
	}

	if (intent === 'mark-all-read') {
		await authenticatedApi(request, '/notifications/read-all', {
			method: 'PUT',
		});
	}

	return null;
}

const typeIcon = (type: string) => {
	switch (type) {
		case 'success':
			return (
				<div className="flex h-8 w-8 items-center justify-center rounded-full bg-emerald-400/10 text-emerald-400">
					<svg
						className="h-4 w-4"
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
						strokeWidth={2}
					>
						<path
							strokeLinecap="round"
							strokeLinejoin="round"
							d="M5 13l4 4L19 7"
						/>
					</svg>
				</div>
			);
		case 'warning':
			return (
				<div className="flex h-8 w-8 items-center justify-center rounded-full bg-amber-400/10 text-amber-400">
					<svg
						className="h-4 w-4"
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
						strokeWidth={2}
					>
						<path
							strokeLinecap="round"
							strokeLinejoin="round"
							d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
						/>
					</svg>
				</div>
			);
		case 'billing':
			return (
				<div className="flex h-8 w-8 items-center justify-center rounded-full bg-blue-400/10 text-blue-400">
					<svg
						className="h-4 w-4"
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
						strokeWidth={2}
					>
						<path
							strokeLinecap="round"
							strokeLinejoin="round"
							d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z"
						/>
					</svg>
				</div>
			);
		default:
			return (
				<div className="flex h-8 w-8 items-center justify-center rounded-full bg-zinc-800 text-zinc-400">
					<svg
						className="h-4 w-4"
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
						strokeWidth={2}
					>
						<path
							strokeLinecap="round"
							strokeLinejoin="round"
							d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
						/>
					</svg>
				</div>
			);
	}
};

export default function Notifications({ loaderData }: Route.ComponentProps) {
	const { notifications } = loaderData;
	const unreadCount = notifications.filter((n) => !n.read).length;

	return (
		<>
			<PageHeader
				title="Notifications"
				description={
					unreadCount > 0
						? `${unreadCount} unread notification${unreadCount === 1 ? '' : 's'}`
						: 'All caught up'
				}
				action={
					unreadCount > 0 ? (
						<Form method="post">
							<input type="hidden" name="intent" value="mark-all-read" />
							<Button variant="ghost" type="submit">
								Mark all read
							</Button>
						</Form>
					) : undefined
				}
			/>

			{notifications.length === 0 ? (
				<div className="mt-8 animate-fade-in-up">
					<EmptyState
						type="notifications"
						title="All caught up"
						description="We'll let you know when something important happens."
					/>
				</div>
			) : (
				<Card className="mt-8 overflow-hidden animate-fade-in-up">
					<div className="divide-y divide-zinc-800/50">
						{notifications.map((n) => (
							<div
								key={n.id}
								className={`flex items-start gap-4 px-6 py-4 transition-colors hover:bg-zinc-800/20 ${!n.read ? 'bg-zinc-800/15' : ''}`}
							>
								{typeIcon(n.type)}
								<div className="min-w-0 flex-1">
									<div className="flex items-center gap-2">
										<p className="text-sm font-medium text-white">{n.title}</p>
										{!n.read && (
											<span className="h-1.5 w-1.5 rounded-full bg-emerald-400" />
										)}
									</div>
									<p className="mt-0.5 text-[13px] text-zinc-500">{n.body}</p>
									<p className="mt-1 font-mono text-[11px] text-zinc-700">
										{relativeTime(n.created_at)}
									</p>
								</div>
								{!n.read && (
									<Form method="post">
										<input type="hidden" name="intent" value="mark-read" />
										<input type="hidden" name="id" value={n.id} />
										<button
											type="submit"
											className="shrink-0 rounded-md border border-zinc-800 bg-zinc-900/50 px-2.5 py-1 font-mono text-[10px] uppercase tracking-wider text-zinc-500 transition-colors hover:border-zinc-700 hover:text-zinc-300"
										>
											Mark read
										</button>
									</Form>
								)}
							</div>
						))}
					</div>
				</Card>
			)}
		</>
	);
}
