import clsx from 'clsx';

interface ToggleProps {
	checked: boolean;
	onChange: (checked: boolean) => void;
	disabled?: boolean;
	className?: string;
}

export default function Toggle({
	checked,
	onChange,
	disabled,
	className,
}: ToggleProps) {
	return (
		<button
			type="button"
			role="switch"
			aria-checked={checked}
			disabled={disabled}
			onClick={() => onChange(!checked)}
			className={clsx(
				'relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-emerald-400/50 focus-visible:ring-offset-2 focus-visible:ring-offset-zinc-950 disabled:cursor-not-allowed disabled:opacity-50',
				checked ? 'bg-emerald-400' : 'bg-zinc-700',
				className,
			)}
		>
			<span
				className={clsx(
					'pointer-events-none inline-block h-3.5 w-3.5 rounded-full shadow-sm transition-transform duration-200',
					checked
						? 'translate-x-[18px] bg-zinc-950'
						: 'translate-x-[3px] bg-zinc-400',
				)}
			/>
		</button>
	);
}
