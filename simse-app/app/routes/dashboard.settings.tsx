import clsx from 'clsx';
import { NavLink, Outlet } from 'react-router';

const tabs = [
	{ label: 'General', to: '/dashboard/settings' },
	{ label: 'Billing', to: '/dashboard/settings/billing' },
	{ label: 'Team', to: '/dashboard/settings/team' },
];

export default function SettingsLayout() {
	return (
		<>
			<h1 className="text-2xl font-bold text-white">Settings</h1>
			<nav className="mt-4 flex gap-1 border-b border-zinc-800">
				{tabs.map((tab) => (
					<NavLink
						key={tab.to}
						to={tab.to}
						end={tab.to === '/dashboard/settings'}
						className={({ isActive }) =>
							clsx(
								'px-4 py-2.5 text-sm transition-colors',
								isActive
									? 'border-b-2 border-emerald-400 text-white'
									: 'text-zinc-500 hover:text-zinc-300',
							)
						}
					>
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
