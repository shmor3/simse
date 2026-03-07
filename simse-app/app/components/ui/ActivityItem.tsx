import clsx from 'clsx';

type ActivityType = 'session' | 'library' | 'remote' | 'team' | 'billing';

interface ActivityItemProps {
	type: ActivityType;
	title: string;
	description: string;
	time: string;
	isLast?: boolean;
}

const typeStyles: Record<ActivityType, { bg: string; text: string }> = {
	session: { bg: 'bg-emerald-400/10', text: 'text-emerald-400' },
	library: { bg: 'bg-blue-400/10', text: 'text-blue-400' },
	remote: { bg: 'bg-cyan-400/10', text: 'text-cyan-400' },
	team: { bg: 'bg-violet-400/10', text: 'text-violet-400' },
	billing: { bg: 'bg-amber-400/10', text: 'text-amber-400' },
};

const typeIcons: Record<ActivityType, React.ReactNode> = {
	session: (
		<svg
			className="h-3.5 w-3.5"
			fill="none"
			viewBox="0 0 24 24"
			stroke="currentColor"
			strokeWidth={2}
		>
			<path
				strokeLinecap="round"
				strokeLinejoin="round"
				d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
			/>
		</svg>
	),
	library: (
		<svg
			className="h-3.5 w-3.5"
			fill="none"
			viewBox="0 0 24 24"
			stroke="currentColor"
			strokeWidth={2}
		>
			<path
				strokeLinecap="round"
				strokeLinejoin="round"
				d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253"
			/>
		</svg>
	),
	remote: (
		<svg
			className="h-3.5 w-3.5"
			fill="none"
			viewBox="0 0 24 24"
			stroke="currentColor"
			strokeWidth={2}
		>
			<path
				strokeLinecap="round"
				strokeLinejoin="round"
				d="M5 12h14M5 12a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v4a2 2 0 01-2 2M5 12a2 2 0 00-2 2v4a2 2 0 002 2h14a2 2 0 002-2v-4a2 2 0 00-2-2m-2-4h.01M17 16h.01"
			/>
		</svg>
	),
	team: (
		<svg
			className="h-3.5 w-3.5"
			fill="none"
			viewBox="0 0 24 24"
			stroke="currentColor"
			strokeWidth={2}
		>
			<path
				strokeLinecap="round"
				strokeLinejoin="round"
				d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"
			/>
		</svg>
	),
	billing: (
		<svg
			className="h-3.5 w-3.5"
			fill="none"
			viewBox="0 0 24 24"
			stroke="currentColor"
			strokeWidth={2}
		>
			<path
				strokeLinecap="round"
				strokeLinejoin="round"
				d="M3 10h18M7 15h1m4 0h1m-7 4h12a3 3 0 003-3V8a3 3 0 00-3-3H6a3 3 0 00-3 3v8a3 3 0 003 3z"
			/>
		</svg>
	),
};

export default function ActivityItem({
	type,
	title,
	description,
	time,
	isLast,
}: ActivityItemProps) {
	const style = typeStyles[type];

	return (
		<div className="relative flex gap-4">
			{/* Timeline connector */}
			{!isLast && (
				<div className="absolute left-[15px] top-8 bottom-0 w-px bg-zinc-800" />
			)}
			{/* Icon */}
			<div
				className={clsx(
					'relative z-10 flex h-8 w-8 shrink-0 items-center justify-center rounded-full',
					style.bg,
					style.text,
				)}
			>
				{typeIcons[type]}
			</div>
			{/* Content */}
			<div className="min-w-0 flex-1 pb-6">
				<p className="text-sm font-medium text-zinc-200">{title}</p>
				<p className="mt-0.5 text-[13px] text-zinc-500">{description}</p>
				<p className="mt-1 font-mono text-[11px] text-zinc-600">{time}</p>
			</div>
		</div>
	);
}
