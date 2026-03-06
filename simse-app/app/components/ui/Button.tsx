import clsx from 'clsx';
import type { ButtonHTMLAttributes } from 'react';

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';

const variants: Record<Variant, string> = {
	primary:
		'bg-emerald-400 text-zinc-950 hover:bg-emerald-300 active:bg-emerald-500',
	secondary:
		'bg-zinc-800 text-zinc-100 border border-zinc-700 hover:bg-zinc-700 active:bg-zinc-600',
	ghost:
		'text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800/50 active:bg-zinc-800',
	danger:
		'bg-red-500/10 text-red-400 border border-red-500/20 hover:bg-red-500/20 active:bg-red-500/30',
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
	variant?: Variant;
	loading?: boolean;
}

export default function Button({
	variant = 'primary',
	loading,
	className,
	children,
	disabled,
	...props
}: ButtonProps) {
	return (
		<button
			className={clsx(
				'inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2.5 font-mono text-sm font-bold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-400/50 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 disabled:pointer-events-none disabled:opacity-50',
				variants[variant],
				className,
			)}
			disabled={disabled || loading}
			{...props}
		>
			{loading ? (
				<>
					<span className="h-4 w-4 animate-spin rounded-full border-2 border-current border-t-transparent" />
					{children}
				</>
			) : (
				children
			)}
		</button>
	);
}
