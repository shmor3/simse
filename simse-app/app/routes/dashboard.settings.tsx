import clsx from 'clsx';
import { NavLink, Outlet } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';

const tabs = [
	{
		label: 'General',
		to: '/dashboard/settings',
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
					d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4"
				/>
			</svg>
		),
	},
	{
		label: 'Billing',
		to: '/dashboard/settings/billing',
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
		to: '/dashboard/settings/team',
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
					d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"
				/>
			</svg>
		),
	},
];

export default function SettingsLayout() {
	return (
		<>
			<PageHeader
				title="Settings"
				description="Manage your preferences, billing, and team."
			/>
			<nav className="mt-6 flex gap-1 rounded-lg bg-zinc-900/50 p-1">
				{tabs.map((tab) => (
					<NavLink
						key={tab.to}
						to={tab.to}
						end={tab.to === '/dashboard/settings'}
						className={({ isActive }) =>
							clsx(
								'flex items-center gap-2 rounded-md px-4 py-2 text-sm transition-all',
								isActive
									? 'bg-zinc-800 text-white shadow-sm'
									: 'text-zinc-500 hover:text-zinc-300',
							)
						}
					>
						{tab.icon}
						{tab.label}
					</NavLink>
				))}
			</nav>
			<div className="mt-6">
				<Outlet />
			</div>
		</>
	);
}
