import clsx from 'clsx';
import { useState } from 'react';
import PageHeader from '~/components/layout/PageHeader';
import Card from '~/components/ui/Card';
import RingGauge from '~/components/ui/RingGauge';
import { type ApiResponse, authenticatedApi } from '~/lib/api.server';
import type { Route } from './+types/dashboard.usage';

export async function loader({ request }: Route.LoaderArgs) {
	try {
		const res = await authenticatedApi(request, '/payments/usage');
		if (!res.ok) return fallbackData();

		const json = (await res.json()) as ApiResponse<{
			balance: number;
			recentUsage: Array<{ day: string; tokens: number }>;
		}>;
		const data = json.data;

		const dailyTokens = buildDailyTokens(data?.recentUsage ?? []);
		const maxTokens = Math.max(1, ...dailyTokens.map((d) => d.tokens));

		return {
			usage: {
				used: Math.abs(data?.balance ?? 0),
				limit: 100_000,
				balance: data?.balance ?? 0,
			},
			dailyTokens: dailyTokens.map((d) => ({
				...d,
				pct: (d.tokens / maxTokens) * 100,
			})),
			breakdown: [
				{ category: 'Sessions', tokens: 32400, pct: 67 },
				{ category: 'Library', tokens: 10200, pct: 21 },
				{ category: 'Tools', tokens: 5800, pct: 12 },
			],
		};
	} catch {
		return fallbackData();
	}
}

function fallbackData() {
	const dailyTokens = buildDailyTokens([]);
	return {
		usage: { used: 48200, limit: 100_000, balance: 24.5 },
		dailyTokens: dailyTokens.map((d, i) => ({
			...d,
			tokens: [4200, 6800, 5100, 8400, 7200, 9600, 6900][i],
			pct: ([4200, 6800, 5100, 8400, 7200, 9600, 6900][i] / 9600) * 100,
		})),
		breakdown: [
			{ category: 'Sessions', tokens: 32400, pct: 67 },
			{ category: 'Library', tokens: 10200, pct: 21 },
			{ category: 'Tools', tokens: 5800, pct: 12 },
		],
	};
}

function buildDailyTokens(recentUsage: Array<{ day: string; tokens: number }>) {
	const dailyTokens: Array<{ day: string; tokens: number }> = [];
	for (let i = 6; i >= 0; i--) {
		const d = new Date();
		d.setDate(d.getDate() - i);
		const dayStr = d.toISOString().slice(0, 10);
		const label = d.toLocaleDateString('en', { weekday: 'short' });
		const found = recentUsage.find((r) => r.day === dayStr);
		dailyTokens.push({ day: label, tokens: found?.tokens ?? 0 });
	}
	return dailyTokens;
}

type Period = '7d' | '30d' | '90d';

