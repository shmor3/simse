# Cloudflare Secrets Store Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace all `wrangler secret put` secrets and plain `[vars]` values in simse-api and simse-mailer with a single shared Cloudflare Secrets Store (`simse-secrets`).

**Architecture:** One shared `simse-secrets` store holds all 7 secrets. Two workers (simse-api, simse-mailer) bind to it via `SECRETS: SecretsStoreNamespace`. A Hono middleware fetches each service's needed secrets in parallel at request start and puts them on `c.var.secrets`. The queue handler in simse-mailer fetches secrets directly from `env.SECRETS` at batch start.

**Tech Stack:** TypeScript, Hono, Cloudflare Workers, Cloudflare Secrets Store, Wrangler v4

**Prerequisites:** This plan applies on top of the service extraction plan (`feature/service-extraction`). Run after that branch is merged or from within that worktree.

---

### Task 1: Create the secrets store and populate secrets

**Files:**
- No code changes — CLI only

**Step 1: Create the store**

```bash
cd simse-api  # any worker dir with wrangler
wrangler secrets-store create simse-secrets
```

Expected output includes the store ID. Copy it — you'll need it for Tasks 2 and 3.

**Step 2: Add all 7 secrets**

```bash
wrangler secrets-store secret put --store-id <STORE_ID> --name AUTH_API_URL
wrangler secrets-store secret put --store-id <STORE_ID> --name AUTH_API_SECRET
wrangler secrets-store secret put --store-id <STORE_ID> --name PAYMENTS_API_URL
wrangler secrets-store secret put --store-id <STORE_ID> --name PAYMENTS_API_SECRET
wrangler secrets-store secret put --store-id <STORE_ID> --name MAILER_API_URL
wrangler secrets-store secret put --store-id <STORE_ID> --name RESEND_API_KEY
wrangler secrets-store secret put --store-id <STORE_ID> --name MAILER_API_SECRET
```

Each command prompts for the value. Enter the appropriate secret for each.

**Step 3: Verify**

```bash
wrangler secrets-store secret list --store-id <STORE_ID>
```

Expected: 7 secrets listed.

**Step 4: Commit (just the store ID note)**

Add the store ID to a `.env.example` or note file so it's tracked:

```bash
echo "SIMSE_SECRETS_STORE_ID=<STORE_ID>" >> .env.example
git add .env.example
git commit -m "chore: record simse-secrets store ID in .env.example"
```

---

### Task 2: Update simse-api — types, middleware, wrangler

**Files:**
- Modify: `simse-api/src/types.ts`
- Create: `simse-api/src/middleware/secrets.ts`
- Modify: `simse-api/wrangler.toml`

**Step 1: Read current types.ts**

Read `simse-api/src/types.ts` first. It currently has string fields for each secret on `Env`.

**Step 2: Rewrite src/types.ts**

```typescript
export interface Env {
	COMMS_QUEUE: Queue;
	SECRETS: SecretsStoreNamespace;
}

export interface ApiSecrets {
	authApiUrl: string;
	authApiSecret: string;
	paymentsApiUrl: string;
	paymentsApiSecret: string;
	mailerApiUrl: string;
	mailerApiSecret: string;
}

export interface ValidateResponse {
	data: {
		userId: string;
		sessionId?: string;
		teamId: string | null;
		role: string | null;
	};
}
```

**Step 3: Create src/middleware/secrets.ts**

