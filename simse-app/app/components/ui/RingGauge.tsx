import clsx from 'clsx';

interface RingGaugeProps {
	value: number;
	max: number;
	size?: number;
	strokeWidth?: number;
	label?: string;
	sublabel?: string;
	className?: string;
}

export default function RingGauge({
	value,
	max,
	size = 160,
	strokeWidth = 10,
	label,
	sublabel,
	className,
}: RingGaugeProps) {
	const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
	const radius = (size - strokeWidth) / 2;
	const circumference = 2 * Math.PI * radius;
	const offset = circumference - (pct / 100) * circumference;

	const color =
		pct >= 90
			? 'text-red-400'
			: pct >= 70
				? 'text-amber-400'
				: 'text-emerald-400';

	const strokeColor = pct >= 90 ? '#f87171' : pct >= 70 ? '#fbbf24' : '#34d399';

	return (
		<div
			className={clsx(
				'relative inline-flex items-center justify-center',
				className,
			)}
		>
			<svg
				width={size}
				height={size}
				viewBox={`0 0 ${size} ${size}`}
				className="-rotate-90"
			>
				{/* Background track */}
				<circle
					cx={size / 2}
					cy={size / 2}
					r={radius}
					fill="none"
					stroke="#27272a"
					strokeWidth={strokeWidth}
				/>
				{/* Foreground arc */}
				<circle
					cx={size / 2}
					cy={size / 2}
					r={radius}
					fill="none"
					stroke={strokeColor}
					strokeWidth={strokeWidth}
					strokeLinecap="round"
					strokeDasharray={circumference}
					strokeDashoffset={offset}
					className="animate-ring-fill"
					style={
						{
							'--ring-circumference': circumference,
							'--ring-offset': offset,
						} as React.CSSProperties
					}
				/>
			</svg>
			{/* Center text */}
			<div className="absolute inset-0 flex flex-col items-center justify-center">
				<span className={clsx('text-2xl font-bold tracking-tight', color)}>
					{pct.toFixed(0)}%
				</span>
				{label && (
					<span className="mt-0.5 font-mono text-[10px] font-bold uppercase tracking-[0.2em] text-zinc-500">
						{label}
					</span>
				)}
				{sublabel && (
					<span className="mt-1 font-mono text-[11px] text-zinc-400">
						{sublabel}
					</span>
				)}
			</div>
		</div>
	);
}
