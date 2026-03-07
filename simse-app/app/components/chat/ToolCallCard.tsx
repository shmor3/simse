import clsx from 'clsx';
import { useState } from 'react';

interface ToolCallCardProps {
	name: string;
	input: string;
	output?: string;
	status: 'running' | 'completed' | 'error';
}

const statusConfig = {
	running: { label: 'Running', color: 'text-emerald-400' },
	completed: { label: 'Done', color: 'text-zinc-500' },
	error: { label: 'Error', color: 'text-red-400' },
};

function StatusDot({ status }: { status: ToolCallCardProps['status'] }) {
	if (status === 'running') {
		return (
			<span className="relative flex h-2 w-2">
				<span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75" />
				<span className="relative inline-flex h-2 w-2 rounded-full bg-emerald-400" />
			</span>
		);
	}
	if (status === 'completed') {
		return (
			<svg
				className="h-3.5 w-3.5 text-emerald-400"
				fill="none"
				viewBox="0 0 24 24"
				stroke="currentColor"
				strokeWidth={2.5}
			>
				<path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
			</svg>
		);
	}
	return (
		<svg
			className="h-3.5 w-3.5 text-red-400"
			fill="none"
			viewBox="0 0 24 24"
			stroke="currentColor"
			strokeWidth={2.5}
		>
			<path
				strokeLinecap="round"
				strokeLinejoin="round"
				d="M6 18L18 6M6 6l12 12"
			/>
		</svg>
	);
}

export default function ToolCallCard({
	name,
	input,
	output,
	status,
}: ToolCallCardProps) {
	const [expanded, setExpanded] = useState(false);
	const config = statusConfig[status];

	return (
		<div className="mx-auto max-w-3xl px-4 py-1.5 animate-fade-in">
			<div
				className={clsx(
					'overflow-hidden rounded-xl border bg-zinc-900/80 transition-colors',
					status === 'error'
						? 'border-red-500/20'
						: status === 'running'
							? 'border-emerald-400/20'
							: 'border-zinc-800',
				)}
			>
				{/* Header */}
				<button
					type="button"
					onClick={() => setExpanded((v) => !v)}
					className="flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-zinc-800/40"
				>
					<StatusDot status={status} />
					<span className="font-mono text-[12px] font-bold text-zinc-300">
						{name}
					</span>
					<span
						className={clsx(
							'ml-1 font-mono text-[10px] uppercase tracking-wider',
							config.color,
						)}
					>
						{config.label}
					</span>
					<svg
						className={clsx(
							'ml-auto h-3.5 w-3.5 text-zinc-600 transition-transform duration-200',
							expanded && 'rotate-180',
						)}
						fill="none"
						viewBox="0 0 24 24"
						stroke="currentColor"
						strokeWidth={2}
					>
						<path
							strokeLinecap="round"
							strokeLinejoin="round"
							d="M19 9l-7 7-7-7"
						/>
					</svg>
				</button>

				{/* Expanded content */}
				<div
					className={clsx(
						'grid transition-all duration-200',
						expanded ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]',
					)}
				>
					<div className="overflow-hidden">
						<div className="border-t border-zinc-800/50 px-4 py-3 space-y-3">
							{/* Input */}
							<div>
								<p className="mb-1 font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-600">
									Input
								</p>
								<pre className="overflow-x-auto rounded-lg bg-zinc-950/80 p-3 font-mono text-[12px] leading-relaxed text-zinc-400">
									{input}
								</pre>
							</div>

							{/* Output */}
							{output !== undefined && (
								<div>
									<p className="mb-1 font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-600">
										Output
									</p>
									<pre
										className={clsx(
											'overflow-x-auto rounded-lg bg-zinc-950/80 p-3 font-mono text-[12px] leading-relaxed',
											status === 'error' ? 'text-red-400' : 'text-zinc-400',
										)}
									>
										{output}
									</pre>
								</div>
							)}
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}
