# simse-analytics Service Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a centralized analytics service that is the sole writer to Cloudflare Analytics Engine and D1 audit store, consuming events from all 8 existing services via a dedicated queue.

**Architecture:** New CF Worker (`simse-analytics`) consumes `ANALYTICS_QUEUE`. All 8 services replace direct `ANALYTICS.writeDataPoint()` calls with `ANALYTICS_QUEUE.send()`. Audit events move from `COMMS_QUEUE` to `ANALYTICS_QUEUE`. The analytics service writes datapoints to Analytics Engine and persists audit events in D1.

**Tech Stack:** Hono, Cloudflare Workers, Cloudflare Queues, D1, Analytics Engine, Biome, TypeScript

**Design doc:** `docs/plans/2026-03-06-simse-analytics-design.md`

---

### Task 1: Scaffold simse-analytics service

**Files:**
- Create: `simse-analytics/package.json`
- Create: `simse-analytics/tsconfig.json`
- Create: `simse-analytics/biome.json`
- Create: `simse-analytics/wrangler.toml`
- Create: `simse-analytics/src/types.ts`

**Step 1: Create package.json**

```json
{
	"name": "simse-analytics",
	"private": true,
	"type": "module",
	"scripts": {
		"dev": "wrangler dev",
		"build": "wrangler deploy --dry-run --outdir dist",
		"deploy": "wrangler deploy",
		"lint": "biome check .",
		"lint:fix": "biome check --write .",
		"db:migrate": "wrangler d1 migrations apply simse-analytics-db --local",
		"db:migrate:prod": "wrangler d1 migrations apply simse-analytics-db --remote"
	},
	"dependencies": {
		"hono": "^4.7.0"
	},
	"devDependencies": {
		"@biomejs/biome": "^2.3.12",
		"@cloudflare/workers-types": "^4.20260305.0",
		"wrangler": "^4.14.4"
	}
}
```

**Step 2: Create tsconfig.json**

```json
{
	"compilerOptions": {
		"target": "ESNext",
		"module": "ESNext",
		"moduleResolution": "bundler",
		"strict": true,
		"noEmit": true,
		"types": ["@cloudflare/workers-types"]
	},
	"include": ["src"]
}
```

**Step 3: Create biome.json**

Copy from `simse-auth/biome.json` — identical config (tabs, single quotes, organize imports).

**Step 4: Create wrangler.toml**

```toml
name = "simse-analytics"
compatibility_date = "2025-04-01"
main = "src/index.ts"

workers_dev = true

[[d1_databases]]
binding = "DB"
database_name = "simse-analytics-db"
database_id = "PLACEHOLDER_FILL_AFTER_CREATION"

[analytics_engine]
dataset = "simse-analytics"
binding = "ANALYTICS"

# Consumes analytics events from all services
[[queues.consumers]]
queue = "simse-api-analytics"

[[queues.consumers]]
queue = "simse-auth-analytics"

[[queues.consumers]]
queue = "simse-cdn-analytics"

[[queues.consumers]]
queue = "simse-cloud-analytics"

[[queues.consumers]]
queue = "simse-mailer-analytics"

[[queues.consumers]]
queue = "simse-payments-analytics"

[[queues.consumers]]
queue = "simse-landing-analytics"

[[queues.consumers]]
queue = "simse-status-analytics"

# --- Environments ---

[env.staging]
name = "simse-analytics-staging"
workers_dev = true

[[env.staging.d1_databases]]
binding = "DB"
database_name = "simse-analytics-db-staging"
database_id = "PLACEHOLDER_STAGING_ANALYTICS_DB_ID"

[[env.staging.queues.consumers]]
queue = "simse-api-analytics-staging"

[[env.staging.queues.consumers]]
queue = "simse-auth-analytics-staging"

[[env.staging.queues.consumers]]
queue = "simse-cdn-analytics-staging"

[[env.staging.queues.consumers]]
queue = "simse-cloud-analytics-staging"

[[env.staging.queues.consumers]]
queue = "simse-mailer-analytics-staging"

[[env.staging.queues.consumers]]
queue = "simse-payments-analytics-staging"

[[env.staging.queues.consumers]]
queue = "simse-landing-analytics-staging"

[[env.staging.queues.consumers]]
queue = "simse-status-analytics-staging"

[env.production]
name = "simse-analytics"

[[env.production.d1_databases]]
binding = "DB"
database_name = "simse-analytics-db"
database_id = "PLACEHOLDER_FILL_AFTER_CREATION"

[[env.production.queues.consumers]]
queue = "simse-api-analytics"

[[env.production.queues.consumers]]
queue = "simse-auth-analytics"

[[env.production.queues.consumers]]
queue = "simse-cdn-analytics"

[[env.production.queues.consumers]]
queue = "simse-cloud-analytics"

[[env.production.queues.consumers]]
queue = "simse-mailer-analytics"

[[env.production.queues.consumers]]
queue = "simse-payments-analytics"

[[env.production.queues.consumers]]
queue = "simse-landing-analytics"

[[env.production.queues.consumers]]
queue = "simse-status-analytics"
```

