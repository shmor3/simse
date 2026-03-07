import { Link } from 'react-router';

interface ChecklistItem {
	label: string;
	done: boolean;
	to: string;
}

interface WelcomeHeroProps {
	userName: string;
	checklist: ChecklistItem[];
}

function getGreeting(): string {
	const hour = new Date().getHours();
	if (hour < 12) return 'Good morning';
	if (hour < 17) return 'Good afternoon';
	return 'Good evening';
}

export default function WelcomeHero({ userName, checklist }: WelcomeHeroProps) {
	const completedCount = checklist.filter((i) => i.done).length;
	const allDone = completedCount === checklist.length;
	const pct =
		checklist.length > 0 ? (completedCount / checklist.length) * 100 : 0;

	// Progress ring
	const ringSize = 36;
	const ringStroke = 3;
	const ringRadius = (ringSize - ringStroke) / 2;
	const ringCircumference = 2 * Math.PI * ringRadius;
	const ringOffset = ringCircumference - (pct / 100) * ringCircumference;

	const firstName = userName.split(' ')[0] || 'there';

	return (
		<div className="relative overflow-hidden rounded-xl border border-zinc-800 bg-zinc-900">
			{/* Gradient top accent */}
			<div className="h-px gradient-border animate-gradient-shift" />

			{/* Subtle background glow */}
			<div className="pointer-events-none absolute -top-24 -right-24 h-48 w-48 rounded-full bg-emerald-400/[0.03] blur-3xl" />
			<div className="pointer-events-none absolute -bottom-16 -left-16 h-32 w-32 rounded-full bg-cyan-400/[0.02] blur-3xl" />

			<div className="relative p-6">
				<div className="flex items-start justify-between">
					<div>
						<h1 className="text-xl font-bold tracking-tight text-white">
							{getGreeting()},{' '}
							<span className="gradient-text">{firstName}</span>
						</h1>
						<p className="mt-1 text-sm text-zinc-500">
							{allDone
								? 'Your workspace is ready. Start building.'
								: 'Complete your setup to get the most out of simse.'}
						</p>
					</div>

					{!allDone && (
						<div className="flex items-center gap-2.5">
							<svg
								width={ringSize}
								height={ringSize}
								viewBox={`0 0 ${ringSize} ${ringSize}`}
								className="-rotate-90"
							>
								<circle
									cx={ringSize / 2}
									cy={ringSize / 2}
									r={ringRadius}
									fill="none"
									stroke="#27272a"
									strokeWidth={ringStroke}
								/>
								<circle
									cx={ringSize / 2}
									cy={ringSize / 2}
									r={ringRadius}
									fill="none"
									stroke="#34d399"
									strokeWidth={ringStroke}
									strokeLinecap="round"
									strokeDasharray={ringCircumference}
									strokeDashoffset={ringOffset}
									className="transition-all duration-700"
								/>
							</svg>
							<span className="font-mono text-[11px] text-zinc-500">
								{completedCount}/{checklist.length}
							</span>
						</div>
					)}
				</div>

				{/* Checklist */}
				{!allDone && (
					<div className="mt-5 grid grid-cols-1 gap-2 sm:grid-cols-2">
						{checklist.map((item) => (
							<Link
								key={item.label}
								to={item.to}
								className="group flex items-center gap-3 rounded-lg border border-zinc-800/50 bg-zinc-950/50 px-3.5 py-2.5 transition-all hover:border-zinc-700 hover:bg-zinc-800/30"
							>
								<div
									className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full border ${
										item.done
											? 'border-emerald-400 bg-emerald-400'
											: 'border-zinc-600 group-hover:border-zinc-500'
									}`}
								>
									{item.done && (
										<svg
											className="h-3 w-3 text-zinc-950"
											fill="none"
											viewBox="0 0 24 24"
											stroke="currentColor"
											strokeWidth={3}
										>
											<path
												strokeLinecap="round"
												strokeLinejoin="round"
												d="M5 13l4 4L19 7"
											/>
										</svg>
									)}
								</div>
								<span
									className={`text-sm ${
										item.done
											? 'text-zinc-600 line-through'
											: 'text-zinc-300 group-hover:text-white'
									}`}
								>
									{item.label}
								</span>
								{!item.done && (
									<svg
										className="ml-auto h-3.5 w-3.5 text-zinc-700 transition-all group-hover:translate-x-0.5 group-hover:text-zinc-500"
										fill="none"
										viewBox="0 0 24 24"
										stroke="currentColor"
										strokeWidth={2}
									>
										<path
											strokeLinecap="round"
											strokeLinejoin="round"
											d="M9 5l7 7-7 7"
										/>
									</svg>
								)}
							</Link>
						))}
					</div>
				)}
			</div>
		</div>
	);
}
