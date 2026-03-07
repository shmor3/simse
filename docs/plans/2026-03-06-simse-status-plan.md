# simse-status Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a status page at status.simse.dev that monitors 7 web services via cron health checks with 90-day D1-backed history.

**Architecture:** Cloudflare Pages app (React Router v7 SSR) with a cron trigger that checks all service /health endpoints every minute. Results stored in D1, displayed as a minimal status page with per-service uptime bars.

**Tech Stack:** React Router v7, @react-router/cloudflare, Tailwind CSS v4, D1, Cloudflare Pages + Cron Triggers, Biome

---

### Task 1: Add /health endpoint to simse-app

**Files:**
- Modify: `simse-app/worker.ts:17-19` (add early return before requestHandler)

**Step 1: Add health check in worker.ts**

In `simse-app/worker.ts`, add an early return for `/health` before the `requestHandler` call. Insert at line 18, before `const start = Date.now();`:

```typescript
const url = new URL(request.url);
if (url.pathname === '/health') {
	return new Response(JSON.stringify({ ok: true }), {
		headers: { 'Content-Type': 'application/json' },
	});
}
```

Then update the existing `const url = new URL(request.url);` on the analytics line (around line 27) to reuse the already-declared `url`. Just remove the second `const url` declaration since `url` is already in scope.

The full fetch handler body should be:
```typescript
async fetch(request, env, ctx) {
	const url = new URL(request.url);
	if (url.pathname === '/health') {
		return new Response(JSON.stringify({ ok: true }), {
			headers: { 'Content-Type': 'application/json' },
		});
	}

	const start = Date.now();
	const response = await requestHandler(request, {
		cloudflare: { env, ctx },
	});
	const latencyMs = Date.now() - start;

	// biome-ignore lint/suspicious/noExplicitAny: Cloudflare cf object not typed on Request
	const cf = (request as any).cf;

	ctx.waitUntil(
		// ... analytics unchanged, but remove `const url = new URL(request.url);`
	);

	return response;
},
```

**Step 2: Verify**

Run: `cd simse-app && npx tsc --noEmit`
Expected: No type errors

**Step 3: Commit**

```bash
git add simse-app/worker.ts
git commit -m "feat(simse-app): add /health endpoint"
```

---

### Task 2: Add /health endpoint to simse-landing

**Files:**
- Modify: `simse-landing/worker.ts:21-23` (add early return before requestHandler)

**Step 1: Add health check in worker.ts**

Same pattern as Task 1. In `simse-landing/worker.ts`, add an early return for `/health` before the `requestHandler` call. Insert at line 22:

```typescript
const url = new URL(request.url);
if (url.pathname === '/health') {
	return new Response(JSON.stringify({ ok: true }), {
		headers: { 'Content-Type': 'application/json' },
	});
}
```

Remove the duplicate `const url = new URL(request.url);` from the analytics section below.

**Step 2: Verify**

Run: `cd simse-landing && npx tsc --noEmit`
Expected: No type errors

**Step 3: Commit**

```bash
git add simse-landing/worker.ts
git commit -m "feat(simse-landing): add /health endpoint"
```

---

### Task 3: Scaffold simse-status project

**Files:**
- Create: `simse-status/package.json`
- Create: `simse-status/tsconfig.json`
- Create: `simse-status/vite.config.ts`
- Create: `simse-status/react-router.config.ts`
- Create: `simse-status/biome.json`
- Create: `simse-status/wrangler.toml`
- Create: `simse-status/worker.ts`
- Create: `simse-status/app/entry.server.tsx`
- Create: `simse-status/app/root.tsx`
- Create: `simse-status/app/styles/app.css`
- Create: `simse-status/app/routes.ts`

**Step 1: Create package.json**

