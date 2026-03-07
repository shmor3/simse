import clsx from 'clsx';
import { NavLink } from 'react-router';

interface NavPanelProps {
	context: 'home' | 'remote';
	contextName: string;
	remoteId?: string;
	onClose?: () => void;
}

interface NavItem {
	label: string;
	to: string;
	end?: boolean;
	icon: React.ReactNode;
	badge?: number;
	shortcut?: string;
}

const homeNav: NavItem[] = [
	{
		label: 'Overview',
		to: '/dashboard',
		end: true,
		shortcut: '1',
		icon: (
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
					d="M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-4 0a1 1 0 01-1-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 01-1 1"
				/>
			</svg>
		),
	},
	{
		label: 'Usage',
		to: '/dashboard/usage',
		shortcut: '2',
		icon: (
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
					d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
				/>
			</svg>
		),
	},
	{
		label: 'Library',
		to: '/dashboard/library',
		shortcut: '3',
		icon: (
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
					d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
				/>
			</svg>
		),
	},
];

function remoteNav(remoteId: string): NavItem[] {
	return [
		{
			label: 'Chat',
			to: `/dashboard/chat/${remoteId}`,
			shortcut: '1',
			icon: (
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
						d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
					/>
				</svg>
			),
		},
		{
			label: 'Files',
			to: `/dashboard/remote/${remoteId}/files`,
			shortcut: '2',
			icon: (
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
						d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
					/>
				</svg>
			),
		},
		{
			label: 'Shell',
			to: `/dashboard/remote/${remoteId}/shell`,
			shortcut: '3',
			icon: (
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
						d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"
					/>
				</svg>
			),
		},
		{
			label: 'Network',
			to: `/dashboard/remote/${remoteId}/network`,
			shortcut: '4',
			icon: (
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
						d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9"
					/>
				</svg>
			),
		},
	];
}

export default function NavPanel({
	context,
	contextName,
	remoteId,
	onClose,
}: NavPanelProps) {
	const items = context === 'home' ? homeNav : remoteNav(remoteId ?? '');
	const settingsTo =
		context === 'home'
			? '/dashboard/settings'
			: `/dashboard/remote/${remoteId}/settings`;

	return (
		<aside className="flex h-full w-55 flex-col border-r border-zinc-800/50 bg-zinc-950">
			{/* Header */}
			<div className="flex items-center justify-between px-5 pt-6 pb-4">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					{contextName}
				</p>
				{onClose && (
					<button
						type="button"
						onClick={onClose}
						className="rounded-lg p-1 text-zinc-600 transition-colors hover:bg-zinc-800/50 hover:text-zinc-400 md:hidden"
					>
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
								d="M6 18L18 6M6 6l12 12"
							/>
						</svg>
					</button>
				)}
			</div>

			{/* Nav items */}
			<nav className="flex-1 space-y-0.5 px-3">
				{items.map((item) => (
					<NavLink
						key={item.to}
						to={item.to}
						end={item.end}
						onClick={() => onClose?.()}
						className={({ isActive }) =>
							clsx(
								'group relative flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-all',
								isActive
									? 'bg-zinc-800/80 text-white'
									: 'text-zinc-500 hover:bg-zinc-800/40 hover:text-zinc-300',
							)
						}
					>
						{({ isActive }) => (
							<>
								{/* Active indicator bar */}
								{isActive && (
									<div className="absolute -left-3 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-r-full bg-emerald-400" />
								)}
								{item.icon}
								<span className="flex-1">{item.label}</span>
								{item.badge && item.badge > 0 && (
									<span className="flex h-4 min-w-4 items-center justify-center rounded-full bg-emerald-400/15 px-1 font-mono text-[9px] font-bold text-emerald-400">
										{item.badge}
									</span>
								)}
								{item.shortcut && (
									<span className="hidden font-mono text-[10px] text-zinc-700 group-hover:inline">
										{item.shortcut}
									</span>
								)}
							</>
						)}
					</NavLink>
				))}
			</nav>

			{/* Bottom section */}
			<div className="border-t border-zinc-800 p-3 space-y-0.5">
				<NavLink
					to={settingsTo}
					onClick={() => onClose?.()}
					className={({ isActive }) =>
						clsx(
							'group relative flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-all',
							isActive
								? 'bg-zinc-800/80 text-white'
								: 'text-zinc-500 hover:bg-zinc-800/40 hover:text-zinc-300',
						)
					}
				>
					{({ isActive }) => (
						<>
							{isActive && (
								<div className="absolute -left-3 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-r-full bg-emerald-400" />
							)}
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
									d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
								/>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
								/>
							</svg>
							<span>Settings</span>
						</>
					)}
				</NavLink>

				{/* Version */}
				<div className="px-3 pt-2">
					<p className="font-mono text-[10px] text-zinc-800">v0.1.0-alpha</p>
				</div>
			</div>
		</aside>
	);
}
