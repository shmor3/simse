import clsx from 'clsx';

interface AvatarProps {
	name: string;
	size?: 'sm' | 'md' | 'lg';
	className?: string;
}

const sizes = {
	sm: 'h-7 w-7 text-[10px]',
	md: 'h-9 w-9 text-[12px]',
	lg: 'h-12 w-12 text-[14px]',
};

function initials(name: string): string {
	return name
		.split(' ')
		.slice(0, 2)
		.map((w) => w[0])
		.join('')
		.toUpperCase();
}

export default function Avatar({ name, size = 'md', className }: AvatarProps) {
	return (
		<div
			className={clsx(
				'inline-flex items-center justify-center rounded-full bg-emerald-400/10 font-mono font-bold text-emerald-400',
				sizes[size],
				className,
			)}
			title={name}
		>
			{initials(name)}
		</div>
	);
}
