import clsx from 'clsx';
import Card from './Card';
import Sparkline from './Sparkline';

interface StatCardProps {
	label: string;
	value: string | number;
	change?: string;
	positive?: boolean;
	loading?: boolean;
	sparklineData?: number[];
	className?: string;
}

export default function StatCard({
	label,
	value,
	change,
	positive,
	loading,
	sparklineData,
	className,
}: StatCardProps) {
	if (loading) {
		return (
			<Card className={clsx('p-5', className)}>
				<div className="h-3 w-16 rounded bg-zinc-800 animate-shimmer bg-gradient-to-r from-zinc-800 via-zinc-700 to-zinc-800" />
				<div className="mt-3 h-7 w-24 rounded bg-zinc-800 animate-shimmer bg-gradient-to-r from-zinc-800 via-zinc-700 to-zinc-800" />
				{sparklineData && (
					<div className="mt-3 h-7 w-full rounded bg-zinc-800 animate-shimmer bg-gradient-to-r from-zinc-800 via-zinc-700 to-zinc-800" />
				)}
			</Card>
		);
	}

	return (
		<Card className={clsx('card-hover p-5', className)}>
			<div className="flex items-start justify-between">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					{label}
				</p>
				{sparklineData && sparklineData.length >= 2 && (
					<Sparkline
						data={sparklineData}
						width={64}
						height={24}
						className="opacity-60"
					/>
				)}
			</div>
			<p className="mt-3 text-2xl font-bold tracking-tight text-white">
				{value}
			</p>
			{change && (
				<p
					className={clsx(
						'mt-1 font-mono text-[12px]',
						positive ? 'text-emerald-400' : 'text-red-400',
					)}
				>
					{positive ? '+' : ''}
					{change}
				</p>
			)}
		</Card>
	);
}
