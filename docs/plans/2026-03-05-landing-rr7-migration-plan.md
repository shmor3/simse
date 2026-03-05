# simse-landing React Router v7 Migration Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Migrate simse-landing from a plain Vite + React SPA to React Router v7 framework mode on Cloudflare Pages, moving Cloudflare Functions into RR7 loaders/actions.

**Architecture:** Replace `@vitejs/plugin-react` with `@react-router/dev` + `@react-router/cloudflare`. Move all source from `src/` to `app/`. Convert `functions/` (waitlist API + unsubscribe) to RR7 route actions/loaders. Use simse-cloud as the reference implementation for RR7 + Cloudflare patterns.

**Tech Stack:** React Router v7, Cloudflare Pages/Workers, D1, Vite, Tailwind CSS v4, Zod v4. Emails are sent via simse-api → simse-mailer (no direct Resend calls).

---

### Task 1: Update dependencies

**Files:**
- Modify: `simse-landing/package.json`

**Step 1: Update package.json**

Replace the scripts and dependencies. Keep all existing deps, add RR7 + Cloudflare deps, remove `@vitejs/plugin-react`:

```json
{
	"name": "simse-landing",
	"private": true,
	"type": "module",
	"scripts": {
		"dev": "react-router dev",
		"build": "react-router build",
		"start": "wrangler pages dev",
		"preview": "wrangler pages dev ./build/client",
		"typecheck": "react-router typegen && tsc --noEmit",
		"deploy": "wrangler pages deploy build/client",
		"cf-typegen": "wrangler types",
		"lint": "biome check .",
		"lint:fix": "biome check --write .",
		"db:migrate": "wrangler d1 migrations apply simse_waitlist --local",
		"db:migrate:prod": "wrangler d1 migrations apply simse_waitlist --remote"
	},
	"dependencies": {
		"@fontsource-variable/dm-sans": "^5.2.8",
		"@fontsource/space-mono": "^5.2.9",
		"@react-router/cloudflare": "^7.13.1",
		"clsx": "^2.1.1",
		"isbot": "^5.1.27",
		"react": "^19.2.0",
		"react-dom": "^19.2.0",
		"react-router": "^7.13.1",
		"zod": "^4.3.6"
	},
	"devDependencies": {
		"@biomejs/biome": "^2.4.5",
		"@cloudflare/workers-types": "^4.20260305.0",
		"@react-router/dev": "^7.13.1",
		"@resvg/resvg-js": "^2.6.2",
		"@tailwindcss/vite": "^4.2.1",
		"@types/react": "^19.0.0",
		"@types/react-dom": "^19.0.0",
		"satori": "^0.21.0",
		"tailwindcss": "^4.2.1",
		"typescript": "^5.7.0",
		"vite": "^7.3.0",
		"vite-tsconfig-paths": "^6.1.1",
		"wrangler": "^4.0.0"
	}
}
```

**Changes from current:**
- Added: `@react-router/cloudflare`, `@react-router/dev`, `isbot`, `vite-tsconfig-paths`
- Removed: `@vitejs/plugin-react`, `@react-email/components`, `@react-email/render` (emails now handled by simse-mailer)
- Scripts: `vite` → `react-router dev`, `tsc -b && vite build` → `react-router build`, added `typecheck`, `start`, `preview`, changed `deploy` output dir to `build/client`

**Step 2: Install dependencies**

Run: `cd simse-landing && bun install`

**Step 3: Commit**

```
chore(simse-landing): update deps for React Router v7 framework mode
```

---

### Task 2: Add RR7 config files

**Files:**
- Create: `simse-landing/react-router.config.ts`
- Modify: `simse-landing/vite.config.ts`
- Modify: `simse-landing/tsconfig.json`
- Modify: `simse-landing/wrangler.toml`

**Step 1: Create react-router.config.ts**

```typescript
import type { Config } from '@react-router/dev/config';

export default {
	ssr: true,
} satisfies Config;
```

