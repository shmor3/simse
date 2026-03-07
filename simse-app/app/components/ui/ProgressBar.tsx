import clsx from 'clsx';

interface ProgressBarProps {
	value: number;
	max: number;
	label?: string;
	showValue?: boolean;
	className?: string;
}

export default function ProgressBar({
	value,
	max,
	label,
	showValue = true,
	className,
}: ProgressBarProps) {
	const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
	const isHigh = pct >= 90;
	const isMedium = pct >= 70;

	return (
		<div className={clsx('space-y-2', className)}>
			{(label || showValue) && (
				<div className="flex items-center justify-between">
					{label && (
						<span className="font-mono text-[11px] font-bold uppercase tracking-[0.15em] text-zinc-500">
							{label}
						</span>
					)}
					{showValue && (
						<span
							className={clsx(
								'font-mono text-[12px]',
								isHigh
									? 'text-red-400'
									: isMedium
										? 'text-amber-400'
										: 'text-zinc-400',
							)}
						>
							{value.toLocaleString()} / {max.toLocaleString()}
						</span>
					)}
				</div>
			)}
			<div className="h-2 overflow-hidden rounded-full bg-zinc-800">
				<div
					className={clsx(
						'h-full rounded-full transition-all duration-700 ease-out',
						isHigh
							? 'bg-gradient-to-r from-red-500 to-red-400'
							: isMedium
								? 'bg-gradient-to-r from-amber-500 to-amber-400'
								: 'bg-gradient-to-r from-emerald-500 to-emerald-400',
					)}
					style={{ width: `${pct}%` }}
				/>
			</div>
		</div>
	);
}
