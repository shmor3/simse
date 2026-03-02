import { Form } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';

import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import { getSession } from '~/lib/session.server';
import type { Route } from './+types/dashboard.notifications';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) return { notifications: [] };

	const db = context.cloudflare.env.DB;
	const notifications = await db
		.prepare(
			'SELECT id, type, title, body, read, link, created_at FROM notifications WHERE user_id = ? ORDER BY created_at DESC LIMIT 100',
		)
		.bind(session.userId)
		.all<{
			id: string;
			type: string;
			title: string;
			body: string;
			read: number;
			link: string | null;
			created_at: string;
		}>();

	return { notifications: notifications.results };
}

export async function action({ request, context }: Route.ActionArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) return null;

	const formData = await request.formData();
	const intent = formData.get('intent');
	const db = context.cloudflare.env.DB;

	if (intent === 'mark-read') {
		const notifId = formData.get('id') as string;
		await db
			.prepare('UPDATE notifications SET read = 1 WHERE id = ? AND user_id = ?')
			.bind(notifId, session.userId)
			.run();
	}

	if (intent === 'mark-all-read') {
		await db
			.prepare(
				'UPDATE notifications SET read = 1 WHERE user_id = ? AND read = 0',
			)
			.bind(session.userId)
			.run();
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
				<Card className="mt-8 p-10 text-center">
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
								d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"
							/>
						</svg>
					</div>
					<p className="mt-4 text-sm font-medium text-zinc-400">
						All caught up
					</p>
					<p className="mt-1 text-[13px] text-zinc-600">
						We'll let you know when something important happens.
					</p>
				</Card>
			) : (
				<Card className="mt-8 overflow-hidden">
					<div className="divide-y divide-zinc-800/50">
						{notifications.map((n) => (
							<div
								key={n.id}
								className={`flex items-start gap-4 px-6 py-4 ${!n.read ? 'bg-zinc-800/20' : ''}`}
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
									<p className="mt-1 text-[12px] text-zinc-700">
										{new Date(n.created_at).toLocaleString()}
									</p>
								</div>
								{!n.read && (
									<Form method="post">
										<input type="hidden" name="intent" value="mark-read" />
										<input type="hidden" name="id" value={n.id} />
										<button
											type="submit"
											className="text-[12px] text-zinc-600 transition-colors hover:text-zinc-400"
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