**Step 5: Create src/types.ts**

```typescript
export interface Env {
	DB: D1Database;
	ANALYTICS: AnalyticsEngineDataset;
}

export interface DatapointMessage {
	type: 'datapoint';
	service: string;
	method: string;
	path: string;
	status: number;
	userId?: string;
	teamId?: string;
	country?: string;
	city?: string;
	continent?: string;
	userAgent?: string;
	referer?: string;
	contentType?: string;
	cfRay?: string;
	latencyMs: number;
	requestSize: number;
	responseSize: number;
	colo?: number;
}

export interface AuditMessage {
	type: 'audit';
	action: string;
	userId: string;
	timestamp: string;
	[key: string]: string;
}

export type AnalyticsMessage = DatapointMessage | AuditMessage;
```

**Step 6: Install dependencies**

Run: `cd simse-analytics && npm install`

**Step 7: Commit**

```bash
git add simse-analytics/
git commit -m "feat: scaffold simse-analytics service"
```

---

### Task 2: Create D1 migration for audit_events table

**Files:**
- Create: `simse-analytics/migrations/0001_initial.sql`

**Step 1: Create migration**

```sql
CREATE TABLE audit_events (
  id TEXT PRIMARY KEY,
  action TEXT NOT NULL,
  user_id TEXT NOT NULL,
  metadata TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_audit_user ON audit_events(user_id);
CREATE INDEX idx_audit_action ON audit_events(action);
CREATE INDEX idx_audit_created ON audit_events(created_at);
```

**Step 2: Commit**

```bash
git add simse-analytics/migrations/
git commit -m "feat: add audit_events D1 migration"
```

---

### Task 3: Implement the analytics service entry point

**Files:**
- Create: `simse-analytics/src/index.ts`

**Step 1: Write the service**

The service has two responsibilities:
1. **Queue handler** — processes `DatapointMessage` and `AuditMessage` batches
2. **HTTP handler** — `GET /health` and `GET /audit/:userId`