```typescript
import { createMiddleware } from 'hono/factory';
import type { Env, ApiSecrets } from '../types';

export const secretsMiddleware = createMiddleware<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>(async (c, next) => {
	const [authApiUrl, authApiSecret, paymentsApiUrl, paymentsApiSecret, mailerApiUrl, mailerApiSecret] =
		await Promise.all([
			c.env.SECRETS.get('AUTH_API_URL'),
			c.env.SECRETS.get('AUTH_API_SECRET'),
			c.env.SECRETS.get('PAYMENTS_API_URL'),
			c.env.SECRETS.get('PAYMENTS_API_SECRET'),
			c.env.SECRETS.get('MAILER_API_URL'),
			c.env.SECRETS.get('MAILER_API_SECRET'),
		]);

	if (!authApiUrl || !authApiSecret || !paymentsApiUrl || !paymentsApiSecret || !mailerApiUrl || !mailerApiSecret) {
		return c.json({ error: { code: 'MISCONFIGURED', message: 'Service misconfigured' } }, 500);
	}

	c.set('secrets', { authApiUrl, authApiSecret, paymentsApiUrl, paymentsApiSecret, mailerApiUrl, mailerApiSecret });
	await next();
});
```

**Step 4: Update wrangler.toml**

Read `simse-api/wrangler.toml` first, then replace the secrets comment block with the store binding:

```toml
name = "simse-api"
compatibility_date = "2025-04-01"
main = "src/index.ts"

workers_dev = true

routes = [
  { pattern = "api.simse.dev", custom_domain = true }
]

# No database — pure gateway

[[queues.producers]]
queue = "simse-api-comms"
binding = "COMMS_QUEUE"

[[secrets_store.bindings]]
binding = "SECRETS"
store_id = "PLACEHOLDER_REPLACE_WITH_STORE_ID"
```

Replace `PLACEHOLDER_REPLACE_WITH_STORE_ID` with the actual store ID from Task 1.

**Step 5: Commit**

```
chore(simse-api): add Cloudflare Secrets Store binding
```

---

### Task 3: Update simse-api gateway to use c.var.secrets

**Files:**
- Modify: `simse-api/src/routes/gateway.ts`
- Modify: `simse-api/src/index.ts`

**Step 1: Read current gateway.ts**

Read `simse-api/src/routes/gateway.ts`. It currently uses `c.env.AUTH_API_URL`, `c.env.AUTH_API_SECRET`, `c.env.PAYMENTS_API_URL`, `c.env.PAYMENTS_API_SECRET`, `c.env.MAILER_API_URL`.

**Step 2: Update gateway.ts**

Replace all `c.env.AUTH_API_URL` → `c.var.secrets.authApiUrl`, `c.env.AUTH_API_SECRET` → `c.var.secrets.authApiSecret`, etc. The Hono type for the route must include the `Variables` constraint. Update the top of the file:

```typescript
import { Hono } from 'hono';
import type { Env, ApiSecrets, ValidateResponse } from '../types';

const gateway = new Hono<{ Bindings: Env; Variables: { secrets: ApiSecrets } }>();
```

Then update all `c.env.*` secret references:
- `c.env.AUTH_API_URL` → `c.var.secrets.authApiUrl`
- `c.env.AUTH_API_SECRET` → `c.var.secrets.authApiSecret`
- `c.env.PAYMENTS_API_URL` → `c.var.secrets.paymentsApiUrl`
- `c.env.PAYMENTS_API_SECRET` → `c.var.secrets.paymentsApiSecret`
- `c.env.MAILER_API_URL` → `c.var.secrets.mailerApiUrl`

Also update `proxyNotifications` to send `MAILER_API_SECRET` for HTTP reads:

```typescript
async function proxyNotifications(c: any) {
	const auth = await validateToken(c);
	if (!auth) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	// POST /notifications → enqueue (fire-and-forget)
	if (c.req.method === 'POST') {
		const body = await c.req.json();
		await c.env.COMMS_QUEUE.send({
			type: 'notification',
			userId: auth.userId,
			...body,
		});
		return c.json({ data: { ok: true } });
	}

	// GET/PUT → proxy to mailer HTTP (needs response)
	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.var.secrets.mailerApiSecret}`);
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);

	return proxyTo(c, `${c.var.secrets.mailerApiUrl}${c.req.path}`, headers);
}
```

Update `validateToken` to use `c.var.secrets.authApiUrl`:

```typescript
async function validateToken(c: any): Promise<ValidateResponse['data'] | null> {
	const authHeader = c.req.header('Authorization');
	if (!authHeader?.startsWith('Bearer ')) return null;

	const token = authHeader.slice(7);

	const res = await fetch(`${c.var.secrets.authApiUrl}/auth/validate`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	});

	if (!res.ok) return null;

	const json = (await res.json()) as ValidateResponse;
	return json.data;
}
```

**Step 3: Read current index.ts, then register secretsMiddleware**

Read `simse-api/src/index.ts`. Add the middleware import and register it before routes:

```typescript
import { Hono } from 'hono';
import { secretsMiddleware } from './middleware/secrets';
import gateway from './routes/gateway';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));
app.use('*', secretsMiddleware());
app.route('', gateway);

