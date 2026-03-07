interface Props {
	dailyStatus: Array<{ date: string; status: 'up' | 'degraded' | 'down' }>;
}

const STATUS_COLORS: Record<string, string> = {
	up: 'bg-emerald-400',
	degraded: 'bg-yellow-400',
	down: 'bg-red-400',
	empty: 'bg-zinc-800',
};

export function UptimeBar({ dailyStatus }: Props) {
	const today = new Date();
	const days: Array<{
		date: string;
		status: 'up' | 'degraded' | 'down' | 'empty';
	}> = [];
	const statusMap = new Map(dailyStatus.map((d) => [d.date, d.status]));

	for (let i = 89; i >= 0; i--) {
		const d = new Date(today);
		d.setUTCDate(d.getUTCDate() - i);
		const key = d.toISOString().slice(0, 10);
		days.push({ date: key, status: statusMap.get(key) ?? 'empty' });
	}

	return (
		<div className="flex gap-px">
			{days.map((day) => (
				<div
					key={day.date}
					className={`h-8 flex-1 rounded-[1px] first:rounded-l last:rounded-r ${STATUS_COLORS[day.status]}`}
					title={`${day.date}: ${day.status}`}
				/>
			))}
		</div>
	);
}
