import { redirect } from 'react-router';
import DashboardLayout from '~/components/layout/DashboardLayout';
import { getSession } from '~/lib/session.server';
import type { Route } from './+types/dashboard';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) throw redirect('/auth/login');

	// Count unread notifications
	const result = await context.cloudflare.env.DB.prepare(
		'SELECT COUNT(*) as count FROM notifications WHERE user_id = ? AND read = 0',
	)
		.bind(session.userId)
		.first<{ count: number }>();

	return { unreadCount: result?.count ?? 0 };
}

export default function Dashboard({ loaderData }: Route.ComponentProps) {
	return <DashboardLayout unreadCount={loaderData.unreadCount} />;
}