```typescript
import { Hono } from 'hono';
import type { AnalyticsMessage, AuditMessage, DatapointMessage, Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));

app.get('/audit/:userId', async (c) => {
	const userId = c.req.param('userId');
	const limit = Math.min(Number(c.req.query('limit') ?? 50), 200);
	const offset = Number(c.req.query('offset') ?? 0);

	const rows = await c.env.DB.prepare(
		'SELECT id, action, user_id, metadata, created_at FROM audit_events WHERE user_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?',
	)
		.bind(userId, limit, offset)
		.all<{
			id: string;
			action: string;
			user_id: string;
			metadata: string | null;
			created_at: string;
		}>();

	return c.json({
		data: rows.results.map((r) => ({
			id: r.id,
			action: r.action,
			userId: r.user_id,
			metadata: r.metadata ? JSON.parse(r.metadata) : null,
			createdAt: r.created_at,
		})),
	});
});

function writeDatapoint(env: Env, msg: DatapointMessage): void {
	env.ANALYTICS.writeDataPoint({
		indexes: [msg.service],
		blobs: [
			msg.method,
			msg.path,
			String(msg.status),
			msg.service,
			msg.userId ?? '',
			msg.teamId ?? '',
			msg.country ?? '',
			msg.city ?? '',
			msg.continent ?? '',
			(msg.userAgent ?? '').slice(0, 256),
			(msg.referer ?? '').split('?')[0],
			msg.contentType ?? '',
			msg.cfRay ?? '',
		],
		doubles: [
			msg.latencyMs,
			msg.status,
			msg.requestSize,
			msg.responseSize,
			msg.colo ?? 0,
		],
	});
}

async function writeAudit(env: Env, msg: AuditMessage): Promise<void> {
	const { type, action, userId, timestamp, ...rest } = msg;
	const id = crypto.randomUUID();
	const metadata = Object.keys(rest).length > 0 ? JSON.stringify(rest) : null;

	await env.DB.prepare(
		'INSERT INTO audit_events (id, action, user_id, metadata, created_at) VALUES (?, ?, ?, ?, ?)',
	)
		.bind(id, action, userId, metadata, timestamp)
		.run();

	// Also write to Analytics Engine for dashboards
	env.ANALYTICS.writeDataPoint({
		indexes: ['audit'],
		blobs: [action, userId, timestamp, metadata ?? '', '', '', '', '', '', '', '', '', ''],
		doubles: [0, 0, 0, 0, 0],
	});
}

export default {
	fetch: app.fetch,

	async queue(batch: MessageBatch<AnalyticsMessage>, env: Env): Promise<void> {
		for (const message of batch.messages) {
			const msg = message.body;
			try {
				if (msg.type === 'datapoint') {
					writeDatapoint(env, msg);
				} else if (msg.type === 'audit') {
					await writeAudit(env, msg);
				}
				message.ack();
			} catch (e) {
				if (msg.type === 'audit') {
					// Audit failures should retry — data must not be lost
					console.error('Audit write failed', e);
					message.retry();
				} else {
					// Analytics datapoint failures are non-critical
					console.error('Datapoint write failed', e);
					message.ack();
				}
			}
		}
	},
};
```

**Step 2: Run lint**

Run: `cd simse-analytics && npx biome check src/`
Expected: No errors

**Step 3: Commit**

```bash
git add simse-analytics/src/
git commit -m "feat: implement analytics service queue handler and HTTP routes"
```

---

### Task 4: Migrate simse-api — replace ANALYTICS with ANALYTICS_QUEUE

**Files:**
- Modify: `simse-api/wrangler.toml` — replace `[analytics_engine]` with `[[queues.producers]]`, all envs
- Modify: `simse-api/src/types.ts` — replace `ANALYTICS: AnalyticsEngineDataset` with `ANALYTICS_QUEUE: Queue`
- Modify: `simse-api/src/middleware/analytics.ts` — rewrite to send queue messages
- Modify: `simse-api/src/index.ts` — no changes needed (middleware name stays the same)

**Step 1: Update wrangler.toml**

Replace these lines in all three sections (top-level, staging, production):

```toml
# Top-level: replace [analytics_engine] block (lines 25-27) with:
[[queues.producers]]
queue = "simse-api-analytics"
binding = "ANALYTICS_QUEUE"

# env.staging: add after existing queue producer:
[[env.staging.queues.producers]]
queue = "simse-api-analytics-staging"
binding = "ANALYTICS_QUEUE"

# env.production: add:
[[env.production.queues.producers]]
queue = "simse-api-analytics"
binding = "ANALYTICS_QUEUE"
```

**Step 2: Update types.ts**

Replace `ANALYTICS: AnalyticsEngineDataset` with `ANALYTICS_QUEUE: Queue` in the Env interface.

**Step 3: Rewrite analytics middleware**

Replace entire `simse-api/src/middleware/analytics.ts` with:

