import PageHeader from '~/components/layout/PageHeader';
import Card from '~/components/ui/Card';
import ProgressBar from '~/components/ui/ProgressBar';
import { createPaymentsClient } from '~/lib/payments.server';
import { getSession } from '~/lib/session.server';
import type { Route } from './+types/dashboard.usage';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) return { usage: null, dailyTokens: [], breakdown: [] };

	const env = context.cloudflare.env;
	const payments = createPaymentsClient({
		apiUrl: env.PAYMENTS_API_URL,
		apiSecret: env.PAYMENTS_API_SECRET,
	});

	const data = await payments.getUsage(session.userId);

	// Build 7-day chart data
	const dailyTokens: Array<{ day: string; tokens: number }> = [];
	for (let i = 6; i >= 0; i--) {
		const d = new Date();
		d.setDate(d.getDate() - i);
		const dayStr = d.toISOString().slice(0, 10);
		const label = d.toLocaleDateString('en', { weekday: 'short' });
		const found = data.recentUsage.find((r) => r.day === dayStr);
		dailyTokens.push({ day: label, tokens: found?.tokens ?? 0 });
	}

	const maxTokens = Math.max(1, ...dailyTokens.map((d) => d.tokens));

	return {
		usage: {
			used: Math.abs(data.balance),
			limit: 100_000,
			balance: data.balance,
		},
		dailyTokens: dailyTokens.map((d) => ({
			...d,
			pct: (d.tokens / maxTokens) * 100,
		})),
		breakdown: [] as Array<{
			category: string;
			tokens: number;
			pct: number;
		}>,
	};
}

export default function Usage({ loaderData }: Route.ComponentProps) {
	const { usage, dailyTokens } = loaderData;

	return (
		<>
			<PageHeader
				title="Usage"
				description="Monitor your token consumption and credit balance."
			/>

			{/* Usage meter */}
			<Card className="mt-8 p-6">
				<ProgressBar
					value={usage?.used ?? 0}
					max={usage?.limit ?? 100_000}
					label="Monthly usage"
				/>
				<div className="mt-4 flex items-center justify-between">
					<p className="text-sm text-zinc-500">
						{(((usage?.used ?? 0) / (usage?.limit ?? 1)) * 100).toFixed(1)}% of
						plan limit
					</p>
					<p className="font-mono text-sm text-emerald-400">
						${(usage?.balance ?? 0).toFixed(2)} credit
					</p>
				</div>
			</Card>

			{/* Daily token chart (CSS-only) */}
			<Card className="mt-6 p-6">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Daily tokens (7 days)
				</p>
				<div className="mt-6 flex items-end gap-2" style={{ height: 120 }}>
					{dailyTokens.map((d) => (
						<div
							key={d.day}
							className="group flex flex-1 flex-col items-center gap-2"
						>
							<div
								className="relative w-full flex flex-col items-center justify-end"
								style={{ height: 100 }}
							>
								{/* Tooltip */}
								<div className="pointer-events-none absolute -top-6 left-1/2 -translate-x-1/2 rounded bg-zinc-800 px-2 py-1 opacity-0 transition-opacity group-hover:opacity-100">
									<span className="whitespace-nowrap font-mono text-[10px] text-zinc-300">
										{d.tokens.toLocaleString()}
									</span>
								</div>
								<div
									className="w-full max-w-8 rounded-sm bg-emerald-400/20 transition-all group-hover:bg-emerald-400/40"
									style={{ height: `${Math.max(2, d.pct)}%` }}
								/>
							</div>
							<span className="font-mono text-[10px] text-zinc-600">
								{d.day}
							</span>
						</div>
					))}
				</div>
			</Card>

			{/* Usage breakdown */}
			<Card className="mt-6 overflow-hidden">
				<div className="border-b border-zinc-800 px-6 py-4">
					<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
						Usage breakdown
					</p>
				</div>
				<div className="p-6">
					<div className="flex flex-col items-center py-4">
						<div className="flex h-10 w-10 items-center justify-center rounded-full bg-zinc-800">
							<svg
								className="h-5 w-5 text-zinc-600"
								fill="none"
								viewBox="0 0 24 24"
								stroke="currentColor"
								strokeWidth={1.5}
							>
								<path
									strokeLinecap="round"
									strokeLinejoin="round"
									d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
								/>
							</svg>
						</div>
						<p className="mt-3 text-sm text-zinc-500">
							No usage data yet. Start a session to see your breakdown.
						</p>
					</div>
				</div>
			</Card>
		</>
	);
}