**Step 2: Rewrite vite.config.ts**

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

**Step 3: Rewrite tsconfig.json**

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
	"include": ["app", ".react-router/types/**/*"]
}
```

**Changes:** Removed `"functions"` from include (functions dir is being deleted). Added `paths` alias, `rootDirs`, `vite/client` type, and `.react-router/types` include.

**Step 4: Update wrangler.toml**

Change `pages_build_output_dir` from `./dist` to `./build/client`:

```toml
name = "simse-landing"
compatibility_date = "2026-02-28"
pages_build_output_dir = "./build/client"

routes = [{ pattern = "simse.dev", custom_domain = true }]

[[d1_databases]]
binding = "DB"
database_name = "simse_waitlist"
database_id = "e5659e77-878c-440c-bbf6-dd74a324938b"

[[queues.producers]]
queue = "simse-landing-comms"
binding = "COMMS_QUEUE"

# No RESEND_API_KEY — emails enqueued to simse-landing-comms → simse-mailer
```

**Changes:** `./dist` → `./build/client`, binding renamed from `simse_waitlist` to `DB`, removed `RESEND_API_KEY`, added `COMMS_QUEUE` queue producer (emails go directly to simse-mailer queue).

**Step 5: Update .gitignore** — add `.react-router/` to .gitignore

Read the current `.gitignore` first, then add `.react-router/` line.

**Step 6: Commit**

```
chore(simse-landing): add RR7 config, update vite + tsconfig + wrangler
```

---

### Task 3: Create app/ directory with root and entry

**Files:**
- Create: `simse-landing/app/root.tsx`
- Create: `simse-landing/app/entry.server.tsx`
- Move: `simse-landing/src/index.css` → `simse-landing/app/app.css`

**Step 1: Create app/root.tsx**

This replaces both `index.html` (head/meta tags) and `src/App.tsx` (layout):

```tsx
import '@fontsource-variable/dm-sans';
import '@fontsource/space-mono';
import clsx from 'clsx';
import { Links, Meta, Outlet, Scripts, ScrollRestoration } from 'react-router';
import DotGrid from './components/DotGrid';
import './app.css';