```json
{
	"name": "simse-status",
	"private": true,
	"type": "module",
	"scripts": {
		"dev": "react-router dev",
		"build": "react-router build",
		"start": "wrangler pages dev",
		"preview": "wrangler pages dev ./build/client",
		"typecheck": "react-router typegen && tsc --noEmit",
		"lint": "biome check .",
		"lint:fix": "biome check --write .",
		"cf-typegen": "wrangler types",
		"db:migrate": "wrangler d1 migrations apply simse-status-db --local",
		"db:migrate:prod": "wrangler d1 migrations apply simse-status-db --remote"
	},
	"dependencies": {
		"@fontsource-variable/dm-sans": "^5.2.8",
		"@fontsource/space-mono": "^5.2.9",
		"@react-router/cloudflare": "^7.13.1",
		"isbot": "^5.1.27",
		"react": "^19.2.0",
		"react-dom": "^19.2.0",
		"react-router": "^7.13.1"
	},
	"devDependencies": {
		"@biomejs/biome": "^2.3.12",
		"@cloudflare/workers-types": "^4.20260305.0",
		"@react-router/dev": "^7.13.1",
		"@tailwindcss/vite": "^4.2.1",
		"@types/react": "^19.0.0",
		"@types/react-dom": "^19.0.0",
		"tailwindcss": "^4.2.1",
		"typescript": "^5.7.0",
		"vite": "^7.3.0",
		"vite-tsconfig-paths": "^6.1.1",
		"wrangler": "^4.0.0"
	}
}
```

**Step 2: Create wrangler.toml**

```toml
name = "simse-status"
compatibility_date = "2026-03-01"
pages_build_output_dir = "./build/client"

routes = [{ pattern = "status.simse.dev", custom_domain = true }]

[[d1_databases]]
binding = "DB"
database_name = "simse-status-db"
database_id = "placeholder-create-via-wrangler"

[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"

# Cron trigger: check all services every minute
[triggers]
crons = ["* * * * *"]
```

**Step 3: Create tsconfig.json**

```json
{
	"compilerOptions": {
		"target": "ES2022",
		"module": "ESNext",
		"moduleResolution": "bundler",
		"jsx": "react-jsx",
		"strict": true,
		"esModuleInterop": true,
		"skipLibCheck": true,
		"forceConsistentCasingInFileNames": true,
		"resolveJsonModule": true,
		"isolatedModules": true,
		"noEmit": true,
		"types": ["@cloudflare/workers-types", "vite/client"],
		"paths": {
			"~/*": ["./app/*"]
		},
		"rootDirs": [".", "./.react-router/types"]
	},
	"include": ["app", "worker.ts", ".react-router/types/**/*"]
}
```

**Step 4: Create vite.config.ts**

```typescript
import { reactRouter } from '@react-router/dev/vite';
import { cloudflareDevProxy } from '@react-router/dev/vite/cloudflare';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import tsconfigPaths from 'vite-tsconfig-paths';

export default defineConfig({
	plugins: [
		cloudflareDevProxy(),
		reactRouter(),
		tailwindcss(),
		tsconfigPaths(),
	],
});
```

**Step 5: Create react-router.config.ts**

```typescript
import type { Config } from '@react-router/dev/config';

export default {
	ssr: true,
} satisfies Config;
```

**Step 6: Create biome.json**

```json
{
	"$schema": "https://biomejs.dev/schemas/2.4.4/schema.json",
	"vcs": {
		"enabled": true,
		"clientKind": "git",
		"useIgnoreFile": true
	},
	"files": {
		"includes": ["**", "!!**/dist", "!!.react-router", "!!**/*.css"]
	},
	"formatter": {
		"enabled": true,
		"indentStyle": "tab"
	},
	"linter": {
		"enabled": true,
		"rules": {
			"recommended": true,
			"style": {
				"noNonNullAssertion": "off"
			}
		}
	},
	"javascript": {
		"formatter": {
			"quoteStyle": "single"
		}
	},
	"assist": {
		"enabled": true,
		"actions": {
			"source": {
				"organizeImports": "on"
			}
		}
	}
}
```

**Step 7: Create worker.ts**

