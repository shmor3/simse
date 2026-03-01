const features = [
	{
		title: 'ACP Client',
		description:
			'Connect to AI backends via Agent Client Protocol. Streaming, sessions, permissions.',
		icon: (
			<svg
				className="size-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				strokeWidth="1.5"
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M7.5 21L3 16.5m0 0L7.5 12M3 16.5h13.5m0-13.5L21 7.5m0 0L16.5 12M21 7.5H7.5"
				/>
			</svg>
		),
		label: 'protocol',
	},
	{
		title: 'MCP Server',
		description:
			'Expose and consume tools, resources, and prompts via Model Context Protocol.',
		icon: (
			<svg
				className="size-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				strokeWidth="1.5"
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M5.25 14.25h13.5m-13.5 0a3 3 0 01-3-3m3 3a3 3 0 100 6h13.5a3 3 0 100-6m-16.5-3a3 3 0 013-3h13.5a3 3 0 013 3m-19.5 0a4.5 4.5 0 01.9-2.7L5.737 5.1a3.375 3.375 0 012.7-1.35h7.126c1.062 0 2.062.5 2.7 1.35l2.587 3.45a4.5 4.5 0 01.9 2.7"
				/>
			</svg>
		),
		label: 'server',
	},
	{
		title: 'Agentic Loop',
		description:
			'Multi-turn tool-use loop with auto-compaction, stream retry, and doom-loop detection.',
		icon: (
			<svg
				className="size-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				strokeWidth="1.5"
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0l3.181 3.183a8.25 8.25 0 0013.803-3.7M4.031 9.865a8.25 8.25 0 0113.803-3.7l3.181 3.182"
				/>
			</svg>
		),
		label: 'loop',
	},
	{
		title: 'Vector Memory',
		description:
			'File-backed vector store with cosine search, deduplication, and compression.',
		icon: (
			<svg
				className="size-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				strokeWidth="1.5"
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M20.25 6.375c0 2.278-3.694 4.125-8.25 4.125S3.75 8.653 3.75 6.375m16.5 0c0-2.278-3.694-4.125-8.25-4.125S3.75 4.097 3.75 6.375m16.5 0v11.25c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125V6.375m16.5 3.75c0 2.278-3.694 4.125-8.25 4.125s-8.25-1.847-8.25-4.125"
				/>
			</svg>
		),
		label: 'storage',
	},
	{
		title: 'Virtual Filesystem',
		description:
			'In-memory filesystem with history, diffing, snapshots, and disk persistence.',
		icon: (
			<svg
				className="size-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				strokeWidth="1.5"
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M2.25 12.75V12A2.25 2.25 0 014.5 9.75h15A2.25 2.25 0 0121.75 12v.75m-8.69-6.44l-2.12-2.12a1.5 1.5 0 00-1.061-.44H4.5A2.25 2.25 0 002.25 6v12a2.25 2.25 0 002.25 2.25h15A2.25 2.25 0 0021.75 18V9a2.25 2.25 0 00-2.25-2.25h-5.379a1.5 1.5 0 01-1.06-.44z"
				/>
			</svg>
		),
		label: 'filesystem',
	},
	{
		title: 'Resilience',
		description:
			'Circuit breaker, health monitor, retry with exponential backoff and jitter.',
		icon: (
			<svg
				className="size-5"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				strokeWidth="1.5"
			>
				<path
					strokeLinecap="round"
					strokeLinejoin="round"
					d="M9 12.75L11.25 15 15 9.75m-3-7.036A11.959 11.959 0 013.598 6 11.99 11.99 0 003 9.749c0 5.592 3.824 10.29 9 11.623 5.176-1.332 9-6.03 9-11.622 0-1.31-.21-2.571-.598-3.751h-.152c-3.196 0-6.1-1.248-8.25-3.285z"
				/>
			</svg>
		),
		label: 'safety',
	},
] as const;

export default function Features() {
	return (
		<section className="relative px-6 py-8">
			{/* Section header */}
			<div className="mx-auto max-w-5xl">
				{/* Card grid */}
				<div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
					{features.map((feature, i) => (
						<article
							key={feature.title}
							className="group relative rounded-xl border border-zinc-800/80 bg-zinc-900/50 p-5 opacity-0 animate-fade-in-up [animation-fill-mode:forwards] transition-all duration-300 hover:border-zinc-700 hover:bg-zinc-900/80"
							style={{ animationDelay: `${i * 80}ms` }}
						>
							{/* Corner glow on hover */}
							<div className="pointer-events-none absolute -inset-px rounded-xl opacity-0 transition-opacity duration-300 group-hover:opacity-100">
								<div className="absolute top-0 left-0 h-px w-16 bg-gradient-to-r from-emerald-500/40 to-transparent" />
								<div className="absolute top-0 left-0 h-16 w-px bg-gradient-to-b from-emerald-500/40 to-transparent" />
							</div>

							<div className="relative">
								{/* Icon + label row */}
								<div className="mb-4 flex items-center gap-3">
									<div className="flex size-9 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900 text-zinc-400 transition-colors group-hover:border-emerald-500/30 group-hover:text-emerald-400">
										{feature.icon}
									</div>
									<span className="font-mono text-[10px] tracking-widest text-zinc-700 uppercase transition-colors group-hover:text-zinc-600">
										{feature.label}
									</span>
								</div>

								<h3 className="text-base font-semibold text-zinc-200">
									{feature.title}
								</h3>
								<p className="mt-2 text-sm leading-relaxed text-zinc-500">
									{feature.description}
								</p>
							</div>
						</article>
					))}
				</div>
			</div>
		</section>
	);
}
