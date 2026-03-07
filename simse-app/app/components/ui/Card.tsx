import clsx from 'clsx';
import type { HTMLAttributes } from 'react';

interface CardProps extends HTMLAttributes<HTMLDivElement> {
	accent?: boolean | 'gradient';
}

export default function Card({
	accent,
	className,
	children,
	...props
}: CardProps) {
	return (
		<div
			className={clsx(
				'rounded-xl border border-zinc-800 bg-zinc-900',
				accent && 'overflow-hidden',
				className,
			)}
			{...props}
		>
			{accent === true && <div className="h-px bg-emerald-400" />}
			{accent === 'gradient' && (
				<div className="h-px gradient-border animate-gradient-shift" />
			)}
			{children}
		</div>
	);
}
