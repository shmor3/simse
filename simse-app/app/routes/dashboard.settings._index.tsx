import { useState } from 'react';
import Card from '~/components/ui/Card';
import Toggle from '~/components/ui/Toggle';

const defaultPrefs = [
	{
		id: 'billing',
		label: 'Billing alerts',
		desc: 'Payment receipts and failed payment notices',
		default: true,
	},
	{
		id: 'digest',
		label: 'Weekly digest',
		desc: 'Summary of your weekly activity',
		default: true,
	},
	{
		id: 'product',
		label: 'Product updates',
		desc: 'New features and changelog',
		default: true,
	},
	{
		id: 'security',
		label: 'Security alerts',
		desc: 'New device logins and suspicious activity',
		default: true,
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
							<div>
								<p className="text-sm text-zinc-200">{pref.label}</p>
								<p className="mt-0.5 text-[13px] text-zinc-600">{pref.desc}</p>
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

			<Card className="mt-6 p-6">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Appearance
				</p>
				<p className="mt-1 text-[13px] text-zinc-600">
					Customize your dashboard experience.
				</p>
				<div className="mt-6 space-y-1">
					<div className="flex cursor-pointer items-center justify-between rounded-lg px-3 py-3 transition-colors hover:bg-zinc-800/30">
						<div>
							<p className="text-sm text-zinc-200">Compact mode</p>
							<p className="mt-0.5 text-[13px] text-zinc-600">
								Reduce spacing and card padding
							</p>
						</div>
						<Toggle checked={false} onChange={() => {}} />
					</div>
					<div className="flex cursor-pointer items-center justify-between rounded-lg px-3 py-3 transition-colors hover:bg-zinc-800/30">
						<div>
							<p className="text-sm text-zinc-200">Animations</p>
							<p className="mt-0.5 text-[13px] text-zinc-600">
								Enable entrance and hover animations
							</p>
						</div>
						<Toggle checked={true} onChange={() => {}} />
					</div>
				</div>
			</Card>
		</div>
	);
}
