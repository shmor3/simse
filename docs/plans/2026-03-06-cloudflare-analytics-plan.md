# Cloudflare Workers Analytics Engine Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Cloudflare Workers Analytics Engine to all 7 workers, writing per-request data points (method, path, status, latency, user context, geo) to a shared `simse-analytics` dataset.

**Architecture:** Each worker gets an `ANALYTICS: AnalyticsEngineDataset` binding and inline analytics code. Hono workers use a middleware. The CDN worker wraps its fetch handler. Pages workers wrap their worker.ts entry. No shared package — each worker is self-contained.

**Tech Stack:** Cloudflare Workers Analytics Engine, TypeScript, Hono middleware

---

### Task 1: Add analytics to simse-api

**Files:**
- Modify: `simse-api/wrangler.toml`
- Modify: `simse-api/src/types.ts`
- Create: `simse-api/src/middleware/analytics.ts`
- Modify: `simse-api/src/index.ts`

**Step 1: Add Analytics Engine binding to wrangler.toml**

Append to `simse-api/wrangler.toml`:

```toml
[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"
```

**Step 2: Update Env interface**

In `simse-api/src/types.ts`, add the binding to the `Env` interface:

```typescript
export interface Env {
	COMMS_QUEUE: Queue;
	SECRETS: SecretsStoreNamespace;
	ANALYTICS: AnalyticsEngineDataset;
}
```

**Step 3: Create analytics middleware**

Create `simse-api/src/middleware/analytics.ts`:

```typescript
import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const analyticsMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	const cf = (c.req.raw as any).cf;

	c.env.ANALYTICS.writeDataPoint({
		indexes: ['simse-api'],
		blobs: [
			c.req.method,
			c.req.path,
			String(c.res.status),
			'simse-api',
			c.req.header('X-User-Id') ?? '',
			c.req.header('X-Team-Id') ?? '',
			cf?.country ?? '',
			cf?.city ?? '',
			cf?.continent ?? '',
			(c.req.header('User-Agent') ?? '').slice(0, 256),
			c.req.header('Referer') ?? '',
			c.res.headers.get('Content-Type') ?? '',
			c.req.header('Cf-Ray') ?? '',
		],
		doubles: [
			latencyMs,
			c.res.status,
			Number(c.req.header('Content-Length') ?? 0),
			Number(c.res.headers.get('Content-Length') ?? 0),
			Number(cf?.colo ?? 0),
		],
	});
});
```

**Step 4: Wire middleware in index.ts**

Update `simse-api/src/index.ts` to use the analytics middleware. It must run first (before secrets) to wrap the entire request:

```typescript
import { Hono } from 'hono';
import { analyticsMiddleware } from './middleware/analytics';
import { secretsMiddleware } from './middleware/secrets';
import gateway from './routes/gateway';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.use('*', analyticsMiddleware);
app.get('/health', (c) => c.json({ ok: true }));
app.use('*', secretsMiddleware);
app.route('', gateway);

export default app;
```

**Step 5: Verify build**

```bash
cd simse-api && npm run lint
```

Expected: No errors.

**Step 6: Commit**

```bash
git add simse-api/wrangler.toml simse-api/src/types.ts simse-api/src/middleware/analytics.ts simse-api/src/index.ts
git commit -m "feat(simse-api): add Workers Analytics Engine middleware"
```

---

### Task 2: Add analytics to simse-auth

**Files:**
- Modify: `simse-auth/wrangler.toml`
- Modify: `simse-auth/src/types.ts`
- Create: `simse-auth/src/middleware/analytics.ts`
- Modify: `simse-auth/src/index.ts`

**Step 1: Add Analytics Engine binding to wrangler.toml**

Append to `simse-auth/wrangler.toml`:

```toml
[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"
```

**Step 2: Update Env interface**

In `simse-auth/src/types.ts`, add `ANALYTICS`:

```typescript
export interface Env {
	DB: D1Database;
	COMMS_QUEUE: Queue;
	ANALYTICS: AnalyticsEngineDataset;
}
```

**Step 3: Create analytics middleware**

Create `simse-auth/src/middleware/analytics.ts` — same pattern as simse-api but with service name `simse-auth`. The userId comes from `X-User-Id` header (set by simse-api gateway for protected routes):

