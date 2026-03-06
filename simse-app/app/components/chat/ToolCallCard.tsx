import clsx from 'clsx';
import { useState } from 'react';

interface ToolCallCardProps {
	name: string;
	input: string;
	output?: string;
	status: 'running' | 'completed' | 'error';
}

function StatusDot({ status }: { status: ToolCallCardProps['status'] }) {
	if (status === 'running') {
		return (
			<span className="relative flex h-2.5 w-2.5">
				<span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75" />
				<span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-emerald-400" />
			</span>
		);
	}
	if (status === 'completed') {
		return <span className="h-2.5 w-2.5 rounded-full bg-emerald-400" />;
	}
	return <span className="h-2.5 w-2.5 rounded-full bg-red-400" />;
}

export default function ToolCallCard({
	name,
	input,
	output,
	status,
}: ToolCallCardProps) {
	const [expanded, setExpanded] = useState(false);

	return (
		<div className="mx-auto max-w-3xl px-4 py-2">
			<div className="overflow-hidden rounded-xl border border-zinc-800 bg-zinc-900">
				{/* Header (always visible) */}
				<button
					type="button"
					onClick={() => setExpanded((v) => !v)}
					className="flex w-full items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-zinc-800/40"
				>
					<StatusDot status={status} />
					<span className="font-mono text-[12px] font-bold text-zinc-300">
						{name}
					</span>
					<svg
						className={clsx(
							'ml-auto h-3.5 w-3.5 text-zinc-600 transition-transform',
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
				{expanded && (
					<div className="border-t border-zinc-800 px-4 py-3 space-y-3">
						{/* Input */}
						<div>
							<p className="mb-1 font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
								Input
							</p>
							<pre className="overflow-x-auto rounded-lg bg-zinc-950 p-3 font-mono text-[12px] leading-relaxed text-zinc-400">
								{input}
							</pre>
						</div>

						{/* Output */}
						{output !== undefined && (
							<div>
								<p className="mb-1 font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
									Output
								</p>
								<pre
									className={clsx(
										'overflow-x-auto rounded-lg bg-zinc-950 p-3 font-mono text-[12px] leading-relaxed',
										status === 'error' ? 'text-red-400' : 'text-zinc-400',
									)}
								>
									{output}
								</pre>
							</div>
						)}
					</div>
				)}
			</div>
		</div>
	);
}
