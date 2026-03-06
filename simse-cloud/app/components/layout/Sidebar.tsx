import clsx from 'clsx';
import { NavLink } from 'react-router';
import SimseLogo from '../ui/SimseLogo';

interface NavItem {
	label: string;
	to: string;
	icon: React.ReactNode;
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
		label: 'Library',
		to: '/dashboard/library',
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
	{
		label: 'Sessions',
		to: '/dashboard/sessions',
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
];

interface SidebarProps {
	onClose?: () => void;
}

export default function Sidebar({ onClose }: SidebarProps) {
	return (
		<aside className="flex h-screen w-60 flex-col border-r border-zinc-800 bg-zinc-950">
			{/* Logo */}
			<div className="flex items-center gap-2.5 px-5 pt-6 pb-4">
				<SimseLogo size={20} className="text-zinc-500" />
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
					</NavLink>
				))}
			</nav>

			{/* Bottom — settings */}
			<div className="border-t border-zinc-800 p-3">
				<NavLink
					to="/dashboard/settings"
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
				</NavLink>
			</div>
		</aside>
	);
}
