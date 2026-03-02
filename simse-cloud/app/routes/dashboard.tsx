import { redirect } from 'react-router';
import DashboardLayout from '~/components/layout/DashboardLayout';
import { getSession } from '~/lib/session.server';
import type { Route } from './+types/dashboard';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) throw redirect('/auth/login');

	const db = context.cloudflare.env.DB;

	// Count unread notifications
	const result = await db
		.prepare(
			'SELECT COUNT(*) as count FROM notifications WHERE user_id = ? AND read = 0',
		)
		.bind(session.userId)
		.first<{ count: number }>();

	// Get user info for header
	const user = await db
		.prepare('SELECT name, email FROM users WHERE id = ?')
		.bind(session.userId)
		.first<{ name: string; email: string }>();

	return {
		unreadCount: result?.count ?? 0,
		userName: user?.name ?? '',
		userEmail: user?.email ?? '',
	};
}

export default function Dashboard({ loaderData }: Route.ComponentProps) {
	return (
		<DashboardLayout
			unreadCount={loaderData.unreadCount}
			userName={loaderData.userName}
			userEmail={loaderData.userEmail}
		/>
	);
}
