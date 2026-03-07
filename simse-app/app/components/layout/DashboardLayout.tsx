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
			<div className="flex min-w-0 flex-1 flex-col overflow-hidden">
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
						{/* Command bar hint */}
						<button
							type="button"
							className="hidden items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900/50 px-3 py-1.5 text-sm text-zinc-600 transition-colors hover:border-zinc-700 hover:text-zinc-400 sm:flex"
						>
							<svg
								className="h-3.5 w-3.5"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={2}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
								/>
							</svg>
							<span className="text-zinc-600">Search...</span>
							<kbd className="rounded border border-zinc-800 bg-zinc-900 px-1.5 py-0.5 font-mono text-[10px] text-zinc-600">
								/
							</kbd>
						</button>
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