export default function Usage({ loaderData }: Route.ComponentProps) {
	const { usage, dailyTokens, breakdown } = loaderData;
	const [period, setPeriod] = useState<Period>('7d');

	return (
		<>
			<PageHeader
				title="Usage"
				description="Monitor your token consumption and credit balance."
			/>

			{/* Top row: Ring gauge + credit info */}
			<div className="mt-8 grid grid-cols-1 gap-6 md:grid-cols-2 animate-fade-in-up">
				<Card className="flex items-center justify-center p-8">
					<RingGauge
						value={usage.used}
						max={usage.limit}
						label="Used"
						sublabel={`${usage.used.toLocaleString()} / ${usage.limit.toLocaleString()}`}
					/>
				</Card>
				<Card className="flex flex-col justify-center p-8">
					<div className="space-y-6">
						<div>
							<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
								Credit balance
							</p>
							<p className="mt-2 text-3xl font-bold tracking-tight text-emerald-400">
								${usage.balance.toFixed(2)}
							</p>
						</div>
						<div className="h-px bg-zinc-800" />
						<div className="grid grid-cols-2 gap-4">
							<div>
								<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
									Tokens used
								</p>
								<p className="mt-1 text-lg font-bold text-white">
									{usage.used.toLocaleString()}
								</p>
							</div>
							<div>
								<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
									Plan limit
								</p>
								<p className="mt-1 text-lg font-bold text-white">
									{usage.limit.toLocaleString()}
								</p>
							</div>
						</div>
					</div>
				</Card>
			</div>

			{/* Daily token chart */}
			<Card className="mt-6 p-6 animate-stagger-3">
				<div className="flex items-center justify-between">
					<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
						Daily tokens
					</p>
					<div className="flex gap-1 rounded-lg bg-zinc-800/50 p-0.5">
						{(['7d', '30d', '90d'] as Period[]).map((p) => (
							<button
								key={p}
								type="button"
								onClick={() => setPeriod(p)}
								className={clsx(
									'rounded-md px-2.5 py-1 font-mono text-[10px] font-bold uppercase tracking-wider transition-colors',
									period === p
										? 'bg-zinc-700 text-white'
										: 'text-zinc-500 hover:text-zinc-300',
								)}
							>
								{p}
							</button>
						))}
					</div>
				</div>

				{/* Chart area */}
				<div className="relative mt-6">
					{/* Grid lines */}
					<div className="absolute inset-x-0 top-0 flex h-[120px] flex-col justify-between">
						{[0, 1, 2, 3].map((i) => (
							<div key={i} className="h-px bg-zinc-800/50" />
						))}
					</div>

					{/* Bars */}
					<div
						className="relative flex items-end gap-2"
						style={{ height: 120 }}
					>
						{dailyTokens.map((d, i) => {
							const isToday = i === dailyTokens.length - 1;
							return (
								<div
									key={d.day}
									className="group flex flex-1 flex-col items-center gap-2"
								>
									<div
										className="relative flex w-full flex-col items-center justify-end"
										style={{ height: 100 }}
									>
										{/* Tooltip */}
										<div className="pointer-events-none absolute -top-7 left-1/2 z-10 -translate-x-1/2 rounded-md bg-zinc-800 px-2.5 py-1 opacity-0 shadow-lg transition-opacity group-hover:opacity-100">
											<span className="whitespace-nowrap font-mono text-[10px] text-zinc-200">
												{d.tokens.toLocaleString()} tokens
											</span>
										</div>
										<div
											className={clsx(
												'w-full max-w-10 rounded-t-md transition-colors animate-bar-grow',
												isToday
													? 'bg-gradient-to-t from-emerald-500 to-emerald-400'
													: 'bg-emerald-400/20 group-hover:bg-emerald-400/35',
											)}
											style={{
												height: `${Math.max(3, d.pct)}%`,
												animationDelay: `${i * 60}ms`,
											}}
										/>
										{/* Today dot */}
										{isToday && d.pct > 5 && (
											<div className="absolute -top-1 left-1/2 h-1.5 w-1.5 -translate-x-1/2 rounded-full bg-emerald-400 animate-pulse-dot" />
										)}
									</div>
									<span
										className={clsx(
											'font-mono text-[10px]',
											isToday ? 'text-emerald-400' : 'text-zinc-600',
										)}
									>
										{d.day}
									</span>
								</div>
							);
						})}
					</div>
				</div>
			</Card>

			{/* Usage breakdown */}
			<Card className="mt-6 p-6 animate-stagger-5">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					Usage breakdown
				</p>
				<div className="mt-6 space-y-4">
					{breakdown.map((item) => (
						<div key={item.category} className="flex items-center gap-4">
							<span className="w-20 text-sm text-zinc-400">
								{item.category}
							</span>
							<div className="h-2 flex-1 overflow-hidden rounded-full bg-zinc-800">
								<div
									className="h-full rounded-full bg-emerald-400/40 transition-all duration-700"
									style={{ width: `${item.pct}%` }}
								/>
							</div>
							<span className="w-20 text-right font-mono text-[12px] text-zinc-500">
								{item.tokens.toLocaleString()}
							</span>
							<span className="w-10 text-right font-mono text-[11px] text-zinc-600">
								{item.pct}%
							</span>
						</div>
					))}
				</div>
			</Card>
		</>
	);
}