export function Layout({ children }: { children: React.ReactNode }) {
	return (
		<html lang="en">
			<head>
				<meta charSet="utf-8" />
				<meta name="viewport" content="width=device-width, initial-scale=1" />
				<meta
					name="description"
					content="simse is a modular AI assistant that evolves with you. Connect any ACP or MCP backend. Context carries over. Preferences stick."
				/>
				<meta name="theme-color" content="#34d399" />
				<link rel="canonical" href="https://simse.dev/" />
				<link
					rel="icon"
					type="image/svg+xml"
					href="data:image/svg+xml,%3Csvg viewBox='0 0 100 100' fill='none' xmlns='http://www.w3.org/2000/svg'%3E%3Cdefs%3E%3CclipPath id='h'%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5'/%3E%3C/clipPath%3E%3C/defs%3E%3Cpolygon points='50,5 93.3,27.5 93.3,72.5 50,95 6.7,72.5 6.7,27.5' fill='none' stroke='white' stroke-width='5'/%3E%3Cg clip-path='url(%23h)'%3E%3Cpath d='M44,-10 C90,15 94,35 50,50 C6,65 10,85 56,110' stroke='white' stroke-width='8' stroke-linecap='round' fill='none'/%3E%3C/g%3E%3C/svg%3E"
				/>
				<link rel="manifest" href="/site.webmanifest" />

				{/* Open Graph */}
				<meta property="og:type" content="website" />
				<meta property="og:url" content="https://simse.dev/" />
				<meta property="og:title" content="simse — The assistant that evolves with you" />
				<meta
					property="og:description"
					content="Connect any ACP or MCP backend. Context carries over. Preferences stick. An assistant that gets better the more you use it."
				/>
				<meta property="og:image" content="https://simse.dev/og-image.png" />
				<meta property="og:image:width" content="1200" />
				<meta property="og:image:height" content="630" />

				{/* Twitter Card */}
				<meta name="twitter:card" content="summary_large_image" />
				<meta name="twitter:title" content="simse — The assistant that evolves with you" />
				<meta
					name="twitter:description"
					content="Connect any ACP or MCP backend. Context carries over. Preferences stick. An assistant that gets better the more you use it."
				/>
				<meta name="twitter:image" content="https://simse.dev/og-image.png" />

				<title>simse — The assistant that evolves with you</title>
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
	return (
		<div className={clsx('flex h-screen flex-col overflow-hidden bg-[#0a0a0b]')}>
			<DotGrid />
			<Outlet />
		</div>
	);
}
```

**Step 2: Create app/entry.server.tsx**

Copy from simse-cloud — standard Cloudflare Workers SSR entry:

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

**Step 3: Copy index.css to app/app.css**

Copy `src/index.css` to `app/app.css` — contents unchanged.

**Step 4: Commit**

```
feat(simse-landing): add RR7 root layout and server entry
```

---

### Task 4: Move components and lib to app/

**Files:**
- Move: `simse-landing/src/components/*.tsx` → `simse-landing/app/components/*.tsx`
- Move: `simse-landing/src/lib/schema.ts` → `simse-landing/app/lib/schema.ts`
- Move: `simse-landing/functions/lib/validate-email.ts` → `simse-landing/app/lib/validate-email.server.ts`

**Note:** Email templates (`functions/emails/`) and `functions/lib/send-email.ts` are NOT moved — they will be migrated to simse-mailer in the service extraction plan. The landing page will call simse-api to send emails instead.

**Step 1: Move client components**

```bash
cd simse-landing
mkdir -p app/components app/lib
cp src/components/*.tsx app/components/
cp src/lib/schema.ts app/lib/schema.ts
```

All 7 component files (DotGrid, Features, Footer, Hero, SimseLogo, Typewriter, WaitlistForm) move unchanged.

**Step 2: Move validate-email server lib**

```bash
cp functions/lib/validate-email.ts app/lib/validate-email.server.ts
```

The `.server.ts` suffix ensures it is never bundled for the client.

**Step 3: Commit**

```
refactor(simse-landing): move components and lib to app/ directory
```

---

### Task 5: Create home route with waitlist action

**Files:**
- Create: `simse-landing/app/routes/home.tsx`

**Step 1: Create app/routes/home.tsx**

This replaces `src/pages/Home.tsx` + `functions/api/waitlist.ts`. The component stays the same, but the waitlist logic becomes an RR7 action. Emails are sent via simse-api → simse-mailer instead of calling Resend directly:

```tsx
import type { Route } from './+types/home';
import { waitlistSchema } from '~/lib/schema';
import { validateEmail } from '~/lib/validate-email.server';
import Footer from '~/components/Footer';
import Hero from '~/components/Hero';

export function meta(): Route.MetaDescriptors {
	return [
		{ title: 'simse — The assistant that evolves with you' },
	];
}

export async function action({ request, context }: Route.ActionArgs) {
	let body: unknown;
	try {
		body = await request.json();
	} catch {
		return Response.json({ error: 'Invalid JSON' }, { status: 400 });
	}

	const parsed = waitlistSchema.safeParse(body);
	if (!parsed.success) {
		return Response.json(
			{ error: parsed.error.issues[0].message },
			{ status: 400 },
		);
	}

	const email = parsed.data.email.trim().toLowerCase();

	const validation = await validateEmail(email);
	if (!validation.valid) {
		return Response.json({ error: validation.reason }, { status: 422 });
	}

	const db = context.cloudflare.env.DB;

	let shouldEmail = false;
	try {
		const result = await db
			.prepare(
				`INSERT INTO waitlist (email, subscribed, updated_at) VALUES (?, 1, datetime('now'))
				ON CONFLICT (email) DO UPDATE SET subscribed = 1, updated_at = datetime('now')
				WHERE subscribed = 0 AND updated_at < datetime('now', '-1 day')`,
			)
			.bind(email)
			.run();
		shouldEmail = (result.meta?.changes ?? 0) > 0;
	} catch (err) {
		console.error('D1 insert failed', err);
		return Response.json({ error: 'Database error' }, { status: 500 });
	}

	if (shouldEmail) {
		const origin = new URL(request.url).origin;
		const unsubscribeUrl = `${origin}/unsubscribe?email=${encodeURIComponent(email)}`;

		// Enqueue welcome email to simse-mailer via Cloudflare Queue
		context.cloudflare.ctx.waitUntil(
			context.cloudflare.env.COMMS_QUEUE.send({
				type: 'email',
				template: 'waitlist-welcome',
				to: email,
				props: { unsubscribeUrl },
			}).catch(() => {}),
		);
	}

	return Response.json({ success: true });
}

export default function Home() {
	return (
		<>
			<Hero />
			<Footer />
		</>
	);
}
```

**Key differences from the Cloudflare Function version:**
- `context.env.simse_waitlist` → `context.cloudflare.env.DB`
- `context.waitUntil()` → `context.cloudflare.ctx.waitUntil()`
- `context.request` → `request` (direct param)
- `sendWelcomeEmail(email, apiKey, url)` → `env.COMMS_QUEUE.send({ type: 'email', template, to, props })` (emails via queue)

**Step 2: Update WaitlistForm to POST to current route**

The WaitlistForm currently POSTs to `/api/waitlist`. With RR7 actions, it should POST to the current route (`/`). Read `app/components/WaitlistForm.tsx` and change the fetch URL:

```typescript
// In app/components/WaitlistForm.tsx, change:
const res = await fetch('/api/waitlist', {
// To:
const res = await fetch('/', {
```

**Note:** The form uses `fetch()` directly (not RR7's `useFetcher`), which is fine — the action handles JSON POST requests.

**Step 3: Commit**

```
feat(simse-landing): add home route with waitlist action
```

---

### Task 6: Create unsubscribe route

**Files:**
- Create: `simse-landing/app/routes/unsubscribe.tsx`

**Step 1: Create app/routes/unsubscribe.tsx**

This replaces `functions/unsubscribe.ts`. Convert from raw HTML response to a proper React route:

```tsx
import clsx from 'clsx';
import type { Route } from './+types/unsubscribe';

export async function loader({ request, context }: Route.LoaderArgs) {
	const url = new URL(request.url);
	const email = url.searchParams.get('email')?.trim().toLowerCase();

	if (!email) {
		return { success: false, message: 'Invalid unsubscribe link' };
	}

	try {
		await context.cloudflare.env.DB
			.prepare(
				"UPDATE waitlist SET subscribed = 0, updated_at = datetime('now') WHERE email = ? AND subscribed = 1",
			)
			.bind(email)
			.run();
	} catch (err) {
		console.error('D1 update failed', err);
		return { success: false, message: 'Something went wrong' };
	}

	return { success: true, message: 'Unsubscribed' };
}

export default function Unsubscribe({ loaderData }: Route.ComponentProps) {
	const { success, message } = loaderData;

	return (
		<div className="flex min-h-screen items-center justify-center bg-[#0a0a0b] px-8">
			<div className="max-w-[420px] text-center">
				<div className={clsx('text-[2rem]', success ? 'text-emerald-400' : 'text-red-400')}>
					{success ? '\u2713' : '\u2717'}
				</div>
				<h1
					className={clsx(
						'mt-4 text-xl font-semibold',
						success ? 'text-emerald-400' : 'text-red-400',
					)}
				>
					{message}
				</h1>
				<p className="mt-3 text-sm leading-relaxed text-zinc-500">
					{success
						? "You've been removed from our mailing list and won't receive any more emails from us."
						: 'Please try again or contact us if the issue persists.'}
				</p>
				<p className="mt-6">
					<a
						href="/"
						className="text-zinc-400 underline underline-offset-2 hover:text-zinc-300"
					>
						Back to simse.dev
					</a>
				</p>
			</div>
		</div>
	);
}
```

**Step 2: Commit**

```
feat(simse-landing): add unsubscribe route with loader
```

---

### Task 7: Configure route file convention

**Files:**
- Create: `simse-landing/app/routes.ts` (if using manual route config)

**Step 1: Check if RR7 auto-discovers routes or needs manual config**

By default, RR7 framework mode uses file-based routing in `app/routes/`. With our files named `home.tsx` and `unsubscribe.tsx`, the routes would be `/home` and `/unsubscribe`.

We need `/` (not `/home`) for the home route. Create `app/routes.ts` to configure this:

```typescript
import { type RouteConfig, index, route } from '@react-router/dev/routes';

export default [
	index('routes/home.tsx'),
	route('unsubscribe', 'routes/unsubscribe.tsx'),
] satisfies RouteConfig;
```

**Step 2: Commit**

```
feat(simse-landing): configure route mapping
```

---

### Task 8: Delete old files

**Files:**
- Delete: `simse-landing/src/` (entire directory)
- Delete: `simse-landing/functions/` (entire directory)
- Delete: `simse-landing/index.html`

**Step 1: Delete src/ directory**

```bash
cd simse-landing && rm -rf src/
```

This removes:
- `src/main.tsx` — replaced by RR7 entry
- `src/router.tsx` — replaced by `app/routes.ts`
- `src/App.tsx` — replaced by `app/root.tsx`
- `src/index.css` — moved to `app/app.css`
- `src/pages/Home.tsx` — replaced by `app/routes/home.tsx`
- `src/components/*.tsx` — moved to `app/components/`
- `src/lib/schema.ts` — moved to `app/lib/`

**Step 2: Delete functions/ directory**

```bash
rm -rf functions/
```

This removes:
- `functions/api/waitlist.ts` — replaced by home route action
- `functions/unsubscribe.ts` — replaced by unsubscribe route loader
- `functions/lib/validate-email.ts` — moved to `app/lib/`
- `functions/lib/send-email.ts` — no longer needed (emails via API)
- `functions/emails/` — templates will be added to simse-mailer in the service extraction plan

**Step 3: Delete index.html**

```bash
rm index.html
```

Replaced by `app/root.tsx` Layout component.

**Step 4: Commit**

```
refactor(simse-landing): delete old src/, functions/, and index.html
```

---

### Task 9: Verify build and test

**Step 1: Generate RR7 types**

Run: `cd simse-landing && bun run typecheck`
Expected: Types generate successfully, no type errors

**Step 2: Build**

Run: `cd simse-landing && bun run build`
Expected: Build succeeds, output in `build/client/`

**Step 3: Test locally**

Run: `cd simse-landing && bun run dev`
Expected:
- Landing page loads at `http://localhost:5173/`
- DotGrid animation works
- Typewriter effect works
- Waitlist form submits (POST to `/` → action → D1)
- `/unsubscribe?email=test@example.com` shows unsubscribe page

**Step 4: Lint**

Run: `cd simse-landing && bun run lint`
Expected: No errors

**Step 5: Commit (if any fixes needed)**

```
fix(simse-landing): fix build/lint issues from RR7 migration
```

---

### Task 10: Final cleanup

**Step 1: Verify `dist/` is in .gitignore and add `build/` if not**

Read `.gitignore`, ensure it has:
```
node_modules/
dist/
build/
.wrangler/
.react-router/
```

**Step 2: Remove `dist/` from git if tracked**

```bash
cd simse-landing && git rm -rf dist/ --cached 2>/dev/null || true
```

**Step 3: Final commit**

```
chore(simse-landing): complete React Router v7 migration
```
