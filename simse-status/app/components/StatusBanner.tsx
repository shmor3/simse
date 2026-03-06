interface Props {
	services: Array<{
		status: 'up' | 'degraded' | 'down' | 'unknown';
	}>;
}

export function StatusBanner({ services }: Props) {
	const hasDown = services.some((s) => s.status === 'down');
	const hasDegraded = services.some((s) => s.status === 'degraded');

	let label: string;
	let color: string;
	let dotColor: string;

	if (hasDown) {
		label = 'Major Outage';
		color = 'border-red-500/30 bg-red-500/5';
		dotColor = 'bg-red-400';
	} else if (hasDegraded) {
		label = 'Partial Degradation';
		color = 'border-yellow-500/30 bg-yellow-500/5';
		dotColor = 'bg-yellow-400';
	} else {
		label = 'All Systems Operational';
		color = 'border-emerald-500/30 bg-emerald-500/5';
		dotColor = 'bg-emerald-400';
	}

	return (
		<div
			className={`animate-fade-in rounded-xl border px-6 py-4 text-center ${color}`}
		>
			<div className="flex items-center justify-center gap-2.5">
				<span className={`inline-block h-2.5 w-2.5 rounded-full ${dotColor}`} />
				<span className="font-mono text-sm font-semibold text-zinc-200">
					{label}
				</span>
			</div>
		</div>
	);
}
