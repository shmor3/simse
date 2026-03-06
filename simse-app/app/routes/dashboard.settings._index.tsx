import Card from '~/components/ui/Card';

export default function SettingsGeneral() {
	return (
		<Card className="p-6">
			<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
				Preferences
			</p>
			<div className="mt-6 space-y-4">
				{[
					{
						id: 'billing',
						label: 'Billing alerts',
						desc: 'Payment receipts and failed payment notices',
					},
					{
						id: 'digest',
						label: 'Weekly digest',
						desc: 'Summary of your weekly activity',
					},
					{
						id: 'product',
						label: 'Product updates',
						desc: 'New features and changelog',
					},
					{
						id: 'security',
						label: 'Security alerts',
						desc: 'New device logins and suspicious activity',
					},
				].map((pref) => (
					<label key={pref.id} className="flex items-center justify-between">
						<div>
							<p className="text-sm text-zinc-200">{pref.label}</p>
							<p className="text-[13px] text-zinc-600">{pref.desc}</p>
						</div>
						<input
							type="checkbox"
							defaultChecked
							className="h-4 w-4 rounded border-zinc-700 bg-zinc-800 text-emerald-400 accent-emerald-400"
						/>
					</label>
				))}
			</div>
		</Card>
	);
}