```typescript
import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const analyticsMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	const cf = (c.req.raw as Request & { cf?: IncomingRequestCfProperties }).cf;

	try {
		c.env.ANALYTICS_QUEUE.send({
			type: 'datapoint',
			service: 'simse-api',
			method: c.req.method,
			path: c.req.path,
			status: c.res.status,
			userId: c.req.header('X-User-Id') ?? '',
			teamId: c.req.header('X-Team-Id') ?? '',
			country: cf?.country ?? '',
			city: cf?.city ?? '',
			continent: cf?.continent ?? '',
			userAgent: (c.req.header('User-Agent') ?? '').slice(0, 256),
			referer: (c.req.header('Referer') ?? '').split('?')[0],
			contentType: c.res.headers.get('Content-Type') ?? '',
			cfRay: c.req.header('Cf-Ray') ?? '',
			latencyMs,
			requestSize: Number(c.req.header('Content-Length') ?? 0),
			responseSize: Number(c.res.headers.get('Content-Length') ?? 0),
			colo: Number(cf?.colo ?? 0),
		});
	} catch {
		// Analytics should never block requests
	}
});
```

**Step 4: Run lint**

Run: `cd simse-api && npx biome check src/`
Expected: No errors

**Step 5: Commit**

```bash
git add simse-api/
git commit -m "refactor: simse-api sends analytics to queue instead of direct writes"
```

---

### Task 5: Migrate simse-auth — replace ANALYTICS with ANALYTICS_QUEUE, move audit

**Files:**
- Modify: `simse-auth/wrangler.toml` — replace `[analytics_engine]` with `[[queues.producers]]`, all envs
- Modify: `simse-auth/src/types.ts` — replace `ANALYTICS` with `ANALYTICS_QUEUE: Queue`
- Modify: `simse-auth/src/middleware/analytics.ts` — rewrite to send queue messages
- Modify: `simse-auth/src/lib/audit.ts` — change from `COMMS_QUEUE` to `ANALYTICS_QUEUE`
- Modify: `simse-auth/src/routes/auth.ts` — pass `c.env.ANALYTICS_QUEUE` to `sendAuditEvent`
- Modify: `simse-auth/src/routes/users.ts` — pass `c.env.ANALYTICS_QUEUE` to `sendAuditEvent`
- Modify: `simse-auth/src/routes/teams.ts` — pass `c.env.ANALYTICS_QUEUE` to `sendAuditEvent`
- Modify: `simse-auth/src/routes/api-keys.ts` — pass `c.env.ANALYTICS_QUEUE` to `sendAuditEvent`

**Step 1: Update wrangler.toml**

Replace `[analytics_engine]` block (lines 20-22) with:

```toml
[[queues.producers]]
queue = "simse-auth-analytics"
binding = "ANALYTICS_QUEUE"
```

No staging/production analytics_engine entries exist — just add the queue producers to those sections.

**Step 2: Update types.ts**

```typescript
export interface Env {
	DB: D1Database;
	COMMS_QUEUE: Queue;
	ANALYTICS_QUEUE: Queue;
	SECRETS: SecretsStoreNamespace;
}
```

**Step 3: Rewrite analytics middleware**

Same pattern as simse-api but with `service: 'simse-auth'` and the `biome-ignore` comment for `cf`:

```typescript
import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

export const analyticsMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	const start = Date.now();
	await next();
	const latencyMs = Date.now() - start;

	// biome-ignore lint/suspicious/noExplicitAny: cf properties not in Request type
	const cf = (c.req.raw as any).cf;

	try {
		c.env.ANALYTICS_QUEUE.send({
			type: 'datapoint',
			service: 'simse-auth',
			method: c.req.method,
			path: c.req.path,
			status: c.res.status,
			userId: c.req.header('X-User-Id') ?? '',
			teamId: c.req.header('X-Team-Id') ?? '',
			country: cf?.country ?? '',
			city: cf?.city ?? '',
			continent: cf?.continent ?? '',
			userAgent: (c.req.header('User-Agent') ?? '').slice(0, 256),
			referer: (c.req.header('Referer') ?? '').split('?')[0],
			contentType: c.res.headers.get('Content-Type') ?? '',
			cfRay: c.req.header('Cf-Ray') ?? '',
			latencyMs,
			requestSize: Number(c.req.header('Content-Length') ?? 0),
			responseSize: Number(c.res.headers.get('Content-Length') ?? 0),
			colo: Number(cf?.colo ?? 0),
		});
	} catch {
		// Analytics should never block requests
	}
});
```