```typescript
import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const analyticsMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	const cf = (c.req.raw as any).cf;

	c.env.ANALYTICS.writeDataPoint({
		indexes: ['simse-auth'],
		blobs: [
			c.req.method,
			c.req.path,
			String(c.res.status),
			'simse-auth',
			c.req.header('X-User-Id') ?? '',
			c.req.header('X-Team-Id') ?? '',
			cf?.country ?? '',
			cf?.city ?? '',
			cf?.continent ?? '',
			(c.req.header('User-Agent') ?? '').slice(0, 256),
			c.req.header('Referer') ?? '',
			c.res.headers.get('Content-Type') ?? '',
			c.req.header('Cf-Ray') ?? '',
		],
		doubles: [
			latencyMs,
			c.res.status,
			Number(c.req.header('Content-Length') ?? 0),
			Number(c.res.headers.get('Content-Length') ?? 0),
			Number(cf?.colo ?? 0),
		],
	});
});
```

**Step 4: Wire middleware in index.ts**

Update `simse-auth/src/index.ts` to add analytics as the first middleware:

```typescript
import { Hono } from 'hono';
import apiKeys from './routes/api-keys';
import auth from './routes/auth';
import teams from './routes/teams';
import users from './routes/users';
import { analyticsMiddleware } from './middleware/analytics';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.use('*', analyticsMiddleware);
app.get('/health', (c) => c.json({ ok: true }));

// Auth routes (public — gateway forwards without auth check)
app.route('/auth', auth);

// Protected routes (gateway validates token first, passes X-User-Id)
app.route('/users', users);
app.route('/teams', teams);
app.route('/api-keys', apiKeys);

export default app;
```

**Step 5: Verify build**

```bash
cd simse-auth && npm run lint
```

Expected: No errors.

**Step 6: Commit**

```bash
git add simse-auth/wrangler.toml simse-auth/src/types.ts simse-auth/src/middleware/analytics.ts simse-auth/src/index.ts
git commit -m "feat(simse-auth): add Workers Analytics Engine middleware"
```

---

### Task 3: Add analytics to simse-payments

**Files:**
- Modify: `simse-payments/wrangler.toml`
- Modify: `simse-payments/src/types.ts`
- Create: `simse-payments/src/middleware/analytics.ts`
- Modify: `simse-payments/src/index.ts`

**Step 1: Add Analytics Engine binding to wrangler.toml**

Append to `simse-payments/wrangler.toml`:

```toml
[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"
```

**Step 2: Update Env interface**

In `simse-payments/src/types.ts`, add `ANALYTICS`:

```typescript
export interface Env {
	DB: D1Database;
	STRIPE_SECRET_KEY: string;
	STRIPE_WEBHOOK_SECRET: string;
	API_SECRET: string;
	MAILER_API_URL: string;
	MAILER_API_SECRET: string;
	ANALYTICS: AnalyticsEngineDataset;
}
```

**Step 3: Create analytics middleware**

Create `simse-payments/src/middleware/analytics.ts` — same pattern, service name `simse-payments`:

```typescript
import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const analyticsMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	const cf = (c.req.raw as any).cf;

	c.env.ANALYTICS.writeDataPoint({
		indexes: ['simse-payments'],
		blobs: [
			c.req.method,
			c.req.path,
			String(c.res.status),
			'simse-payments',
			c.req.header('X-User-Id') ?? '',
			c.req.header('X-Team-Id') ?? '',
			cf?.country ?? '',
			cf?.city ?? '',
			cf?.continent ?? '',
			(c.req.header('User-Agent') ?? '').slice(0, 256),
			c.req.header('Referer') ?? '',
			c.res.headers.get('Content-Type') ?? '',
			c.req.header('Cf-Ray') ?? '',
		],
		doubles: [
			latencyMs,
			c.res.status,
			Number(c.req.header('Content-Length') ?? 0),
			Number(c.res.headers.get('Content-Length') ?? 0),
			Number(cf?.colo ?? 0),
		],
	});
});
```

**Step 4: Wire middleware in index.ts**

Update `simse-payments/src/index.ts` — add analytics as the first middleware:

