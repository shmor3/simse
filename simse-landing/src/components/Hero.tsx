import WaitlistForm from './WaitlistForm';

export default function Hero() {
	return (
		<section className="relative flex min-h-[85vh] flex-col items-center justify-center overflow-hidden px-6 py-32">
			{/* Radial glow behind the headline */}
			<div className="pointer-events-none absolute top-1/3 left-1/2 -translate-x-1/2 -translate-y-1/2">
				<div className="h-[500px] w-[800px] rounded-full bg-emerald-500/[0.03] blur-[120px]" />
			</div>

			<div className="relative z-10 flex max-w-2xl flex-col items-center text-center">
				{/* Badge */}
				<div
					className="animate-fade-in mb-8 flex items-center gap-2 rounded-full border border-zinc-800 bg-zinc-900/80 px-4 py-1.5 font-mono text-xs tracking-wide text-zinc-500"
					style={{ animationDelay: '100ms' }}
				>
					<span className="inline-block size-1.5 rounded-full bg-emerald-500 animate-pulse" />
					<span>Currently in development</span>
				</div>

				{/* Headline */}
				<h1
					className="animate-fade-in-up text-5xl leading-[1.1] font-bold tracking-tight text-zinc-50 sm:text-6xl md:text-7xl"
					style={{ animationDelay: '200ms' }}
				>
					Orchestrate AI{' '}
					<span className="bg-gradient-to-r from-emerald-400 to-emerald-300 bg-clip-text text-transparent">
						Workflows
					</span>
				</h1>

				{/* Description */}
				<p
					className="animate-fade-in-up mt-6 max-w-lg text-lg leading-relaxed text-zinc-400 sm:text-xl"
					style={{ animationDelay: '350ms' }}
				>
					A modular pipeline framework connecting AI backends via{' '}
					<span className="font-mono text-zinc-300">ACP</span>, exposing tools
					through{' '}
					<span className="font-mono text-zinc-300">MCP</span>, backed by{' '}
					<span className="font-mono text-zinc-300">vector memory</span>.
				</p>

				{/* Waitlist form */}
				<div
					className="animate-fade-in-up mt-10 w-full flex justify-center"
					style={{ animationDelay: '500ms' }}
				>
					<WaitlistForm />
				</div>

				{/* Social proof hint */}
				<p
					className="animate-fade-in mt-6 font-mono text-xs tracking-wide text-zinc-600"
					style={{ animationDelay: '650ms' }}
				>
					Open source &middot; TypeScript &middot; Zero dependencies on AI SDKs
				</p>
			</div>
		</section>
	);
}
