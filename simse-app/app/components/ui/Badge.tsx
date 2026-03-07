import clsx from 'clsx';

type BadgeVariant = 'default' | 'emerald' | 'warning' | 'danger' | 'info';

const variants: Record<BadgeVariant, string> = {
	default: 'bg-zinc-800 text-zinc-400 border-zinc-700',
	emerald: 'bg-emerald-400/10 text-emerald-400 border-emerald-400/20',
	warning: 'bg-amber-400/10 text-amber-400 border-amber-400/20',
	danger: 'bg-red-400/10 text-red-400 border-red-400/20',
	info: 'bg-blue-400/10 text-blue-400 border-blue-400/20',
};

interface BadgeProps {
	variant?: BadgeVariant;
	dot?: boolean;
	children: React.ReactNode;
	className?: string;
}

const dotColors: Record<BadgeVariant, string> = {
	default: 'bg-zinc-400',
	emerald: 'bg-emerald-400',
	warning: 'bg-amber-400',
	danger: 'bg-red-400',
	info: 'bg-blue-400',
};

export default function Badge({
	variant = 'default',
	dot,
	children,
	className,
}: BadgeProps) {
	return (
		<span
			className={clsx(
				'inline-flex items-center gap-1.5 rounded-md border px-2 py-0.5 font-mono text-[11px] font-bold uppercase tracking-wider',
				variants[variant],
				className,
			)}
		>
			{dot && (
				<span
					className={clsx('h-1.5 w-1.5 rounded-full', dotColors[variant])}
				/>
			)}
			{children}
		</span>
	);
}
