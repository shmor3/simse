import { useState } from 'react';
import Card from '~/components/ui/Card';
import Toggle from '~/components/ui/Toggle';

const defaultPrefs = [
	{
		id: 'billing',
		label: 'Billing alerts',
		desc: 'Payment receipts and failed payment notices',
		default: true,
		icon: (
			<svg
				className="h-4 w-4 text-amber-400"
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
		id: 'digest',
		label: 'Weekly digest',
		desc: 'Summary of your weekly activity',
		default: true,
		icon: (
			<svg
				className="h-4 w-4 text-blue-400"
				fill="none"
				viewBox="0 0 24 24"
				stroke="currentColor"
				strokeWidth={2}
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01"
				/>
			</svg>
		),
	},
	{
		id: 'product',
		label: 'Product updates',
		desc: 'New features and changelog',
		default: true,
		icon: (
			<svg
				className="h-4 w-4 text-emerald-400"
				fill="none"
				viewBox="0 0 24 24"
				stroke="currentColor"
				strokeWidth={2}
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M13 10V3L4 14h7v7l9-11h-7z"
				/>
			</svg>
		),
	},
	{
		id: 'security',
		label: 'Security alerts',
		desc: 'New device logins and suspicious activity',
		default: true,
		icon: (
			<svg
				className="h-4 w-4 text-red-400"
				fill="none"
				viewBox="0 0 24 24"
				stroke="currentColor"
				strokeWidth={2}
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"
				/>
			</svg>
		),
	},
];

export default function SettingsGeneral() {
	const [prefs, setPrefs] = useState<Record<string, boolean>>(() =>
		Object.fromEntries(defaultPrefs.map((p) => [p.id, p.default])),
	);

	return (
		<div className="animate-fade-in-up">
			<Card className="p-6">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Notifications
				</p>
				<p className="mt-1 text-[13px] text-zinc-600">
					Choose which notifications you receive.
				</p>
				<div className="mt-6 space-y-1">
					{defaultPrefs.map((pref) => (
						<div
							key={pref.id}
							className="flex cursor-pointer items-center justify-between rounded-lg px-3 py-3 transition-colors hover:bg-zinc-800/30"
						>
							<div className="flex items-center gap-3">
								<div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-zinc-800/50">
									{pref.icon}
								</div>
								<div>
									<p className="text-sm text-zinc-200">{pref.label}</p>
									<p className="mt-0.5 text-[13px] text-zinc-600">
										{pref.desc}
									</p>
								</div>
							</div>
							<Toggle
								checked={prefs[pref.id]}
								onChange={(v) =>
									setPrefs((prev) => ({ ...prev, [pref.id]: v }))
								}
							/>
						</div>
					))}
				</div>
			</Card>

			<Card className="mt-6 p-6 animate-stagger-3">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Appearance
				</p>
				<p className="mt-1 text-[13px] text-zinc-600">
					Customize your dashboard experience.
				</p>
				<div className="mt-6 space-y-1">
					<div className="flex cursor-pointer items-center justify-between rounded-lg px-3 py-3 transition-colors hover:bg-zinc-800/30">
						<div className="flex items-center gap-3">
							<div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-zinc-800/50">
								<svg
									className="h-4 w-4 text-zinc-400"
									fill="none"
									viewBox="0 0 24 24"
									stroke="currentColor"
									strokeWidth={2}
								>
									<path
										strokeLinecap="round"
										strokeLinejoin="round"
										d="M4 6h16M4 12h8m-8 6h16"
									/>
								</svg>
							</div>
							<div>
								<p className="text-sm text-zinc-200">Compact mode</p>
								<p className="mt-0.5 text-[13px] text-zinc-600">
									Reduce spacing and card padding
								</p>
							</div>
						</div>
						<Toggle checked={false} onChange={() => {}} />
					</div>
					<div className="flex cursor-pointer items-center justify-between rounded-lg px-3 py-3 transition-colors hover:bg-zinc-800/30">
						<div className="flex items-center gap-3">
							<div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-zinc-800/50">
								<svg
									className="h-4 w-4 text-zinc-400"
									fill="none"
									viewBox="0 0 24 24"
									stroke="currentColor"
									strokeWidth={2}
								>
									<path
										strokeLinecap="round"
										strokeLinejoin="round"
										d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z"
									/>
									<path
										strokeLinecap="round"
										strokeLinejoin="round"
										d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
									/>
								</svg>
							</div>
							<div>
								<p className="text-sm text-zinc-200">Animations</p>
								<p className="mt-0.5 text-[13px] text-zinc-600">
									Enable entrance and hover animations
								</p>
							</div>
						</div>
						<Toggle checked={true} onChange={() => {}} />
					</div>
				</div>
			</Card>
		</div>
	);
}