```typescript
import { createRequestHandler } from 'react-router';

declare module 'react-router' {
	export interface AppLoadContext {
		cloudflare: {
			env: {
				DB: D1Database;
				ANALYTICS: AnalyticsEngineDataset;
			};
			ctx: ExecutionContext;
		};
	}
}

interface Env {
	DB: D1Database;
	ANALYTICS: AnalyticsEngineDataset;
}

const requestHandler = createRequestHandler(
	() => import('virtual:react-router/server-build'),
	import.meta.env.MODE,
);

async function checkService(
	service: { id: string; name: string; url: string },
	db: D1Database,
): Promise<void> {
	const start = Date.now();
	let status: 'up' | 'degraded' | 'down' = 'down';
	let statusCode: number | null = null;
	let error: string | null = null;

	try {
		const controller = new AbortController();
		const timeout = setTimeout(() => controller.abort(), 10_000);
		const res = await fetch(service.url, { signal: controller.signal });
		clearTimeout(timeout);
		statusCode = res.status;
		const elapsed = Date.now() - start;

		if (res.ok) {
			status = elapsed > 5000 ? 'degraded' : 'up';
		} else {
			status = 'down';
		}
	} catch (err) {
		error = err instanceof Error ? err.message : 'Unknown error';
		status = 'down';
	}

	const responseTimeMs = Date.now() - start;

	await db
		.prepare(
			'INSERT INTO checks (service_id, status, response_time_ms, status_code, error) VALUES (?, ?, ?, ?, ?)',
		)
		.bind(service.id, status, responseTimeMs, statusCode, error)
		.run();
}

const SERVICES = [
	{ id: 'api', name: 'API Gateway', url: 'https://api.simse.dev/health' },
	{ id: 'auth', name: 'Auth', url: 'https://auth.simse.dev/health' },
	{ id: 'cdn', name: 'CDN', url: 'https://cdn.simse.dev/health' },
	{ id: 'cloud', name: 'Cloud App', url: 'https://app.simse.dev/health' },
	{ id: 'landing', name: 'Landing', url: 'https://simse.dev/health' },
];

export default {
	async fetch(request: Request, env: Env, ctx: ExecutionContext) {
		const url = new URL(request.url);
		if (url.pathname === '/health') {
			return new Response(JSON.stringify({ ok: true }), {
				headers: { 'Content-Type': 'application/json' },
			});
		}

		const start = Date.now();
		const response = await requestHandler(request, {
			cloudflare: { env, ctx },
		});
		const latencyMs = Date.now() - start;

		// biome-ignore lint/suspicious/noExplicitAny: Cloudflare cf object not typed on Request
		const cf = (request as any).cf;

		ctx.waitUntil(
			Promise.resolve(
				env.ANALYTICS.writeDataPoint({
					indexes: ['simse-status'],
					blobs: [
						request.method,
						url.pathname,
						String(response.status),
						'simse-status',
						'',
						'',
						cf?.country ?? '',
						cf?.city ?? '',
						cf?.continent ?? '',
						(request.headers.get('User-Agent') ?? '').slice(0, 256),
						request.headers.get('Referer') ?? '',
						response.headers.get('Content-Type') ?? '',
						request.headers.get('Cf-Ray') ?? '',
					],
					doubles: [
						latencyMs,
						response.status,
						Number(request.headers.get('Content-Length') ?? 0),
						Number(response.headers.get('Content-Length') ?? 0),
						Number(cf?.colo ?? 0),
					],
				}),
			),
		);

		return response;
	},

	async scheduled(_event: ScheduledEvent, env: Env, ctx: ExecutionContext) {
		const checks = SERVICES.map((s) => checkService(s, env.DB));
		ctx.waitUntil(
			Promise.allSettled(checks).then(() =>
				env.DB
					.prepare(
						"DELETE FROM checks WHERE checked_at < datetime('now', '-90 days')",
					)
					.run(),
			),
		);
	},
} satisfies ExportedHandler<Env>;
```

Note: `payments` and `mailer` are excluded from SERVICES because they don't have public custom domains. If they get public URLs later, add them to the array.

**Step 8: Create app/entry.server.tsx**

Copy from simse-app — identical file:

