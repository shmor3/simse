import { redirect } from 'react-router';
import DashboardLayout from '~/components/layout/DashboardLayout';
import { authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard';

export async function loader({ request }: Route.LoaderArgs) {
	const res = await authenticatedApi(request, '/auth/me');
	if (!res.ok) throw redirect('/auth/login');

	const json = await res.json() as any;
	const user = json.data;

	// Get unread notification count
	let unreadCount = 0;
	try {
		const notifRes = await authenticatedApi(request, '/notifications');
		if (notifRes.ok) {
			const notifJson = await notifRes.json() as any;
			const notifications = notifJson.data ?? [];
			unreadCount = notifications.filter((n: any) => !n.read).length;
		}
	} catch {
		// ignore
	}

	return {
		unreadCount,
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
