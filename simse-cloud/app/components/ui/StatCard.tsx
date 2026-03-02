import clsx from 'clsx';
import Card from './Card';

interface StatCardProps {
	label: string;
	value: string | number;
	change?: string;
	positive?: boolean;
	className?: string;
}

export default function StatCard({
	label,
	value,
	change,
	positive,
	className,
}: StatCardProps) {
	return (
		<Card className={clsx('p-5', className)}>
			<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
				{label}
			</p>
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