```tsx
import { isbot } from 'isbot';
import { renderToReadableStream } from 'react-dom/server';
import type { AppLoadContext, EntryContext } from 'react-router';
import { ServerRouter } from 'react-router';

export default async function handleRequest(
	request: Request,
	responseStatusCode: number,
	responseHeaders: Headers,
	routerContext: EntryContext,
	_loadContext: AppLoadContext,
) {
	const userAgent = request.headers.get('user-agent');

	const stream = await renderToReadableStream(
		<ServerRouter context={routerContext} url={request.url} />,
		{
			signal: request.signal,
			onError(error: unknown) {
				console.error(error);
				responseStatusCode = 500;
			},
		},
	);

	if (userAgent && isbot(userAgent)) {
		await stream.allReady;
	}

	responseHeaders.set('Content-Type', 'text/html');

	return new Response(stream, {
		headers: responseHeaders,
		status: responseStatusCode,
	});
}
```

**Step 9: Create app/styles/app.css**

```css
@import "tailwindcss";

@theme {
	--font-sans: "DM Sans Variable", system-ui, sans-serif;
	--font-mono: "Space Mono", ui-monospace, monospace;
}

@layer base {
	html {
		-webkit-font-smoothing: antialiased;
		-moz-osx-font-smoothing: grayscale;
	}

	body {
		font-family: var(--font-sans);
		background-color: #0a0a0b;
		color: #e4e4e7;
	}
}

@keyframes fade-in {
	from { opacity: 0; }
	to { opacity: 1; }
}

@keyframes fade-in-up {
	from { opacity: 0; transform: translateY(10px); }
	to { opacity: 1; transform: translateY(0); }
}

@layer utilities {
	.animate-fade-in {
		animation: fade-in 0.6s ease-out both;
	}

	.animate-fade-in-up {
		animation: fade-in-up 0.5s cubic-bezier(0.16, 1, 0.3, 1) both;
	}
}
```

**Step 10: Create app/routes.ts**

```typescript
import { type RouteConfig, index } from '@react-router/dev/routes';

export default [index('routes/home.tsx')] satisfies RouteConfig;
```

**Step 11: Create app/root.tsx**

```tsx
import '@fontsource-variable/dm-sans';
import '@fontsource/space-mono';
import {
	isRouteErrorResponse,
	Links,
	Meta,
	Outlet,
	Scripts,
	ScrollRestoration,
} from 'react-router';
import type { Route } from './+types/root';
import './styles/app.css';

export function Layout({ children }: { children: React.ReactNode }) {
	return (
		<html lang="en">
			<head>
				<meta charSet="utf-8" />
				<meta name="viewport" content="width=device-width, initial-scale=1" />
				<link
					rel="icon"
					type="image/svg+xml"
					href="data:image/svg+xml,%3Csvg viewBox='0 0 100 100' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cdefs%3E%3CclipPath id='h'%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5'/%3E%3C/clipPath%3E%3C/defs%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5' fill='none' stroke='white' stroke-width='5'/%3E%3Cg clip-path='url(%23h)'%3E%3Cpath d='M44,-10 C90,15 94,35 50,50 C6,65 10,85 56,110' stroke='white' stroke-width='8' stroke-linecap='round' fill='none'/%3E%3C/g%3E%3C/svg%3E"
				/>
				<Meta />
				<Links />
			</head>
			<body>
				{children}
				<ScrollRestoration />
				<Scripts />
			</body>
		</html>
	);
}

export default function App() {
	return <Outlet />;
}

export function ErrorBoundary({ error }: Route.ErrorBoundaryProps) {
	let heading = 'Something went wrong';
	let message = 'An unexpected error occurred.';

	if (isRouteErrorResponse(error)) {
		heading = `${error.status} ${error.statusText}`;
		message = error.data?.toString() ?? message;
	} else if (error instanceof Error) {
		message = error.message;
	}

	return (
		<div className="flex min-h-screen items-center justify-center bg-[#0a0a0b]">
			<div className="max-w-md text-center">
				<p className="font-mono text-[11px] font-bold uppercase tracking-[0.35em] text-zinc-600">
					SIMSE STATUS
				</p>
				<h1 className="mt-8 text-4xl font-bold tracking-tight text-white">
					{heading}
				</h1>
				<p className="mt-4 text-zinc-400">{message}</p>
				<a
					href="/"
					className="mt-8 inline-block rounded-lg bg-emerald-400 px-6 py-3 font-mono text-sm font-bold text-zinc-950 no-underline transition-colors hover:bg-emerald-300"
				>
					Go home
				</a>
			</div>
		</div>
	);
}
```