export default app;
```

**Step 4: Verify build**

```bash
cd simse-api && bun run build
```

Expected: Build succeeds with no TypeScript errors.

**Step 5: Commit**

```
refactor(simse-api): use Cloudflare Secrets Store for all secrets
```

---

### Task 4: Update simse-mailer — types, middleware, wrangler

**Files:**
- Modify: `simse-mailer/src/index.ts`
- Modify: `simse-mailer/wrangler.toml`

**Step 1: Read simse-mailer/src/index.ts and wrangler.toml**

Read both files. The index.ts currently has an inline `interface Env` with `RESEND_API_KEY: string` and `API_SECRET: string`.

**Step 2: Rewrite simse-mailer/src/index.ts**

Replace the inline `Env` interface and all `c.env.RESEND_API_KEY` / `c.env.API_SECRET` references. After the service extraction plan, index.ts exports both `fetch` and `queue` handlers. The full rewrite:

```typescript
import { Hono } from 'hono';
import { sendEmail } from './send';
import { renderTemplate } from './render';

export interface Env {
	DB: D1Database;
	SECRETS: SecretsStoreNamespace;
}

interface MailerSecrets {
	resendApiKey: string;
	mailerApiSecret: string;
}

type CommsMessage =
	| { type: 'email'; template: string; to: string; props?: Record<string, string> }
	| { type: 'notification'; userId: string; kind: string; title: string; body: string; link?: string };

const app = new Hono<{ Bindings: Env; Variables: { secrets: MailerSecrets } }>();

// Secrets middleware
app.use('*', async (c, next) => {
	const [resendApiKey, mailerApiSecret] = await Promise.all([
		c.env.SECRETS.get('RESEND_API_KEY'),
		c.env.SECRETS.get('MAILER_API_SECRET'),
	]);
	if (!resendApiKey || !mailerApiSecret) {
		return c.json({ error: 'Service misconfigured' }, 500);
	}
	c.set('secrets', { resendApiKey, mailerApiSecret });
	await next();
});

app.get('/health', (c) => c.json({ ok: true }));

app.post('/send', async (c) => {
	const authHeader = c.req.header('Authorization');
	if (authHeader !== `Bearer ${c.var.secrets.mailerApiSecret}`) {
		return c.json({ error: 'Unauthorized' }, 401);
	}

	const body = await c.req.json<{
		template: string;
		to: string;
		props?: Record<string, string>;
	}>();

	if (!body.to || !body.template) {
		return c.json({ error: 'Missing required fields: to, template' }, 400);
	}

	const { subject, html } = renderTemplate(body.template, body.props ?? {});
	await sendEmail(c.var.secrets.resendApiKey, { to: body.to, subject, html });
	return c.json({ success: true });
});

// Notification routes are in src/routes/notifications.ts — imported and mounted in Task 12 of service extraction plan

