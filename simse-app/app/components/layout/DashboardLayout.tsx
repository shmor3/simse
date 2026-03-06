import clsx from 'clsx';
import { useState } from 'react';
import { Outlet } from 'react-router';
import AccountDropdown from '../ui/AccountDropdown';
import AcpSwitcher from '../ui/AcpSwitcher';
import NotificationsBell from '../ui/NotificationsBell';
import IconRail from './IconRail';
import NavPanel from './NavPanel';

interface Remote {
	id: string;
	name: string;
	status: 'connected' | 'offline';
}

interface AcpBackend {
	id: string;
	name: string;
	provider: string;
}

interface Notification {
	id: string;
	type: string;
	title: string;
	body: string;
	read: boolean;
	created_at: string;
}

interface DashboardLayoutProps {
	remotes: Remote[];
	activeRemoteId: string | null;
	onRemoteSelect: (id: string | null) => void;
	acpBackends: AcpBackend[];
	activeAcpId: string;
	onAcpSelect: (id: string) => void;
	unreadCount: number;
	notifications: Notification[];
	userName: string;
	userEmail: string;
}

export default function DashboardLayout({
	remotes,
	activeRemoteId,
	onRemoteSelect,
	acpBackends,
	activeAcpId,
	onAcpSelect,
	unreadCount,
	notifications,
	userName,
	userEmail,
}: DashboardLayoutProps) {
	const [mobileOpen, setMobileOpen] = useState(false);

	const openMobile = () => setMobileOpen(true);
	const closeMobile = () => setMobileOpen(false);

	const activeRemote = remotes.find((r) => r.id === activeRemoteId);
	const context: 'home' | 'remote' = activeRemoteId ? 'remote' : 'home';
	const contextName = activeRemoteId
		? (activeRemote?.name ?? 'Remote')
		: 'simse';

	return (
		<div className="flex h-screen bg-[#0a0a0b]">
			{/* Icon Rail - hidden on mobile */}
			<div className="hidden md:block">
				<IconRail
					remotes={remotes}
					activeId={activeRemoteId}
					onSelect={onRemoteSelect}
				/>
			</div>

			{/* Mobile backdrop */}
			{mobileOpen && (
				<div
					className="fixed inset-0 z-40 bg-black/60 md:hidden"
					role="presentation"
					onClick={closeMobile}
					onKeyDown={(e) => {
						if (e.key === 'Escape') closeMobile();
					}}
				/>
			)}

			{/* Nav Panel - always visible on desktop, drawer on mobile */}
			<div
				className={clsx(
					'fixed inset-y-0 left-0 z-50 w-55 transform transition-transform duration-200 ease-out md:static md:z-auto',
					mobileOpen ? 'translate-x-0' : '-translate-x-full md:translate-x-0',
				)}
			>
				<NavPanel
					context={context}
					contextName={contextName}
					remoteId={activeRemoteId ?? undefined}
					onClose={closeMobile}
				/>
			</div>

			{/* Main content */}
			<div className="flex min-w-0 flex-1 flex-col">
				{/* Header */}
				<header className="flex h-14 items-center justify-between border-b border-zinc-800/50 px-4">
					<div className="flex items-center gap-3">
						{/* Hamburger on mobile */}
						<button
							type="button"
							className="rounded-lg p-1.5 text-zinc-500 hover:bg-zinc-800/50 hover:text-zinc-300 md:hidden"
							onClick={openMobile}
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
						<AcpSwitcher
							backends={acpBackends}
							activeId={activeAcpId}
							onSelect={onAcpSelect}
						/>
					</div>
					<div className="flex items-center gap-2">
						<NotificationsBell
							unreadCount={unreadCount}
							notifications={notifications}
						/>
						<AccountDropdown name={userName} email={userEmail} />
					</div>
				</header>

				{/* Content */}
				<main className="flex-1 overflow-y-auto">
					<div className="mx-auto max-w-5xl px-4 py-8 sm:px-6 lg:px-8">
						<Outlet />
					</div>
				</main>
			</div>
		</div>
	);
}