**Step 4: Update audit.ts — change queue parameter**

The `sendAuditEvent` function signature stays the same (accepts `Queue`), but callers will pass `ANALYTICS_QUEUE` instead of `COMMS_QUEUE`. No changes to `audit.ts` itself.

**Step 5: Update all route files**

In each route file, change `c.env.COMMS_QUEUE` to `c.env.ANALYTICS_QUEUE` in `sendAuditEvent` calls:

- `simse-auth/src/routes/auth.ts`: `sendAuditEvent(c.env.ANALYTICS_QUEUE, ...)` (1 call)
- `simse-auth/src/routes/users.ts`: `sendAuditEvent(c.env.ANALYTICS_QUEUE, ...)` (2 calls)
- `simse-auth/src/routes/teams.ts`: `sendAuditEvent(c.env.ANALYTICS_QUEUE, ...)` (3 calls)
- `simse-auth/src/routes/api-keys.ts`: `sendAuditEvent(c.env.ANALYTICS_QUEUE, ...)` (2 calls)

Use find-and-replace: `c.env.COMMS_QUEUE, '` → `c.env.ANALYTICS_QUEUE, '` in those 4 files (only affects audit calls since `sendEmail` uses a different pattern).

**Step 6: Run lint**

Run: `cd simse-auth && npx biome check src/`
Expected: No errors

**Step 7: Commit**

```bash
git add simse-auth/
git commit -m "refactor: simse-auth sends analytics and audit events to ANALYTICS_QUEUE"
```

---

### Task 6: Migrate simse-payments — replace ANALYTICS with ANALYTICS_QUEUE

**Files:**
- Modify: `simse-payments/wrangler.toml` — replace `[analytics_engine]` with `[[queues.producers]]`
- Modify: `simse-payments/src/types.ts` — replace `ANALYTICS` with `ANALYTICS_QUEUE: Queue`
- Modify: `simse-payments/src/middleware/analytics.ts` — rewrite to send queue messages

**Step 1: Update wrangler.toml**

Replace `[analytics_engine]` block (lines 17-19) with:

```toml
[[queues.producers]]
queue = "simse-payments-analytics"
binding = "ANALYTICS_QUEUE"
```

Add to staging and production sections too.

**Step 2: Update types.ts**

Replace `ANALYTICS: AnalyticsEngineDataset` with `ANALYTICS_QUEUE: Queue`.

**Step 3: Rewrite analytics middleware**

Same pattern as simse-auth but with `service: 'simse-payments'`.

**Step 4: Run lint**

Run: `cd simse-payments && npx biome check src/`
Expected: No errors

**Step 5: Commit**

```bash
git add simse-payments/
git commit -m "refactor: simse-payments sends analytics to queue"
```

---

### Task 7: Migrate simse-mailer — replace ANALYTICS with ANALYTICS_QUEUE

**Files:**
- Modify: `simse-mailer/wrangler.toml` — replace `[analytics_engine]` with `[[queues.producers]]`
- Modify: `simse-mailer/src/index.ts` — replace all `env.ANALYTICS.writeDataPoint(...)` with `env.ANALYTICS_QUEUE.send(...)`, update Env interface, remove audit ack

**This is the most complex migration** because simse-mailer has 4 separate analytics write sites (HTTP middleware + 3 queue handler writes) with a non-standard blob schema for queue events.

**Step 1: Update wrangler.toml**

Replace `[analytics_engine]` block (lines 23-25) with:

```toml
[[queues.producers]]
queue = "simse-mailer-analytics"
binding = "ANALYTICS_QUEUE"
```

Add to staging and production sections too.

**Step 2: Update Env interface in index.ts**

```typescript
export interface Env {
	DB: D1Database;
	SECRETS: SecretsStoreNamespace;
	ANALYTICS_QUEUE: Queue;
}
```

**Step 3: Update HTTP analytics middleware**

Replace `c.env.ANALYTICS?.writeDataPoint({...})` with `c.env.ANALYTICS_QUEUE.send({type: 'datapoint', service: 'simse-mailer', ...})`.

**Step 4: Update queue handler analytics writes**