**Step 12: Install dependencies**

Run: `cd simse-status && npm install`

**Step 13: Commit**

```bash
git add simse-status/
git commit -m "feat(simse-status): scaffold React Router v7 project with cron + D1"
```

---

### Task 4: Create D1 database and migrations

**Files:**
- Create: `simse-status/migrations/0001_initial.sql`

**Step 1: Create migration file**

```sql
-- Initial schema for simse-status
CREATE TABLE checks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  service_id TEXT NOT NULL,
  status TEXT NOT NULL,
  response_time_ms INTEGER,
  status_code INTEGER,
  error TEXT,
  checked_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_checks_service_time ON checks(service_id, checked_at);
CREATE INDEX idx_checks_time ON checks(checked_at);
```

Note: The `services` table from the design doc is unnecessary — the service list is hardcoded in `worker.ts` as the `SERVICES` constant. This avoids a seed step and keeps things simple.

**Step 2: Create the D1 database**

Run: `cd simse-status && npx wrangler d1 create simse-status-db`

Copy the returned `database_id` into `wrangler.toml` replacing `placeholder-create-via-wrangler`.

**Step 3: Apply migration locally**

Run: `cd simse-status && npm run db:migrate`
Expected: Migration applied successfully

**Step 4: Commit**

```bash
git add simse-status/migrations/ simse-status/wrangler.toml
git commit -m "feat(simse-status): add D1 migration for checks table"
```

---

### Task 5: Build the status page UI

**Files:**
- Create: `simse-status/app/routes/home.tsx`
- Create: `simse-status/app/components/StatusBanner.tsx`
- Create: `simse-status/app/components/ServiceRow.tsx`
- Create: `simse-status/app/components/UptimeBar.tsx`

**Step 1: Create the loader in app/routes/home.tsx**

This is the main page. The loader queries D1 for:
1. Latest check per service (current status)
2. Daily aggregates for the past 90 days (uptime bars)

```tsx
import type { Route } from './+types/home';
import { ServiceRow } from '~/components/ServiceRow';
import { StatusBanner } from '~/components/StatusBanner';

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

	// Latest check per service
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

	// Daily aggregates for past 90 days
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
				{new Date(lastChecked + 'Z').toLocaleString('en-US', {
					dateStyle: 'medium',
					timeStyle: 'short',
				})}
			</footer>
		</div>
	);
}
```

**Step 2: Create app/components/StatusBanner.tsx**

```tsx
interface Props {
	services: Array<{
		status: 'up' | 'degraded' | 'down' | 'unknown';
	}>;
}

export function StatusBanner({ services }: Props) {
	const hasDown = services.some((s) => s.status === 'down');
	const hasDegraded = services.some((s) => s.status === 'degraded');

	let label: string;
	let color: string;
	let dotColor: string;

	if (hasDown) {
		label = 'Major Outage';
		color = 'border-red-500/30 bg-red-500/5';
		dotColor = 'bg-red-400';
	} else if (hasDegraded) {
		label = 'Partial Degradation';
		color = 'border-yellow-500/30 bg-yellow-500/5';
		dotColor = 'bg-yellow-400';
	} else {
		label = 'All Systems Operational';
		color = 'border-emerald-500/30 bg-emerald-500/5';
		dotColor = 'bg-emerald-400';
	}

	return (
		<div
			className={`animate-fade-in rounded-xl border px-6 py-4 text-center ${color}`}
		>
			<div className="flex items-center justify-center gap-2.5">
				<span
					className={`inline-block h-2.5 w-2.5 rounded-full ${dotColor}`}
				/>
				<span className="font-mono text-sm font-semibold text-zinc-200">
					{label}
				</span>
			</div>
		</div>
	);
}
```

**Step 3: Create app/components/UptimeBar.tsx**

Renders a row of small colored bars — one per day for the last 90 days.

