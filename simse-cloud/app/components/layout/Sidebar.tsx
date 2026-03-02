import clsx from 'clsx';
import { NavLink } from 'react-router';
import Avatar from '../ui/Avatar';

interface NavItem {
	label: string;
	to: string;
	icon: React.ReactNode;
	badge?: number;
}

const nav: NavItem[] = [
	{
		label: 'Overview',
		to: '/dashboard',
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
		label: 'Billing',
		to: '/dashboard/billing',
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
					d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z"
				/>
			</svg>
		),
	},
	{
		label: 'Team',
		to: '/dashboard/team',
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
					d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z"
				/>
			</svg>
		),
	},
	{
		label: 'Notifications',
		to: '/dashboard/notifications',
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
					d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"
				/>
			</svg>
		),
	},
	{
		label: 'Account',
		to: '/dashboard/account',
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
					d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
				/>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
				/>
			</svg>
		),
	},
];

interface SidebarProps {
	unreadCount?: number;
	userName: string;
	onClose?: () => void;
}

export default function Sidebar({
	unreadCount = 0,
	userName,
	onClose,
}: SidebarProps) {
	return (
		<aside className="flex h-screen w-60 flex-col border-r border-zinc-800 bg-zinc-950">
			{/* Logo */}
			<div className="px-5 pt-6 pb-4">
				<p className="font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-zinc-500">
					SIMSE
				</p>
			</div>

			{/* Nav */}
			<nav className="flex-1 space-y-0.5 px-3">
				{nav.map((item) => (
					<NavLink
						key={item.to}
						to={item.to}
						end={item.to === '/dashboard'}
						onClick={() => onClose?.()}
						className={({ isActive }) =>
							clsx(
								'flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors',
								isActive
									? 'bg-zinc-800/80 text-white'
									: 'text-zinc-500 hover:bg-zinc-800/40 hover:text-zinc-300',
							)
						}
					>
						{item.icon}
						<span>{item.label}</span>
						{item.label === 'Notifications' && unreadCount > 0 && (
							<span className="ml-auto inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-emerald-400/15 px-1 font-mono text-[10px] font-bold text-emerald-400">
								{unreadCount > 99 ? '99+' : unreadCount}
							</span>
						)}
					</NavLink>
				))}
			</nav>

			{/* Bottom — user info */}
			<div className="border-t border-zinc-800 p-4">
				<div className="flex items-center gap-3 px-3 py-2">
					<Avatar name={userName} size="sm" />
					<div className="min-w-0 flex-1">
						<p className="truncate text-sm text-zinc-400">{userName}</p>
					</div>
				</div>
			</div>
		</aside>
	);
}
