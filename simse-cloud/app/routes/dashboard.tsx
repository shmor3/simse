import { redirect } from 'react-router';
import DashboardLayout from '~/components/layout/DashboardLayout';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard';

export async function loader({ request }: Route.LoaderArgs) {
	const res = await authenticatedApi(request, '/auth/me');
	if (!res.ok) throw redirect('/auth/login');

	const json = (await res.json()) as ApiResponse<{
		name: string;
		email: string;
	}>;
	const user = json.data;

	// Get notifications
	type Notif = {
		id: string;
		type: string;
		title: string;
		body: string;
		read: boolean;
		created_at: string;
	};
	let notifications: Notif[] = [];
	try {
		const notifRes = await authenticatedApi(request, '/notifications');
		if (notifRes.ok) {
			const notifJson = (await notifRes.json()) as ApiResponse<Notif[]>;
			notifications = notifJson.data ?? [];
		}
	} catch {
		// ignore
	}

	return {
		unreadCount: notifications.filter((n) => !n.read).length,
		notifications,
		userName: user?.name ?? '',
		userEmail: user?.email ?? '',
	};
}

export default function Dashboard({ loaderData }: Route.ComponentProps) {
	return (
		<DashboardLayout
			unreadCount={loaderData.unreadCount}
			notifications={loaderData.notifications}
			userName={loaderData.userName}
			userEmail={loaderData.userEmail}
		/>
	);
}
