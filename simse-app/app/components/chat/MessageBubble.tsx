import clsx from 'clsx';
import SimseLogo from '../ui/SimseLogo';

interface MessageBubbleProps {
	role: 'user' | 'assistant' | 'system';
	content: string;
}

function RoleIcon({ role }: { role: MessageBubbleProps['role'] }) {
	if (role === 'assistant') {
		return (
			<div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-lg bg-emerald-400/10">
				<SimseLogo size={14} className="text-emerald-400" />
			</div>
		);
	}
	if (role === 'user') {
		return (
			<div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-lg bg-zinc-800">
				<svg
					className="h-3.5 w-3.5 text-zinc-400"
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					strokeWidth={2}
				>
					<path
						strokeLinecap="round"
						strokeLinejoin="round"
						d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z"
					/>
				</svg>
			</div>
		);
	}
	return (
		<div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-lg bg-amber-400/10">
			<svg
				className="h-3.5 w-3.5 text-amber-400"
				fill="none"
				viewBox="0 0 24 24"
				stroke="currentColor"
				strokeWidth={2}
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
				/>
			</svg>
		</div>
	);
}

const roleLabels: Record<MessageBubbleProps['role'], string> = {
	user: 'You',
	assistant: 'simse',
	system: 'System',
};

export default function MessageBubble({ role, content }: MessageBubbleProps) {
	return (
		<div
			className={clsx(
				'mx-auto max-w-3xl px-4 py-4 animate-fade-in',
				role === 'user' && 'rounded-xl bg-zinc-800/30',
			)}
		>
			<div className="mb-2 flex items-center gap-2">
				<RoleIcon role={role} />
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					{roleLabels[role]}
				</p>
			</div>
			<div className="prose prose-invert ml-8 max-w-none text-sm leading-relaxed text-zinc-300 whitespace-pre-wrap">
				{content}
			</div>
		</div>
	);
}
