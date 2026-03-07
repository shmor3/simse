import clsx from 'clsx';
import type { InputHTMLAttributes } from 'react';

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
	label?: string;
	error?: string;
	icon?: React.ReactNode;
}

export default function Input({
	label,
	error,
	icon,
	className,
	id,
	...props
}: InputProps) {
	const inputId = id || props.name;

	return (
		<div className="space-y-1.5">
			{label && (
				<label
					htmlFor={inputId}
					className="block font-mono text-[11px] font-bold uppercase tracking-[0.15em] text-zinc-500"
				>
					{label}
				</label>
			)}
			<div className="relative">
				{icon && (
					<div className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-600">
						{icon}
					</div>
				)}
				<input
					id={inputId}
					className={clsx(
						'w-full rounded-lg border bg-zinc-900 px-3 py-2.5 text-sm text-zinc-100 placeholder:text-zinc-600 transition-all focus:border-emerald-400/50 focus:outline-none focus:ring-1 focus:ring-emerald-400/25 focus:shadow-[0_0_12px_rgba(52,211,153,0.06)]',
						error
							? 'border-red-500/50'
							: 'border-zinc-800 hover:border-zinc-700',
						icon && 'pl-10',
						className,
					)}
					{...props}
				/>
			</div>
			{error && <p className="text-[13px] text-red-400/80">{error}</p>}
		</div>
	);
}
