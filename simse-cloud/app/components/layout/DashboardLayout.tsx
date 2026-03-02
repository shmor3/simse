import { Outlet } from 'react-router';
import Sidebar from './Sidebar';

interface DashboardLayoutProps {
	unreadCount?: number;
}

export default function DashboardLayout({ unreadCount }: DashboardLayoutProps) {
	return (
		<div className="flex h-screen bg-[#0a0a0b]">
			<Sidebar unreadCount={unreadCount} />
			<main className="flex-1 overflow-y-auto">
				<div className="mx-auto max-w-5xl px-8 py-8">
					<Outlet />
				</div>
			</main>
		</div>
	);
}