```typescript
import { Hono } from 'hono';
import { analyticsMiddleware } from './middleware/analytics';
import { authMiddleware } from './middleware/auth';
import checkout from './routes/checkout';
import credits from './routes/credits';
import customers from './routes/customers';
import portal from './routes/portal';
import subscriptions from './routes/subscriptions';
import webhooks from './routes/webhooks';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.use('*', analyticsMiddleware);
app.get('/health', (c) => c.json({ ok: true }));

// Webhooks — no auth (Stripe signature verification instead)
app.route('/webhooks', webhooks);

// Authenticated routes
app.use('/customers', authMiddleware);
app.use('/customers/*', authMiddleware);
app.use('/checkout', authMiddleware);
app.use('/portal', authMiddleware);
app.use('/subscriptions/*', authMiddleware);
app.use('/credits', authMiddleware);
app.use('/credits/*', authMiddleware);

app.route('/customers', customers);
app.route('/checkout', checkout);
app.route('/portal', portal);
app.route('/subscriptions', subscriptions);
app.route('/credits', credits);

export default app;
```

**Step 5: Verify build**

```bash
cd simse-payments && npm run lint
```

Expected: No errors.

**Step 6: Commit**

```bash
git add simse-payments/wrangler.toml simse-payments/src/types.ts simse-payments/src/middleware/analytics.ts simse-payments/src/index.ts
git commit -m "feat(simse-payments): add Workers Analytics Engine middleware"
```

---

### Task 4: Add analytics to simse-mailer

**Files:**
- Modify: `simse-mailer/wrangler.toml`
- Modify: `simse-mailer/src/index.ts` (Env interface is defined here, not in separate types.ts)

**Step 1: Add Analytics Engine binding to wrangler.toml**

Append to `simse-mailer/wrangler.toml`:

```toml
[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"
```

**Step 2: Update Env interface and add analytics middleware inline**

The mailer has its Env interface and middleware inline in `src/index.ts`. Update it:

1. Add `ANALYTICS: AnalyticsEngineDataset` to the `Env` interface (line 22-25)
2. Add the analytics middleware as the first `app.use('*', ...)` — before the secrets middleware

The updated `simse-mailer/src/index.ts` should have the Env interface at line 22:

```typescript
export interface Env {
	DB: D1Database;
	SECRETS: SecretsStoreNamespace;
	ANALYTICS: AnalyticsEngineDataset;
}
```

And insert the analytics middleware before the secrets middleware (before line 38). Add this block right after the `app` declaration:

```typescript
// Analytics middleware
app.use('*', async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	const cf = (c.req.raw as any).cf;

	c.env.ANALYTICS.writeDataPoint({
		indexes: ['simse-mailer'],
		blobs: [
			c.req.method,
			c.req.path,
			String(c.res.status),
			'simse-mailer',
			c.req.header('X-User-Id') ?? '',
			'',
			cf?.country ?? '',
			cf?.city ?? '',
			cf?.continent ?? '',
			(c.req.header('User-Agent') ?? '').slice(0, 256),
			c.req.header('Referer') ?? '',
			c.res.headers.get('Content-Type') ?? '',
			c.req.header('Cf-Ray') ?? '',
		],
		doubles: [
			latencyMs,
			c.res.status,
			Number(c.req.header('Content-Length') ?? 0),
			Number(c.res.headers.get('Content-Length') ?? 0),
			Number(cf?.colo ?? 0),
		],
	});
});
```

**Step 3: Verify build**

```bash
cd simse-mailer && npm run lint
```

Expected: No errors.

**Step 4: Commit**

```bash
git add simse-mailer/wrangler.toml simse-mailer/src/index.ts
git commit -m "feat(simse-mailer): add Workers Analytics Engine middleware"
```

---

### Task 5: Add analytics to simse-cdn

The CDN worker uses a raw `ExportedHandler` (no Hono). Wrap the fetch handler to capture timing and write the data point.

**Files:**
- Modify: `simse-cdn/wrangler.toml`
- Modify: `simse-cdn/src/types.ts`
- Modify: `simse-cdn/src/index.ts`

**Step 1: Add Analytics Engine binding to wrangler.toml**

Append to `simse-cdn/wrangler.toml`:

```toml
[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"
```

**Step 2: Update Env interface**

In `simse-cdn/src/types.ts`:

```typescript
export interface Env {
	CDN_BUCKET: R2Bucket;
	VERSION_STORE: KVNamespace;
	ANALYTICS: AnalyticsEngineDataset;
}
```

**Step 3: Wrap the fetch handler with analytics**

In `simse-cdn/src/index.ts`, wrap the existing `fetch` handler body. The handler currently starts at line 13 (`async fetch(request: Request, env: Env): Promise<Response>`). Wrap it so timing is captured and a data point is written after the response is produced:

Replace the `export default` block (lines 12-65) with:

```typescript
export default {
	async fetch(request: Request, env: Env): Promise<Response> {
		const start = Date.now();
		const response = await handleRequest(request, env);
		const latencyMs = Date.now() - start;

		const cf = (request as any).cf;
		const url = new URL(request.url);

		env.ANALYTICS.writeDataPoint({
			indexes: ['simse-cdn'],
			blobs: [
				request.method,
				url.pathname,
				String(response.status),
				'simse-cdn',
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
		});

		return response;
	},
} satisfies ExportedHandler<Env>;

async function handleRequest(request: Request, env: Env): Promise<Response> {
	const url = new URL(request.url);
	const path = url.pathname;

	if (path === '/health') {
		return new Response('ok', { status: 200 });
	}

	const mediaMatch = path.match(/^\/media\/(.+)$/);
	if (mediaMatch) {
		return serveR2(env.CDN_BUCKET, `media/${mediaMatch[1]}`, {
			immutable: true,
		});
	}

	const versionedMatch = path.match(
		/^\/download\/([^/]+)\/([^/]+)\/([^/]+)$/,
	);
	if (versionedMatch && versionedMatch[1] !== 'latest') {
		const [, version, os, arch] = versionedMatch;
		const platform = `${os}/${arch}`;
		const filename = BINARY_FILENAMES[platform];
		if (!filename) {
			return new Response('unknown platform', { status: 404 });
		}
		const key = `releases/${os}/${arch}/${version}/${filename}`;
		return serveR2(env.CDN_BUCKET, key, {
			immutable: true,
			binary: true,
			filename,
		});
	}

	const latestMatch = path.match(/^\/download\/latest\/([^/]+)\/([^/]+)$/);
	if (latestMatch) {
		const [, os, arch] = latestMatch;
		const kvKey = `latest:${os}-${arch}`;
		const version = await env.VERSION_STORE.get(kvKey);
		if (!version) {
			return new Response('unknown platform', { status: 404 });
		}
		return new Response(null, {
			status: 301,
			headers: {
				Location: `/download/${version}/${os}/${arch}`,
				'Cache-Control': 'no-store',
			},
		});
	}

	return new Response('not found', { status: 404 });
}
```

This extracts the original logic into a `handleRequest` function and wraps it with timing + analytics.

**Step 4: Run tests**

```bash
cd simse-cdn && npm run test
```

Expected: All 8 tests pass. The tests use `@cloudflare/vitest-pool-workers` which should handle the new `ANALYTICS` binding gracefully (it's a write-only binding, tests don't read from it).

**Step 5: Commit**

```bash
git add simse-cdn/wrangler.toml simse-cdn/src/types.ts simse-cdn/src/index.ts
git commit -m "feat(simse-cdn): add Workers Analytics Engine"
```

---

### Task 6: Add analytics to simse-cloud

The cloud app uses React Router v7 on Cloudflare Pages with a `worker.ts` entry point.

**Files:**
- Modify: `simse-cloud/wrangler.toml`
- Modify: `simse-cloud/app/env.d.ts`
- Modify: `simse-cloud/worker.ts`

**Step 1: Add Analytics Engine binding to wrangler.toml**

Append to `simse-cloud/wrangler.toml`:

```toml
[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"
```

**Step 2: Update Env interface**

In `simse-cloud/app/env.d.ts`:

```typescript
interface Env {
	APP_URL: string;
	ANALYTICS: AnalyticsEngineDataset;
}
```

**Step 3: Add analytics to worker.ts**

Update `simse-cloud/worker.ts` to wrap the request handler with analytics:

```typescript
import { createRequestHandler } from 'react-router';

declare module 'react-router' {
	export interface AppLoadContext {
		cloudflare: {
			env: Env;
			ctx: ExecutionContext;
		};
	}
}

const requestHandler = createRequestHandler(
	() => import('virtual:react-router/server-build'),
	import.meta.env.MODE,
);

export default {
	async fetch(request, env, ctx) {
		const start = Date.now();
		const response = await requestHandler(request, {
			cloudflare: { env, ctx },
		});
		const latencyMs = Date.now() - start;

		const cf = (request as any).cf;
		const url = new URL(request.url);

		ctx.waitUntil(
			Promise.resolve(
				env.ANALYTICS.writeDataPoint({
					indexes: ['simse-cloud'],
					blobs: [
						request.method,
						url.pathname,
						String(response.status),
						'simse-cloud',
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
} satisfies ExportedHandler<Env>;
```

Note: `ctx.waitUntil` ensures the analytics write doesn't delay the response to the user.