Replace all 4 `env.ANALYTICS.writeDataPoint(...)` calls in the queue handler with `env.ANALYTICS_QUEUE.send(...)`:
- Email success (line ~147): send datapoint with `service: 'simse-mailer'`, use `method: 'queue'`, `path: msg.template`
- Notification success (line ~174): send datapoint with `method: 'queue'`, `path: msg.kind`
- Error (line ~205): send datapoint with `method: 'queue'`, `path: label`
- Batch complete (line ~220): send datapoint with `method: 'queue'`, `path: 'batch'`

**Step 5: Run lint**

Run: `cd simse-mailer && npx biome check src/`

**Step 6: Commit**

```bash
git add simse-mailer/
git commit -m "refactor: simse-mailer sends analytics to queue"
```

---

### Task 8: Migrate simse-cdn — replace ANALYTICS with ANALYTICS_QUEUE

**Files:**
- Modify: `simse-cdn/wrangler.toml` — replace `[analytics_engine]` with `[[queues.producers]]`
- Modify: `simse-cdn/src/types.ts` — replace `ANALYTICS` with `ANALYTICS_QUEUE: Queue`
- Modify: `simse-cdn/src/index.ts` — replace `env.ANALYTICS?.writeDataPoint(...)` with `env.ANALYTICS_QUEUE.send(...)`

**Step 1: Update wrangler.toml**

Replace `[analytics_engine]` block (lines 17-19) with:

```toml
[[queues.producers]]
queue = "simse-cdn-analytics"
binding = "ANALYTICS_QUEUE"
```

**Step 2: Update types.ts**

Replace `ANALYTICS: AnalyticsEngineDataset` with `ANALYTICS_QUEUE: Queue`.

**Step 3: Update index.ts**

Replace the `env.ANALYTICS?.writeDataPoint({...})` block (lines 22-46) with:

```typescript
env.ANALYTICS_QUEUE.send({
	type: 'datapoint',
	service: 'simse-cdn',
	method: request.method,
	path: url.pathname,
	status: response.status,
	country: cf?.country ?? '',
	city: cf?.city ?? '',
	continent: cf?.continent ?? '',
	userAgent: (request.headers.get('User-Agent') ?? '').slice(0, 256),
	referer: (request.headers.get('Referer') ?? '').split('?')[0],
	contentType: response.headers.get('Content-Type') ?? '',
	cfRay: request.headers.get('Cf-Ray') ?? '',
	latencyMs,
	requestSize: Number(request.headers.get('Content-Length') ?? 0),
	responseSize: Number(response.headers.get('Content-Length') ?? 0),
	colo: Number(cf?.colo ?? 0),
}).catch(() => {});
```

Note: CDN uses `env.ANALYTICS?.writeDataPoint` (optional chaining) — the queue send should use `.catch(() => {})` instead.

**Step 4: Run lint and tests**

Run: `cd simse-cdn && npx biome check src/ && npm run test`

**Step 5: Commit**

```bash
git add simse-cdn/
git commit -m "refactor: simse-cdn sends analytics to queue"
```

---

### Task 9: Migrate simse-cloud — replace ANALYTICS with ANALYTICS_QUEUE

**Files:**
- Modify: `simse-cloud/wrangler.toml` — replace `[analytics_engine]` with `[[queues.producers]]`
- Modify: `simse-cloud/app/env.d.ts` — replace `ANALYTICS` with `ANALYTICS_QUEUE: Queue`
- Modify: `simse-cloud/worker.ts` — replace `env.ANALYTICS.writeDataPoint(...)` with `env.ANALYTICS_QUEUE.send(...)`

**Step 1: Update wrangler.toml**

Replace `[analytics_engine]` block (lines 17-19) with:

```toml
[[queues.producers]]
queue = "simse-cloud-analytics"
binding = "ANALYTICS_QUEUE"
```

Add to staging and production sections.

**Step 2: Update env.d.ts**

```typescript
interface Env {
	APP_URL: string;
	ANALYTICS_QUEUE: Queue;
	TUNNEL_SESSION: DurableObjectNamespace;
}
```

**Step 3: Update worker.ts**

