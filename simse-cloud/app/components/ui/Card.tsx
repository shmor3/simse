import clsx from 'clsx';
import type { HTMLAttributes } from 'react';

interface CardProps extends HTMLAttributes<HTMLDivElement> {
	accent?: boolean;
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
			{accent && <div className="h-1 bg-emerald-400" />}
			{children}
		</div>
	);
}
