import { ServiceRow } from '~/components/ServiceRow';
import { StatusBanner } from '~/components/StatusBanner';
import type { Route } from './+types/home';

interface ServiceStatus {
	id: string;
	name: string;
	status: 'up' | 'degraded' | 'down' | 'unknown';
	responseTimeMs: number | null;
	uptimePercent: number;
	dailyStatus: Array<{
		date: string;
		status: 'up' | 'degraded' | 'down';
	}>;
}

const SERVICE_NAMES: Record<string, string> = {
	api: 'API Gateway',
	auth: 'Auth',
	cdn: 'CDN',
	cloud: 'Cloud App',
	landing: 'Landing',
};

export function meta() {
	return [
		{ title: 'simse status' },
		{ name: 'description', content: 'Current status of simse services.' },
	];
}

export async function loader({ context }: Route.LoaderArgs) {
	const db = context.cloudflare.env.DB;

	const latest = await db
		.prepare(
			`SELECT service_id, status, response_time_ms, checked_at
			 FROM checks c1
			 WHERE checked_at = (
				 SELECT MAX(checked_at) FROM checks c2 WHERE c2.service_id = c1.service_id
			 )
			 ORDER BY service_id`,
		)
		.all<{
			service_id: string;
			status: string;
			response_time_ms: number | null;
			checked_at: string;
		}>();

	const daily = await db
		.prepare(
			`SELECT
				service_id,
				date(checked_at) as day,
				COUNT(*) as total,
				SUM(CASE WHEN status = 'down' THEN 1 ELSE 0 END) as down_count,
				SUM(CASE WHEN status = 'degraded' THEN 1 ELSE 0 END) as degraded_count
			 FROM checks
			 WHERE checked_at >= datetime('now', '-90 days')
			 GROUP BY service_id, date(checked_at)
			 ORDER BY service_id, day`,
		)
		.all<{
			service_id: string;
			day: string;
			total: number;
			down_count: number;
			degraded_count: number;
		}>();

	const latestMap = new Map(
		(latest.results ?? []).map((r) => [r.service_id, r]),
	);

	const dailyMap = new Map<
		string,
		Array<{ date: string; status: 'up' | 'degraded' | 'down' }>
	>();
	for (const row of daily.results ?? []) {
		if (!dailyMap.has(row.service_id)) {
			dailyMap.set(row.service_id, []);
		}
		let dayStatus: 'up' | 'degraded' | 'down' = 'up';
		if (row.down_count > 0) {
			dayStatus = row.down_count / row.total > 0.5 ? 'down' : 'degraded';
		} else if (row.degraded_count > 0) {
			dayStatus = 'degraded';
		}
		dailyMap.get(row.service_id)!.push({ date: row.day, status: dayStatus });
	}

	const services: ServiceStatus[] = Object.entries(SERVICE_NAMES).map(
		([id, name]) => {
			const check = latestMap.get(id);
			const days = dailyMap.get(id) ?? [];
			const totalDays = days.length || 1;
			const upDays = days.filter((d) => d.status === 'up').length;

			return {
				id,
				name,
				status: (check?.status as 'up' | 'degraded' | 'down') ?? 'unknown',
				responseTimeMs: check?.response_time_ms ?? null,
				uptimePercent:
					days.length > 0
						? Math.round((upDays / totalDays) * 10000) / 100
						: 100,
				dailyStatus: days,
			};
		},
	);

	const lastChecked =
		latest.results?.[0]?.checked_at ?? new Date().toISOString();

	return { services, lastChecked };
}

export default function StatusPage({ loaderData }: Route.ComponentProps) {
	const { services, lastChecked } = loaderData;

	return (
		<div className="mx-auto min-h-screen max-w-3xl px-4 py-12">
			<header className="mb-10 text-center">
				<p className="font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-zinc-500">
					SIMSE
				</p>
				<h1 className="mt-2 text-2xl font-bold tracking-tight text-white">
					System Status
				</h1>
			</header>

			<StatusBanner services={services} />

			<div className="mt-8 space-y-3">
				{services.map((service) => (
					<ServiceRow key={service.id} service={service} />
				))}
			</div>

			<footer className="mt-10 text-center text-xs text-zinc-600">
				Last checked{' '}
				{new Date(`${lastChecked}Z`).toLocaleString('en-US', {
					dateStyle: 'medium',
					timeStyle: 'short',
				})}
			</footer>
		</div>
	);
}
