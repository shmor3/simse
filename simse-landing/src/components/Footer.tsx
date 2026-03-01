export default function Footer() {
	return (
		<footer className="border-t border-zinc-800/60 px-6 py-8">
			<div className="mx-auto flex max-w-5xl flex-col items-center justify-between gap-4 sm:flex-row">
				<div className="flex items-center gap-3">
					<span className="font-mono text-sm font-medium text-zinc-400">
						simse
					</span>
					<span className="text-zinc-800">/</span>
					<span className="font-mono text-xs text-zinc-600">MIT License</span>
				</div>

				<a
					href="https://github.com/restaadiputra/simse"
					target="_blank"
					rel="noopener noreferrer"
					className="group flex items-center gap-2 font-mono text-xs text-zinc-600 transition-colors hover:text-zinc-300"
				>
					<svg
						className="size-4 transition-colors group-hover:text-zinc-300"
						viewBox="0 0 24 24"
						fill="currentColor"
					>
						<path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z" />
					</svg>
					<span>GitHub</span>
					<svg
						className="size-3 text-zinc-700 transition-transform group-hover:translate-x-0.5"
						viewBox="0 0 12 12"
						fill="none"
						stroke="currentColor"
						strokeWidth="1.5"
					>
						<path strokeLinecap="round" strokeLinejoin="round" d="M2.5 9.5l7-7M9.5 2.5H4.5M9.5 2.5v5" />
					</svg>
				</a>
			</div>
		</footer>
	);
}