Replace the `env.ANALYTICS.writeDataPoint({...})` call inside `ctx.waitUntil(Promise.resolve(...))` with `env.ANALYTICS_QUEUE.send({type: 'datapoint', service: 'simse-cloud', ...})`. Keep the `ctx.waitUntil` wrapper.

**Step 4: Run lint**

Run: `cd simse-cloud && npx biome check worker.ts`

**Step 5: Commit**

```bash
git add simse-cloud/
git commit -m "refactor: simse-cloud sends analytics to queue"
```

---

### Task 10: Migrate simse-landing — replace ANALYTICS with ANALYTICS_QUEUE

**Files:**
- Modify: `simse-landing/wrangler.toml` — replace `[analytics_engine]` with `[[queues.producers]]`
- Modify: `simse-landing/worker.ts` — update Env type and replace writeDataPoint

**Step 1: Update wrangler.toml**

Replace `[analytics_engine]` block (lines 18-20) with:

```toml
[[queues.producers]]
queue = "simse-landing-analytics"
binding = "ANALYTICS_QUEUE"
```

Add to staging and production sections.

**Step 2: Update worker.ts**

Update the inline Env type: replace `ANALYTICS: AnalyticsEngineDataset` with `ANALYTICS_QUEUE: Queue`.

Replace `env.ANALYTICS.writeDataPoint({...})` with `env.ANALYTICS_QUEUE.send({type: 'datapoint', service: 'simse-landing', ...})`. Keep the `ctx.waitUntil` wrapper.

**Step 3: Run lint**

Run: `cd simse-landing && npx biome check worker.ts`

**Step 4: Commit**

```bash
git add simse-landing/
git commit -m "refactor: simse-landing sends analytics to queue"
```

---

### Task 11: Migrate simse-status — replace ANALYTICS with ANALYTICS_QUEUE

**Files:**
- Modify: `simse-status/wrangler.toml` — replace `[analytics_engine]` with `[[queues.producers]]`
- Modify: `simse-status/worker.ts` — update Env type and replace writeDataPoint

**Step 1: Update wrangler.toml**

Replace `[analytics_engine]` block (lines 12-14) with:

```toml
[[queues.producers]]
queue = "simse-status-analytics"
binding = "ANALYTICS_QUEUE"
```

**Step 2: Update worker.ts**

Update inline Env interface: replace `ANALYTICS: AnalyticsEngineDataset` with `ANALYTICS_QUEUE: Queue`.

Replace `env.ANALYTICS.writeDataPoint({...})` with `env.ANALYTICS_QUEUE.send({type: 'datapoint', service: 'simse-status', ...})`. Keep the `ctx.waitUntil` wrapper.

**Step 3: Run lint**

Run: `cd simse-status && npx biome check worker.ts`

**Step 4: Commit**

```bash
git add simse-status/
git commit -m "refactor: simse-status sends analytics to queue"
```

---

### Task 12: Update CLAUDE.md and final verification

**Files:**
- Modify: `CLAUDE.md` — add simse-analytics to repository layout, architecture docs

**Step 1: Add simse-analytics to CLAUDE.md**

Add to the Repository Layout section:

```
simse-analytics/            # TypeScript — Analytics + audit service (Cloudflare Worker, D1, Queues, Analytics Engine)
```

Add to TypeScript Services section with route table and architecture notes.

Add to Key Patterns: "All services produce analytics/audit events to per-service `ANALYTICS_QUEUE` queues consumed by `simse-analytics`, which is the sole writer to the Analytics Engine dataset and D1 audit store."

**Step 2: Run lint across all modified services**

```bash
cd simse-analytics && npx biome check src/
cd ../simse-api && npx biome check src/
cd ../simse-auth && npx biome check src/
cd ../simse-payments && npx biome check src/
cd ../simse-mailer && npx biome check src/
cd ../simse-cdn && npx biome check src/
cd ../simse-cloud && npx biome check worker.ts
cd ../simse-landing && npx biome check worker.ts
cd ../simse-status && npx biome check worker.ts
```

Expected: All pass with no errors.

**Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add simse-analytics to CLAUDE.md"
```
