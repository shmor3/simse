import { useState } from 'react';
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

	// Get remotes
	type Remote = {
		id: string;
		name: string;
		status: 'connected' | 'offline';
	};
	let remotes: Remote[] = [];
	try {
		const remotesRes = await authenticatedApi(request, '/remotes');
		if (remotesRes.ok) {
			const remotesJson = (await remotesRes.json()) as ApiResponse<
				Remote[]
			>;
			remotes = remotesJson.data ?? [];
		}
	} catch {
		// ignore
	}

	// Get ACP backends
	type AcpBackend = {
		id: string;
		name: string;
		provider: string;
	};
	let acpBackends: AcpBackend[] = [];
	try {
		const backendsRes = await authenticatedApi(request, '/acp/backends');
		if (backendsRes.ok) {
			const backendsJson = (await backendsRes.json()) as ApiResponse<
				AcpBackend[]
			>;
			acpBackends = backendsJson.data ?? [];
		}
	} catch {
		// ignore
	}

	return {
		unreadCount: notifications.filter((n) => !n.read).length,
		notifications,
		userName: user?.name ?? '',
		userEmail: user?.email ?? '',
		remotes,
		acpBackends,
	};
}

export default function Dashboard({ loaderData }: Route.ComponentProps) {
	const [activeRemoteId, setActiveRemoteId] = useState<string | null>(null);
	const [activeAcpId, setActiveAcpId] = useState<string>(
		loaderData.acpBackends[0]?.id ?? '',
	);

	return (
		<DashboardLayout
			remotes={loaderData.remotes}
			activeRemoteId={activeRemoteId}
			onRemoteSelect={setActiveRemoteId}
			acpBackends={loaderData.acpBackends}
			activeAcpId={activeAcpId}
			onAcpSelect={setActiveAcpId}
			unreadCount={loaderData.unreadCount}
			notifications={loaderData.notifications}
			userName={loaderData.userName}
			userEmail={loaderData.userEmail}
		/>
	);
}