**Step 4: Commit**

```bash
git add simse-cloud/wrangler.toml simse-cloud/app/env.d.ts simse-cloud/worker.ts
git commit -m "feat(simse-cloud): add Workers Analytics Engine"
```

---

### Task 7: Add analytics to simse-landing

The landing app uses React Router v7 on Cloudflare Pages but has no `worker.ts` — it relies on the default Pages handler. We need to create a `worker.ts` entry.

**Files:**
- Modify: `simse-landing/wrangler.toml`
- Create: `simse-landing/worker.ts`
- Modify: `simse-landing/react-router.config.ts` (if needed to wire custom worker)

**Step 1: Add Analytics Engine binding to wrangler.toml**

Append to `simse-landing/wrangler.toml`:

```toml
[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"
```

**Step 2: Create worker.ts**

Create `simse-landing/worker.ts` following the same pattern as simse-cloud:

```typescript
import { createRequestHandler } from 'react-router';

declare module 'react-router' {
	export interface AppLoadContext {
		cloudflare: {
			env: {
				DB: D1Database;
				COMMS_QUEUE: Queue;
				ANALYTICS: AnalyticsEngineDataset;
			};
			ctx: ExecutionContext;
		};
	}
}

const requestHandler = createRequestHandler(
	() => import('virtual:react-router/server-build'),
	import.meta.env.MODE,
);

export default {
	async fetch(request, env, ctx) {
		const start = Date.now();
		const response = await requestHandler(request, {
			cloudflare: { env, ctx },
		});
		const latencyMs = Date.now() - start;

		const cf = (request as any).cf;
		const url = new URL(request.url);

		ctx.waitUntil(
			Promise.resolve(
				env.ANALYTICS.writeDataPoint({
					indexes: ['simse-landing'],
					blobs: [
						request.method,
						url.pathname,
						String(response.status),
						'simse-landing',
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
} satisfies ExportedHandler;
```

**Step 3: Commit**

```bash
git add simse-landing/wrangler.toml simse-landing/worker.ts
git commit -m "feat(simse-landing): add Workers Analytics Engine"
```

---

### Task 8: Update CDN tests for ANALYTICS binding

The CDN tests use `@cloudflare/vitest-pool-workers`. The new `ANALYTICS` binding needs to be available in the test environment.

**Files:**
- Modify: `simse-cdn/src/index.test.ts` (or `simse-cdn/src/test-setup.ts` if the binding needs mocking)
- Modify: `simse-cdn/vitest.config.ts` (if binding needs wrangler config)

**Step 1: Check if tests pass as-is**

```bash
cd simse-cdn && npm run test
```

If tests pass (the `@cloudflare/vitest-pool-workers` pool reads `wrangler.toml` and should pick up the new binding automatically), skip to Step 3.

**Step 2: If tests fail, add ANALYTICS mock**

In `simse-cdn/src/test-setup.ts`, add a no-op mock for the analytics binding if needed. The `AnalyticsEngineDataset` only has a `writeDataPoint` method that returns void:

```typescript
// Add to test setup if pool doesn't auto-provide ANALYTICS
```

**Step 3: Run tests to confirm**

```bash
cd simse-cdn && npm run test
```

Expected: All 8 tests pass.

**Step 4: Commit (only if changes were needed)**

```bash
git add simse-cdn/
git commit -m "test(simse-cdn): add ANALYTICS binding to test environment"
```

---

### Task 9: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Update CLAUDE.md**

Add a note about the analytics setup in the Key Patterns section. Add after the existing patterns:

```markdown
- **Workers Analytics Engine**: All 7 Cloudflare Workers write to a shared `simse-analytics` dataset via `ANALYTICS: AnalyticsEngineDataset` binding. Data points include method, path, status, latency, userId, geo (country/city/continent), userAgent, and cfRay. Hono workers use an analytics middleware; CDN wraps its fetch handler; Pages workers wrap worker.ts.
```

**Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add Workers Analytics Engine to CLAUDE.md"
```

---

### Task 10: Final verification

**Step 1: Lint all TypeScript services**

```bash
cd simse-api && npm run lint
cd ../simse-auth && npm run lint
cd ../simse-payments && npm run lint
cd ../simse-cdn && npm run lint
```

Expected: All pass.

**Step 2: Run CDN tests**

```bash
cd simse-cdn && npm run test
```

Expected: All 8 tests pass.

**Step 3: Push**

```bash
git push origin main
```
