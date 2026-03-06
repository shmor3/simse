import { useState } from 'react';
import { Outlet } from 'react-router';
import AccountDropdown from '../ui/AccountDropdown';
import NotificationsBell from '../ui/NotificationsBell';
import Sidebar from './Sidebar';

interface Notification {
	id: string;
	type: string;
	title: string;
	body: string;
	read: boolean;
	created_at: string;
}

interface DashboardLayoutProps {
	unreadCount?: number;
	notifications?: Notification[];
	userName: string;
	userEmail: string;
}

export default function DashboardLayout({
	unreadCount = 0,
	notifications = [],
	userName,
	userEmail,
}: DashboardLayoutProps) {
	const [sidebarOpen, setSidebarOpen] = useState(false);

	return (
		<div className="flex h-screen bg-[#0a0a0b]">
			{/* Mobile backdrop */}
			{sidebarOpen && (
				<div
					className="fixed inset-0 z-40 bg-black/60 md:hidden"
					role="presentation"
					onClick={() => setSidebarOpen(false)}
					onKeyDown={(e) => {
						if (e.key === 'Escape') setSidebarOpen(false);
					}}
				/>
			)}

			{/* Sidebar */}
			<div
				className={`fixed inset-y-0 left-0 z-50 w-60 transform transition-transform duration-200 ease-out md:static md:translate-x-0 ${sidebarOpen ? 'translate-x-0' : '-translate-x-full'}`}
			>
				<Sidebar onClose={() => setSidebarOpen(false)} />
			</div>

			<div className="flex flex-1 flex-col overflow-hidden">
				{/* Header bar */}
				<header className="flex h-14 shrink-0 items-center justify-between border-b border-zinc-800/50 px-6">
					{/* Mobile hamburger */}
					<button
						type="button"
						onClick={() => setSidebarOpen(true)}
						className="rounded-lg p-1.5 text-zinc-500 hover:bg-zinc-800/50 hover:text-zinc-300 md:hidden"
					>
						<svg
							className="h-5 w-5"
							fill="none"
							viewBox="0 0 24 24"
							stroke="currentColor"
							strokeWidth={2}
						>
							<path
								strokeLinecap="round"
								strokeLinejoin="round"
								d="M4 6h16M4 12h16M4 18h16"
							/>
						</svg>
					</button>
					<div className="hidden md:block" />
					<div className="flex items-center gap-1">
						<NotificationsBell
							unreadCount={unreadCount}
							notifications={notifications}
						/>
						<AccountDropdown name={userName} email={userEmail} />
					</div>
				</header>

				{/* Main content */}
				<main className="flex-1 overflow-y-auto">
					<div className="mx-auto max-w-5xl px-4 py-6 sm:px-8 sm:py-8">
						<Outlet />
					</div>
				</main>
			</div>
		</div>
	);
}