```tsx
interface Props {
	dailyStatus: Array<{ date: string; status: 'up' | 'degraded' | 'down' }>;
}

const STATUS_COLORS = {
	up: 'bg-emerald-400',
	degraded: 'bg-yellow-400',
	down: 'bg-red-400',
	empty: 'bg-zinc-800',
};

export function UptimeBar({ dailyStatus }: Props) {
	// Build a 90-day array, filling gaps with 'empty'
	const today = new Date();
	const days: Array<{ date: string; status: 'up' | 'degraded' | 'down' | 'empty' }> = [];
	const statusMap = new Map(dailyStatus.map((d) => [d.date, d.status]));

	for (let i = 89; i >= 0; i--) {
		const d = new Date(today);
		d.setUTCDate(d.getUTCDate() - i);
		const key = d.toISOString().slice(0, 10);
		days.push({ date: key, status: statusMap.get(key) ?? 'empty' });
	}

	return (
		<div className="flex gap-px">
			{days.map((day) => (
				<div
					key={day.date}
					className={`h-8 flex-1 rounded-[1px] first:rounded-l last:rounded-r ${STATUS_COLORS[day.status]}`}
					title={`${day.date}: ${day.status}`}
				/>
			))}
		</div>
	);
}
```

**Step 4: Create app/components/ServiceRow.tsx**

```tsx
import { UptimeBar } from './UptimeBar';

interface Props {
	service: {
		id: string;
		name: string;
		status: 'up' | 'degraded' | 'down' | 'unknown';
		responseTimeMs: number | null;
		uptimePercent: number;
		dailyStatus: Array<{ date: string; status: 'up' | 'degraded' | 'down' }>;
	};
}

const STATUS_DOT = {
	up: 'bg-emerald-400',
	degraded: 'bg-yellow-400',
	down: 'bg-red-400',
	unknown: 'bg-zinc-600',
};

const STATUS_LABEL = {
	up: 'Operational',
	degraded: 'Degraded',
	down: 'Down',
	unknown: 'Unknown',
};

export function ServiceRow({ service }: Props) {
	return (
		<div className="animate-fade-in-up rounded-xl border border-zinc-800 bg-zinc-900/50 p-5">
			<div className="mb-3 flex items-center justify-between">
				<div className="flex items-center gap-3">
					<span
						className={`inline-block h-2 w-2 rounded-full ${STATUS_DOT[service.status]}`}
					/>
					<span className="text-sm font-medium text-zinc-200">
						{service.name}
					</span>
				</div>
				<div className="flex items-center gap-4">
					{service.responseTimeMs !== null && (
						<span className="font-mono text-xs text-zinc-500">
							{service.responseTimeMs}ms
						</span>
					)}
					<span className="font-mono text-xs text-zinc-400">
						{service.uptimePercent}%
					</span>
					<span className="text-xs text-zinc-500">
						{STATUS_LABEL[service.status]}
					</span>
				</div>
			</div>
			<UptimeBar dailyStatus={service.dailyStatus} />
			<div className="mt-1 flex justify-between font-mono text-[10px] text-zinc-600">
				<span>90 days ago</span>
				<span>Today</span>
			</div>
		</div>
	);
}
```

**Step 5: Verify build**

Run: `cd simse-status && npm run build`
Expected: Build succeeds

**Step 6: Commit**

```bash
git add simse-status/app/
git commit -m "feat(simse-status): add status page UI with uptime bars"
```

---

### Task 6: Add .gitignore and finalize

**Files:**
- Create: `simse-status/.gitignore`

**Step 1: Create .gitignore**

```
node_modules/
dist/
build/
.react-router/
.wrangler/
.dev.vars
```

**Step 2: Final lint check**

Run: `cd simse-status && npx biome check .`
Expected: No errors (fix any that appear with `npx biome check --write .`)

**Step 3: Final typecheck**

Run: `cd simse-status && npx react-router typegen && npx tsc --noEmit`
Expected: No type errors

**Step 4: Commit all remaining files**

```bash
git add simse-status/.gitignore
git commit -m "feat(simse-status): add gitignore and finalize project"
```

---

### Task 7: Update root CLAUDE.md

**Files:**
- Modify: `CLAUDE.md` (add simse-status to the repository layout and commands sections)

**Step 1: Add simse-status to the repository layout**

Add to the layout section after `simse-mailer/`:
```
simse-status/               # TypeScript — Status page (React Router v7 + Cloudflare Pages + D1 + Cron)
```

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add simse-status to CLAUDE.md"
```