export default {
	async fetch(request: Request, env: Env): Promise<Response> {
		return app.fetch(request, env);
	},
	async queue(batch: MessageBatch<CommsMessage>, env: Env): Promise<void> {
		// Fetch secrets once for the whole batch
		const [resendApiKey] = await Promise.all([env.SECRETS.get('RESEND_API_KEY')]);
		if (!resendApiKey) {
			console.error('RESEND_API_KEY not configured — acking all messages to avoid poison pill');
			for (const message of batch.messages) message.ack();
			return;
		}

		for (const message of batch.messages) {
			const msg = message.body;
			try {
				if (msg.type === 'email') {
					const { subject, html } = renderTemplate(msg.template, msg.props ?? {});
					await sendEmail(resendApiKey, { to: msg.to, subject, html });
				} else if (msg.type === 'notification') {
					const id = crypto.randomUUID();
					await env.DB.prepare(
						'INSERT INTO notifications (id, user_id, type, title, body, link) VALUES (?, ?, ?, ?, ?, ?)',
					)
						.bind(id, msg.userId, msg.kind ?? 'info', msg.title, msg.body, msg.link ?? null)
						.run();
				}
				message.ack();
			} catch (e) {
				console.error('Queue processing error:', e);
				message.retry();
			}
		}
	},
};
```

**Note:** If `src/render.ts` and `src/routes/notifications.ts` don't exist yet (they're created in the service extraction plan's Tasks 11-12), skip those imports for now and add them once those tasks are done.

**Step 3: Update wrangler.toml**

Read `simse-mailer/wrangler.toml`, then rewrite:

```toml
name = "simse-mailer"
compatibility_date = "2025-04-01"
main = "src/index.ts"

[[d1_databases]]
binding = "DB"
database_name = "simse-mailer-db"
database_id = "PLACEHOLDER_FILL_AFTER_CREATION"

[[secrets_store.bindings]]
binding = "SECRETS"
store_id = "PLACEHOLDER_REPLACE_WITH_STORE_ID"

[[queues.consumers]]
queue = "simse-auth-comms"

[[queues.consumers]]
queue = "simse-api-comms"

[[queues.consumers]]
queue = "simse-landing-comms"
```

Replace `PLACEHOLDER_REPLACE_WITH_STORE_ID` with the actual store ID from Task 1.

**Note:** The D1 database and queue consumers are added here if not already present from the service extraction plan.

**Step 4: Verify build**

```bash
cd simse-mailer && bun run build
```

Expected: Build succeeds. If `render.ts` or `notifications.ts` are missing, comment out those imports temporarily to verify the core secrets changes build cleanly.

**Step 5: Commit**

```
refactor(simse-mailer): use Cloudflare Secrets Store for all secrets
```

---

### Task 5: Verify simse-landing and simse-cloud need no changes

**Step 1: Confirm no secrets in simse-landing**

```bash
grep -r "secret\|API_KEY\|API_SECRET\|RESEND" simse-landing/wrangler.toml simse-landing/src 2>/dev/null
```

Expected: No matches (only `COMMS_QUEUE` queue binding and `DB` D1 binding).

**Step 2: Confirm no secrets in simse-cloud**

```bash
grep -r "secret\|API_KEY\|API_SECRET" simse-cloud/wrangler.toml 2>/dev/null
```

Expected: No matches (only `APP_URL` var).

**Step 3: Commit (if any minor cleanup needed)**

```
chore: verify no secrets in simse-landing or simse-cloud
```

---

### Task 6: Final verification

**Step 1: Grep for any remaining hardcoded secrets**

```bash
grep -r "RESEND_API_KEY\|API_SECRET\|MAILER_API_SECRET\|AUTH_API_SECRET\|PAYMENTS_API_SECRET" \
  simse-api/src simse-mailer/src simse-auth/src simse-landing/src simse-cloud/app \
  --include="*.ts" --include="*.tsx" 2>/dev/null
```

Expected: Zero matches in source code. Only `wrangler.toml` files should reference these as store secret names (strings in CLI commands or comments, not as `c.env.*`).

**Step 2: Grep for any remaining `c.env.` string secret access**

```bash
grep -rn "c\.env\.\(AUTH_API\|PAYMENTS_API\|MAILER_API\|RESEND\|API_SECRET\)" \
  simse-api/src simse-mailer/src 2>/dev/null
```

Expected: Zero matches.

**Step 3: Commit**

```
chore: verify all secrets routed through Cloudflare Secrets Store
```
