import clsx from 'clsx';

interface MessageBubbleProps {
	role: 'user' | 'assistant' | 'system';
	content: string;
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
				'mx-auto max-w-3xl px-4 py-4',
				role === 'user' && 'bg-zinc-800/30 rounded-xl',
			)}
		>
			<p className="mb-1.5 font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
				{roleLabels[role]}
			</p>
			<div className="prose prose-invert max-w-none text-zinc-300 text-sm leading-relaxed whitespace-pre-wrap">
				{content}
			</div>
		</div>
	);
}
