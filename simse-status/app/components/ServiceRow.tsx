import { UptimeBar } from './UptimeBar';

interface Props {
	service: {
		id: string;
		name: string;
		status: 'up' | 'degraded' | 'down' | 'unknown';
		responseTimeMs: number | null;
		uptimePercent: number;
		dailyStatus: Array<{
			date: string;
			status: 'up' | 'degraded' | 'down';
		}>;
	};
}

const STATUS_DOT: Record<string, string> = {
	up: 'bg-emerald-400',
	degraded: 'bg-yellow-400',
	down: 'bg-red-400',
	unknown: 'bg-zinc-600',
};

const STATUS_LABEL: Record<string, string> = {
	up: 'Operational',
	degraded: 'Degraded',
	down: 'Down',
	unknown: 'Unknown',
};

export function ServiceRow({ service }: Props) {
	return (
		<div className="animate-fade-in-up rounded-xl border border-zinc-800 bg-zinc-900/50 p-5">
			<div className="mb-3 flex items-center justify-between">
				<div className="flex items-center gap-3">
					<span
						className={`inline-block h-2 w-2 rounded-full ${STATUS_DOT[service.status]}`}
					/>
					<span className="text-sm font-medium text-zinc-200">
						{service.name}
					</span>
				</div>
				<div className="flex items-center gap-4">
					{service.responseTimeMs !== null && (
						<span className="font-mono text-xs text-zinc-500">
							{service.responseTimeMs}ms
						</span>
					)}
					<span className="font-mono text-xs text-zinc-400">
						{service.uptimePercent}%
					</span>
					<span className="text-xs text-zinc-500">
						{STATUS_LABEL[service.status]}
					</span>
				</div>
			</div>
			<UptimeBar dailyStatus={service.dailyStatus} />
			<div className="mt-1 flex justify-between font-mono text-[10px] text-zinc-600">
				<span>90 days ago</span>
				<span>Today</span>
			</div>
		</div>
	);
}
