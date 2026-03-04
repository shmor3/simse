# simse-payments Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract all Stripe/payment/credit logic from simse-cloud into a standalone simse-payments Cloudflare Worker with its own D1 database.

**Architecture:** Hono-based Cloudflare Worker with D1 for state, Stripe SDK for payments, calling simse-mailer for email notifications. simse-cloud calls simse-payments REST API for all billing operations.

**Tech Stack:** Hono, Stripe SDK, Cloudflare Workers + D1, TypeScript, Biome

---

### Task 1: Scaffold simse-payments project

**Files:**
- Create: `simse-payments/package.json`
- Create: `simse-payments/tsconfig.json`
- Create: `simse-payments/biome.json`
- Create: `simse-payments/wrangler.toml`
- Create: `simse-payments/moon.yml`
- Create: `simse-payments/.gitignore`

**Step 1: Create package.json**

```json
{
	"name": "simse-payments",
	"private": true,
	"type": "module",
	"scripts": {
		"dev": "wrangler dev",
		"build": "wrangler deploy --dry-run --outdir dist",
		"deploy": "wrangler deploy",
		"lint": "biome check .",
		"lint:fix": "biome check --write .",
		"db:migrate": "wrangler d1 migrations apply simse-payments-db --local",
		"db:migrate:prod": "wrangler d1 migrations apply simse-payments-db --remote"
	},
	"dependencies": {
		"hono": "^4.7.0",
		"stripe": "^18.1.0"
	},
	"devDependencies": {
		"@biomejs/biome": "^2.3.12",
		"@cloudflare/workers-types": "^4.20260305.0",
		"typescript": "^5.7.0",
		"wrangler": "^4.0.0"
	}
}
```

**Step 2: Create tsconfig.json** (copy from simse-mailer)

```json
{
	"compilerOptions": {
		"target": "ESNext",
		"module": "ESNext",
		"moduleResolution": "bundler",
		"jsx": "react-jsx",
		"strict": true,
		"noEmit": true,
		"skipLibCheck": true,
		"esModuleInterop": true,
		"forceConsistentCasingInFileNames": true,
		"resolveJsonModule": true,
		"isolatedModules": true,
		"types": ["@cloudflare/workers-types"]
	},
	"include": ["src/**/*.ts", "src/**/*.tsx"]
}
```

**Step 3: Create biome.json** (copy from simse-mailer)

```json
{
	"$schema": "https://biomejs.dev/schemas/2.4.5/schema.json",
	"vcs": {
		"enabled": true,
		"clientKind": "git",
		"useIgnoreFile": true
	},
	"files": {
		"includes": ["**", "!!**/dist"]
	},
	"formatter": {
		"enabled": true,
		"indentStyle": "tab"
	},
	"linter": {
		"enabled": true,
		"rules": {
			"recommended": true
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

**Step 4: Create wrangler.toml**

```toml
name = "simse-payments"
compatibility_date = "2025-04-01"
main = "src/index.ts"

[[d1_databases]]
binding = "DB"
database_name = "simse-payments-db"
database_id = "placeholder-create-via-wrangler"

# Secrets (set via `wrangler secret put`):
# STRIPE_SECRET_KEY
# STRIPE_WEBHOOK_SECRET
# API_SECRET
# MAILER_API_URL
# MAILER_API_SECRET
```

**Step 5: Create moon.yml**

```yaml
language: "typescript"
tags: ["app"]

tasks:
  test:
    command: "bun run lint"
    options:
      mergeArgs: "replace"
  start:
    command: "bun run dev"
    options:
      mergeArgs: "replace"
```

**Step 6: Create .gitignore**

```
node_modules/
dist/
.wrangler/
```

**Step 7: Install dependencies**

Run: `cd simse-payments && bun install`

**Step 8: Create D1 database**

Run: `cd simse-payments && wrangler d1 create simse-payments-db`
Then update `wrangler.toml` with the real database_id from the output.

**Step 9: Commit**

```bash
git add simse-payments/
git commit -m "feat(simse-payments): scaffold cloudflare worker project"
```

---

### Task 2: D1 migration — create schema

**Files:**
- Create: `simse-payments/migrations/0001_initial.sql`

**Step 1: Create migration file**

```sql
-- Customers: maps team IDs to Stripe customers
CREATE TABLE customers (
  team_id TEXT PRIMARY KEY,
  stripe_customer_id TEXT NOT NULL UNIQUE,
  email TEXT NOT NULL,
  name TEXT NOT NULL,
  created_at TEXT DEFAULT (datetime('now'))
);

-- Subscriptions: tracks plan state per team
CREATE TABLE subscriptions (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL UNIQUE REFERENCES customers(team_id),
  stripe_subscription_id TEXT UNIQUE,
  plan TEXT DEFAULT 'free',
  status TEXT DEFAULT 'active',
  created_at TEXT DEFAULT (datetime('now')),
  updated_at TEXT DEFAULT (datetime('now'))
);

-- Credit ledger: tracks usage credits per user
CREATE TABLE credit_ledger (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  amount REAL NOT NULL,
  description TEXT NOT NULL,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_credit_ledger_user ON credit_ledger(user_id);
CREATE INDEX idx_customers_stripe ON customers(stripe_customer_id);
```

**Step 2: Run migration locally**

Run: `cd simse-payments && wrangler d1 migrations apply simse-payments-db --local`
Expected: Tables created successfully.

**Step 3: Commit**

```bash
git add simse-payments/migrations/
git commit -m "feat(simse-payments): add D1 schema migration"
```

---

### Task 3: Types and helpers

**Files:**
- Create: `simse-payments/src/types.ts`
- Create: `simse-payments/src/lib/db.ts`
- Create: `simse-payments/src/lib/stripe.ts`
- Create: `simse-payments/src/lib/mailer.ts`

**Step 1: Create types.ts**

```typescript
export interface Env {
	DB: D1Database;
	STRIPE_SECRET_KEY: string;
	STRIPE_WEBHOOK_SECRET: string;
	API_SECRET: string;
	MAILER_API_URL: string;
	MAILER_API_SECRET: string;
}
```

**Step 2: Create db.ts**

```typescript
export function generateId(): string {
	return crypto.randomUUID();
}
```

**Step 3: Create stripe.ts** (moved from simse-cloud, adapted)

```typescript
import Stripe from 'stripe';

export function createStripe(secretKey: string): Stripe {
	return new Stripe(secretKey);
}

export async function createCheckoutSession(
	stripe: Stripe,
	customerId: string,
	priceId: string,
	appUrl: string,
): Promise<string> {
	const session = await stripe.checkout.sessions.create({
		customer: customerId,
		mode: 'subscription',
		line_items: [{ price: priceId, quantity: 1 }],
		success_url: `${appUrl}/dashboard/billing?success=true`,
		cancel_url: `${appUrl}/dashboard/billing?canceled=true`,
	});
	return session.url!;
}

export async function createBillingPortalSession(
	stripe: Stripe,
	customerId: string,
	appUrl: string,
): Promise<string> {
	const session = await stripe.billingPortal.sessions.create({
		customer: customerId,
		return_url: `${appUrl}/dashboard/billing`,
	});
	return session.url;
}

export async function verifyWebhookSignature(
	stripe: Stripe,
	body: string,
	signature: string,
	secret: string,
): Promise<Stripe.Event> {
	return stripe.webhooks.constructEventAsync(body, signature, secret);
}
```

**Step 4: Create mailer.ts**

```typescript
export async function sendEmail(
	mailerUrl: string,
	mailerSecret: string,
	to: string,
	subject: string,
	html: string,
): Promise<void> {
	const res = await fetch(`${mailerUrl}/send`, {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${mailerSecret}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({ to, subject, html }),
	});

	if (!res.ok) {
		const body = await res.text();
		console.error(`Mailer error (${res.status}): ${body}`);
	}
}
```

**Step 5: Commit**

```bash
git add simse-payments/src/
git commit -m "feat(simse-payments): add types, db helpers, stripe and mailer libs"
```

---

### Task 4: Auth middleware and health route

**Files:**
- Create: `simse-payments/src/middleware/auth.ts`
- Create: `simse-payments/src/index.ts`

**Step 1: Create auth middleware**

```typescript
import type { Context, Next } from 'hono';
import type { Env } from '../types';

export async function authMiddleware(
	c: Context<{ Bindings: Env }>,
	next: Next,
) {
	const authHeader = c.req.header('Authorization');
	if (authHeader !== `Bearer ${c.env.API_SECRET}`) {
		return c.json({ error: 'Unauthorized' }, 401);
	}
	await next();
}
```

**Step 2: Create index.ts with health route**

```typescript
import { Hono } from 'hono';
import { authMiddleware } from './middleware/auth';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));

// All routes below require auth (except webhooks, added later)
app.use('/customers/*', authMiddleware);
app.use('/checkout', authMiddleware);
app.use('/portal', authMiddleware);
app.use('/subscriptions/*', authMiddleware);
app.use('/credits/*', authMiddleware);
app.use('/credits', authMiddleware);

export default app;
```

**Step 3: Verify lint passes**

Run: `cd simse-payments && bun run lint`
Expected: No errors.

**Step 4: Commit**

```bash
git add simse-payments/src/
git commit -m "feat(simse-payments): add auth middleware and health route"
```

---

### Task 5: Customer routes

**Files:**
- Create: `simse-payments/src/routes/customers.ts`
- Modify: `simse-payments/src/index.ts`

**Step 1: Create customers.ts**

```typescript
import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { createStripe } from '../lib/stripe';
import type { Env } from '../types';

const customers = new Hono<{ Bindings: Env }>();

// POST /customers — create or get Stripe customer
customers.post('/', async (c) => {
	const body = await c.req.json<{
		teamId: string;
		email: string;
		name: string;
	}>();

	if (!body.teamId || !body.email || !body.name) {
		return c.json({ error: 'Missing required fields: teamId, email, name' }, 400);
	}

	const db = c.env.DB;

	// Check if customer already exists
	const existing = await db
		.prepare('SELECT stripe_customer_id FROM customers WHERE team_id = ?')
		.bind(body.teamId)
		.first<{ stripe_customer_id: string }>();

	if (existing) {
		return c.json({ customerId: existing.stripe_customer_id });
	}

	// Create in Stripe
	const stripe = createStripe(c.env.STRIPE_SECRET_KEY);
	const customer = await stripe.customers.create({
		email: body.email,
		name: body.name,
		metadata: { teamId: body.teamId },
	});

	// Store locally
	await db
		.prepare(
			'INSERT INTO customers (team_id, stripe_customer_id, email, name) VALUES (?, ?, ?, ?)',
		)
		.bind(body.teamId, customer.id, body.email, body.name)
		.run();

	// Create default free subscription record
	await db
		.prepare(
			"INSERT INTO subscriptions (id, team_id, plan, status) VALUES (?, ?, 'free', 'active')",
		)
		.bind(generateId(), body.teamId)
		.run();

	return c.json({ customerId: customer.id });
});

export default customers;
```

**Step 2: Mount in index.ts — add import and route**

Add to `simse-payments/src/index.ts`:
```typescript
import customers from './routes/customers';

// After auth middleware setup:
app.route('/customers', customers);
```

**Step 3: Commit**

```bash
git add simse-payments/src/
git commit -m "feat(simse-payments): add customer create/get route"
```

---

### Task 6: Checkout and portal routes

**Files:**
- Create: `simse-payments/src/routes/checkout.ts`
- Create: `simse-payments/src/routes/portal.ts`
- Modify: `simse-payments/src/index.ts`

**Step 1: Create checkout.ts**

```typescript
import { Hono } from 'hono';
import { createCheckoutSession, createStripe } from '../lib/stripe';
import type { Env } from '../types';

const checkout = new Hono<{ Bindings: Env }>();

checkout.post('/', async (c) => {
	const body = await c.req.json<{
		teamId: string;
		priceId: string;
		appUrl: string;
	}>();

	if (!body.teamId || !body.priceId || !body.appUrl) {
		return c.json(
			{ error: 'Missing required fields: teamId, priceId, appUrl' },
			400,
		);
	}

	const db = c.env.DB;
	const customer = await db
		.prepare('SELECT stripe_customer_id FROM customers WHERE team_id = ?')
		.bind(body.teamId)
		.first<{ stripe_customer_id: string }>();

	if (!customer) {
		return c.json({ error: 'Customer not found. Create customer first.' }, 404);
	}

	const stripe = createStripe(c.env.STRIPE_SECRET_KEY);
	const url = await createCheckoutSession(
		stripe,
		customer.stripe_customer_id,
		body.priceId,
		body.appUrl,
	);

	return c.json({ url });
});

export default checkout;
```

**Step 2: Create portal.ts**

```typescript
import { Hono } from 'hono';
import { createBillingPortalSession, createStripe } from '../lib/stripe';
import type { Env } from '../types';

const portal = new Hono<{ Bindings: Env }>();

portal.post('/', async (c) => {
	const body = await c.req.json<{
		teamId: string;
		appUrl: string;
	}>();

	if (!body.teamId || !body.appUrl) {
		return c.json({ error: 'Missing required fields: teamId, appUrl' }, 400);
	}

	const db = c.env.DB;
	const customer = await db
		.prepare('SELECT stripe_customer_id FROM customers WHERE team_id = ?')
		.bind(body.teamId)
		.first<{ stripe_customer_id: string }>();

	if (!customer) {
		return c.json({ error: 'Customer not found' }, 404);
	}

	const stripe = createStripe(c.env.STRIPE_SECRET_KEY);
	const url = await createBillingPortalSession(
		stripe,
		customer.stripe_customer_id,
		body.appUrl,
	);

	return c.json({ url });
});

export default portal;
```

**Step 3: Mount in index.ts**

Add imports and routes:
```typescript
import checkout from './routes/checkout';
import portal from './routes/portal';

app.route('/checkout', checkout);
app.route('/portal', portal);
```

**Step 4: Commit**

```bash
git add simse-payments/src/
git commit -m "feat(simse-payments): add checkout and billing portal routes"
```

---

### Task 7: Subscription route

**Files:**
- Create: `simse-payments/src/routes/subscriptions.ts`
- Modify: `simse-payments/src/index.ts`

**Step 1: Create subscriptions.ts**

```typescript
import { Hono } from 'hono';
import type { Env } from '../types';

const subscriptions = new Hono<{ Bindings: Env }>();

// GET /subscriptions/:teamId — get current plan
subscriptions.get('/:teamId', async (c) => {
	const teamId = c.req.param('teamId');
	const db = c.env.DB;

	const sub = await db
		.prepare(
			'SELECT team_id, stripe_subscription_id, plan, status FROM subscriptions WHERE team_id = ?',
		)
		.bind(teamId)
		.first<{
			team_id: string;
			stripe_subscription_id: string | null;
			plan: string;
			status: string;
		}>();

	if (!sub) {
		return c.json({
			teamId,
			plan: 'free',
			status: 'active',
			stripeSubscriptionId: null,
		});
	}

	return c.json({
		teamId: sub.team_id,
		plan: sub.plan,
		status: sub.status,
		stripeSubscriptionId: sub.stripe_subscription_id,
	});
});

export default subscriptions;
```

**Step 2: Mount in index.ts**

```typescript
import subscriptions from './routes/subscriptions';

app.route('/subscriptions', subscriptions);
```

**Step 3: Commit**

```bash
git add simse-payments/src/
git commit -m "feat(simse-payments): add subscription query route"
```

---

### Task 8: Credit routes

**Files:**
- Create: `simse-payments/src/routes/credits.ts`
- Modify: `simse-payments/src/index.ts`

**Step 1: Create credits.ts**

```typescript
import { Hono } from 'hono';
import { generateId } from '../lib/db';
import type { Env } from '../types';

const credits = new Hono<{ Bindings: Env }>();

// GET /credits/:userId — balance + recent history
credits.get('/:userId', async (c) => {
	const userId = c.req.param('userId');
	const db = c.env.DB;

	const balance = await db
		.prepare(
			'SELECT COALESCE(SUM(amount), 0) as total FROM credit_ledger WHERE user_id = ?',
		)
		.bind(userId)
		.first<{ total: number }>();

	const history = await db
		.prepare(
			'SELECT id, amount, description, created_at FROM credit_ledger WHERE user_id = ? ORDER BY created_at DESC LIMIT 50',
		)
		.bind(userId)
		.all<{
			id: string;
			amount: number;
			description: string;
			created_at: string;
		}>();

	return c.json({
		balance: balance?.total ?? 0,
		history: history.results,
	});
});

// POST /credits — add/deduct credit
credits.post('/', async (c) => {
	const body = await c.req.json<{
		userId: string;
		amount: number;
		description: string;
	}>();

	if (!body.userId || body.amount === undefined || !body.description) {
		return c.json(
			{ error: 'Missing required fields: userId, amount, description' },
			400,
		);
	}

	const db = c.env.DB;
	const id = generateId();

	await db
		.prepare(
			'INSERT INTO credit_ledger (id, user_id, amount, description) VALUES (?, ?, ?, ?)',
		)
		.bind(id, body.userId, body.amount, body.description)
		.run();

	const balance = await db
		.prepare(
			'SELECT COALESCE(SUM(amount), 0) as total FROM credit_ledger WHERE user_id = ?',
		)
		.bind(body.userId)
		.first<{ total: number }>();

	return c.json({ id, balance: balance?.total ?? 0 });
});

// GET /credits/:userId/usage — last 7 days usage (for dashboard.usage)
credits.get('/:userId/usage', async (c) => {
	const userId = c.req.param('userId');
	const db = c.env.DB;

	const balance = await db
		.prepare(
			'SELECT COALESCE(SUM(amount), 0) as total FROM credit_ledger WHERE user_id = ?',
		)
		.bind(userId)
		.first<{ total: number }>();

	const recentUsage = await db
		.prepare(
			"SELECT date(created_at) as day, SUM(ABS(amount)) as tokens FROM credit_ledger WHERE user_id = ? AND amount < 0 AND created_at > datetime('now', '-7 days') GROUP BY date(created_at) ORDER BY day",
		)
		.bind(userId)
		.all<{ day: string; tokens: number }>();

	return c.json({
		balance: balance?.total ?? 0,
		recentUsage: recentUsage.results,
	});
});

export default credits;
```

**Step 2: Mount in index.ts**

```typescript
import credits from './routes/credits';

app.route('/credits', credits);
```

**Step 3: Commit**

```bash
git add simse-payments/src/
git commit -m "feat(simse-payments): add credit balance, history, and usage routes"
```

---

### Task 9: Stripe webhook handler

**Files:**
- Create: `simse-payments/src/routes/webhooks.ts`
- Modify: `simse-payments/src/index.ts`

**Step 1: Create webhooks.ts**

```typescript
import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { sendEmail } from '../lib/mailer';
import { createStripe, verifyWebhookSignature } from '../lib/stripe';
import type { Env } from '../types';

const webhooks = new Hono<{ Bindings: Env }>();

webhooks.post('/stripe', async (c) => {
	const stripe = createStripe(c.env.STRIPE_SECRET_KEY);
	const body = await c.req.text();
	const signature = c.req.header('Stripe-Signature');

	if (!signature) {
		return c.json({ error: 'Missing signature' }, 400);
	}

	let event: Awaited<ReturnType<typeof verifyWebhookSignature>>;
	try {
		event = await verifyWebhookSignature(
			stripe,
			body,
			signature,
			c.env.STRIPE_WEBHOOK_SECRET,
		);
	} catch {
		return c.json({ error: 'Invalid signature' }, 400);
	}

	const db = c.env.DB;

	switch (event.type) {
		case 'customer.subscription.created':
		case 'customer.subscription.updated': {
			const sub = event.data.object;
			const customerId =
				typeof sub.customer === 'string' ? sub.customer : sub.customer.id;

			const plan =
				sub.status === 'active'
					? (sub.items.data[0]?.price?.lookup_key ?? 'pro')
					: 'free';

			const status = sub.status === 'active' ? 'active' : 'inactive';

			// Find team by stripe customer ID
			const customer = await db
				.prepare(
					'SELECT team_id FROM customers WHERE stripe_customer_id = ?',
				)
				.bind(customerId)
				.first<{ team_id: string }>();

			if (customer) {
				// Upsert subscription
				const existing = await db
					.prepare('SELECT id FROM subscriptions WHERE team_id = ?')
					.bind(customer.team_id)
					.first<{ id: string }>();

				if (existing) {
					await db
						.prepare(
							"UPDATE subscriptions SET plan = ?, status = ?, stripe_subscription_id = ?, updated_at = datetime('now') WHERE team_id = ?",
						)
						.bind(plan, status, sub.id, customer.team_id)
						.run();
				} else {
					await db
						.prepare(
							'INSERT INTO subscriptions (id, team_id, stripe_subscription_id, plan, status) VALUES (?, ?, ?, ?, ?)',
						)
						.bind(generateId(), customer.team_id, sub.id, plan, status)
						.run();
				}
			}
			break;
		}

		case 'customer.subscription.deleted': {
			const sub = event.data.object;
			const customerId =
				typeof sub.customer === 'string' ? sub.customer : sub.customer.id;

			const customer = await db
				.prepare(
					'SELECT team_id FROM customers WHERE stripe_customer_id = ?',
				)
				.bind(customerId)
				.first<{ team_id: string }>();

			if (customer) {
				await db
					.prepare(
						"UPDATE subscriptions SET plan = 'free', status = 'canceled', stripe_subscription_id = NULL, updated_at = datetime('now') WHERE team_id = ?",
					)
					.bind(customer.team_id)
					.run();
			}
			break;
		}

		case 'invoice.payment_succeeded': {
			const invoice = event.data.object;
			const customerId =
				typeof invoice.customer === 'string'
					? invoice.customer
					: invoice.customer?.id;

			if (customerId) {
				const customer = await db
					.prepare(
						'SELECT email, name FROM customers WHERE stripe_customer_id = ?',
					)
					.bind(customerId)
					.first<{ email: string; name: string }>();

				if (customer) {
					const amount = `$${((invoice.amount_paid ?? 0) / 100).toFixed(2)}`;
					await sendEmail(
						c.env.MAILER_API_URL,
						c.env.MAILER_API_SECRET,
						customer.email,
						`Receipt for your simse payment — ${amount}`,
						`<p>Payment of ${amount} received. Thank you!</p>`,
					);
				}
			}
			break;
		}

		case 'invoice.payment_failed': {
			const invoice = event.data.object;
			const customerId =
				typeof invoice.customer === 'string'
					? invoice.customer
					: invoice.customer?.id;

			if (customerId) {
				const customer = await db
					.prepare(
						'SELECT email, name FROM customers WHERE stripe_customer_id = ?',
					)
					.bind(customerId)
					.first<{ email: string; name: string }>();

				if (customer) {
					const amount = `$${((invoice.amount_due ?? 0) / 100).toFixed(2)}`;
					await sendEmail(
						c.env.MAILER_API_URL,
						c.env.MAILER_API_SECRET,
						customer.email,
						`Your simse payment of ${amount} didn't go through`,
						`<p>We couldn't process your payment of ${amount}. Please update your payment method.</p>`,
					);
				}
			}
			break;
		}
	}

	return c.json({ received: true });
});

export default webhooks;
```

**Step 2: Mount in index.ts (NO auth middleware for webhooks)**

```typescript
import webhooks from './routes/webhooks';

// Mount BEFORE the auth middleware lines, or just add it separately:
app.route('/webhooks', webhooks);
```

**Step 3: Commit**

```bash
git add simse-payments/src/
git commit -m "feat(simse-payments): add Stripe webhook handler"
```

---

### Task 10: Final index.ts assembly and lint check

**Files:**
- Modify: `simse-payments/src/index.ts` (ensure all routes are properly assembled)

**Step 1: Verify final index.ts looks correct**

The complete `simse-payments/src/index.ts` should be:

```typescript
import { Hono } from 'hono';
import { authMiddleware } from './middleware/auth';
import checkout from './routes/checkout';
import credits from './routes/credits';
import customers from './routes/customers';
import portal from './routes/portal';
import subscriptions from './routes/subscriptions';
import webhooks from './routes/webhooks';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));

// Webhooks — no auth (Stripe signature verification instead)
app.route('/webhooks', webhooks);

// Authenticated routes
app.use('/customers/*', authMiddleware);
app.use('/checkout', authMiddleware);
app.use('/portal', authMiddleware);
app.use('/subscriptions/*', authMiddleware);
app.use('/credits/*', authMiddleware);
app.use('/credits', authMiddleware);

app.route('/customers', customers);
app.route('/checkout', checkout);
app.route('/portal', portal);
app.route('/subscriptions', subscriptions);
app.route('/credits', credits);

export default app;
```

**Step 2: Run lint**

Run: `cd simse-payments && bun run lint`
Expected: No errors.

**Step 3: Run build**

Run: `cd simse-payments && bun run build`
Expected: Build succeeds.

**Step 4: Commit**

```bash
git add simse-payments/src/
git commit -m "feat(simse-payments): assemble all routes in index"
```

---

### Task 11: Create simse-cloud payments client

**Files:**
- Create: `simse-cloud/app/lib/payments.server.ts`

**Step 1: Create payments.server.ts**

```typescript
interface PaymentsClientOptions {
	apiUrl: string;
	apiSecret: string;
}

async function request<T>(
	opts: PaymentsClientOptions,
	method: string,
	path: string,
	body?: unknown,
): Promise<T> {
	const res = await fetch(`${opts.apiUrl}${path}`, {
		method,
		headers: {
			Authorization: `Bearer ${opts.apiSecret}`,
			'Content-Type': 'application/json',
		},
		body: body ? JSON.stringify(body) : undefined,
	});

	if (!res.ok) {
		const text = await res.text();
		throw new Error(`Payments API error (${res.status}): ${text}`);
	}

	return res.json() as Promise<T>;
}

export function createPaymentsClient(opts: PaymentsClientOptions) {
	return {
		getOrCreateCustomer(teamId: string, email: string, name: string) {
			return request<{ customerId: string }>(opts, 'POST', '/customers', {
				teamId,
				email,
				name,
			});
		},

		createCheckoutSession(teamId: string, priceId: string, appUrl: string) {
			return request<{ url: string }>(opts, 'POST', '/checkout', {
				teamId,
				priceId,
				appUrl,
			});
		},

		createPortalSession(teamId: string, appUrl: string) {
			return request<{ url: string }>(opts, 'POST', '/portal', {
				teamId,
				appUrl,
			});
		},

		getSubscription(teamId: string) {
			return request<{
				teamId: string;
				plan: string;
				status: string;
				stripeSubscriptionId: string | null;
			}>(opts, 'GET', `/subscriptions/${teamId}`);
		},

		getCredits(userId: string) {
			return request<{
				balance: number;
				history: Array<{
					id: string;
					amount: number;
					description: string;
					created_at: string;
				}>;
			}>(opts, 'GET', `/credits/${userId}`);
		},

		getUsage(userId: string) {
			return request<{
				balance: number;
				recentUsage: Array<{ day: string; tokens: number }>;
			}>(opts, 'GET', `/credits/${userId}/usage`);
		},

		addCredit(userId: string, amount: number, description: string) {
			return request<{ id: string; balance: number }>(
				opts,
				'POST',
				'/credits',
				{ userId, amount, description },
			);
		},
	};
}
```

**Step 2: Commit**

```bash
git add simse-cloud/app/lib/payments.server.ts
git commit -m "feat(simse-cloud): add payments API client"
```

---

### Task 12: Update simse-cloud env and config

**Files:**
- Modify: `simse-cloud/app/env.d.ts`
- Modify: `simse-cloud/wrangler.toml`

**Step 1: Update env.d.ts — remove Stripe vars, add payments API vars**

Replace the full file:
```typescript
interface Env {
	DB: D1Database;
	SESSIONS: KVNamespace;
	PAYMENTS_API_URL: string;
	PAYMENTS_API_SECRET: string;
	EMAIL_API_URL: string;
	EMAIL_API_SECRET: string;
	SESSION_SECRET: string;
	APP_URL: string;
}
```

**Step 2: Update wrangler.toml — remove Stripe secrets comments, add payments secrets**

Replace the secrets comment block:
```toml
# Secrets (set via `wrangler secret put`):
# PAYMENTS_API_URL
# PAYMENTS_API_SECRET
# EMAIL_API_URL
# EMAIL_API_SECRET
# SESSION_SECRET
```

**Step 3: Commit**

```bash
git add simse-cloud/app/env.d.ts simse-cloud/wrangler.toml
git commit -m "refactor(simse-cloud): swap Stripe env vars for payments API"
```

---

### Task 13: Rewrite dashboard.billing.tsx to use payments client

**Files:**
- Modify: `simse-cloud/app/routes/dashboard.billing.tsx`

**Step 1: Rewrite loader and action**

Replace the entire file — keep the UI component unchanged but rewrite loader/action to use payments client:

```typescript
import { Form, redirect, useNavigation } from 'react-router';
import PageHeader from '~/components/layout/PageHeader';
import Badge from '~/components/ui/Badge';
import Button from '~/components/ui/Button';
import Card from '~/components/ui/Card';
import { createPaymentsClient } from '~/lib/payments.server';
import { getSession } from '~/lib/session.server';
import type { Route } from './+types/dashboard.billing';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) throw redirect('/auth/login');

	const env = context.cloudflare.env;
	const payments = createPaymentsClient({
		apiUrl: env.PAYMENTS_API_URL,
		apiSecret: env.PAYMENTS_API_SECRET,
	});

	// Get user's team
	const db = env.DB;
	const team = await db
		.prepare(
			"SELECT t.id, t.name FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role = 'owner' LIMIT 1",
		)
		.bind(session.userId)
		.first<{ id: string; name: string }>();

	const [subscription, credits] = await Promise.all([
		team ? payments.getSubscription(team.id) : null,
		payments.getCredits(session.userId),
	]);

	return {
		plan: subscription?.plan ?? 'free',
		teamName: team?.name ?? '',
		hasPaymentMethod: !!subscription?.stripeSubscriptionId,
		creditBalance: credits.balance,
	};
}

export async function action({ request, context }: Route.ActionArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) throw redirect('/auth/login');

	const formData = await request.formData();
	const intent = formData.get('intent');
	const env = context.cloudflare.env;
	const db = env.DB;

	const payments = createPaymentsClient({
		apiUrl: env.PAYMENTS_API_URL,
		apiSecret: env.PAYMENTS_API_SECRET,
	});

	const user = await db
		.prepare('SELECT email, name FROM users WHERE id = ?')
		.bind(session.userId)
		.first<{ email: string; name: string }>();

	const team = await db
		.prepare(
			"SELECT t.id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role = 'owner' LIMIT 1",
		)
		.bind(session.userId)
		.first<{ id: string }>();

	if (!user || !team) throw redirect('/dashboard');

	// Ensure customer exists
	await payments.getOrCreateCustomer(team.id, user.email, user.name);

	if (intent === 'manage') {
		const { url } = await payments.createPortalSession(team.id, env.APP_URL);
		throw redirect(url);
	}

	return null;
}

// ... (keep the entire plans array and Billing component unchanged)
```

Note: The `plans` array and `Billing` default export component remain exactly the same.

**Step 2: Commit**

```bash
git add simse-cloud/app/routes/dashboard.billing.tsx
git commit -m "refactor(simse-cloud): billing page uses payments API client"
```

---

### Task 14: Rewrite dashboard.billing.credit.tsx

**Files:**
- Modify: `simse-cloud/app/routes/dashboard.billing.credit.tsx`

**Step 1: Rewrite loader to use payments client**

Replace the loader function only:

```typescript
import { createPaymentsClient } from '~/lib/payments.server';
import { getSession } from '~/lib/session.server';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) return { balance: 0, history: [] };

	const env = context.cloudflare.env;
	const payments = createPaymentsClient({
		apiUrl: env.PAYMENTS_API_URL,
		apiSecret: env.PAYMENTS_API_SECRET,
	});

	const data = await payments.getCredits(session.userId);

	return {
		balance: data.balance,
		history: data.history,
	};
}
```

Remove the direct DB import for credit_ledger. Keep the component unchanged.

**Step 2: Commit**

```bash
git add simse-cloud/app/routes/dashboard.billing.credit.tsx
git commit -m "refactor(simse-cloud): credit page uses payments API"
```

---

### Task 15: Rewrite dashboard.usage.tsx

**Files:**
- Modify: `simse-cloud/app/routes/dashboard.usage.tsx`

**Step 1: Rewrite loader to use payments client**

Replace the loader:

```typescript
import { createPaymentsClient } from '~/lib/payments.server';
import { getSession } from '~/lib/session.server';

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
```

Keep the component unchanged.

**Step 2: Commit**

```bash
git add simse-cloud/app/routes/dashboard.usage.tsx
git commit -m "refactor(simse-cloud): usage page uses payments API"
```

---

### Task 16: Rewrite dashboard.team.plans.tsx

**Files:**
- Modify: `simse-cloud/app/routes/dashboard.team.plans.tsx`

**Step 1: Rewrite loader to use payments client**

```typescript
import { createPaymentsClient } from '~/lib/payments.server';
import { getSession } from '~/lib/session.server';

export async function loader({ request, context }: Route.LoaderArgs) {
	const session = await getSession(request, context.cloudflare.env);
	if (!session) return { currentPlan: 'free' };

	const env = context.cloudflare.env;
	const db = env.DB;

	const team = await db
		.prepare(
			'SELECT t.id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1',
		)
		.bind(session.userId)
		.first<{ id: string }>();

	if (!team) return { currentPlan: 'free' };

	const payments = createPaymentsClient({
		apiUrl: env.PAYMENTS_API_URL,
		apiSecret: env.PAYMENTS_API_SECRET,
	});

	const sub = await payments.getSubscription(team.id);
	return { currentPlan: sub.plan };
}
```

Keep the component unchanged.

**Step 2: Commit**

```bash
git add simse-cloud/app/routes/dashboard.team.plans.tsx
git commit -m "refactor(simse-cloud): plans page uses payments API"
```

---

### Task 17: Update dashboard.account.tsx — remove credit_ledger delete

**Files:**
- Modify: `simse-cloud/app/routes/dashboard.account.tsx`

**Step 1: Remove the credit_ledger DELETE from the delete-account cascade**

In the `delete-account` intent handler, remove this block:
```typescript
		await db
			.prepare('DELETE FROM credit_ledger WHERE user_id = ?')
			.bind(session.userId)
			.run();
```

Credit ledger is now in simse-payments DB. The orphaned records there are harmless.

**Step 2: Commit**

```bash
git add simse-cloud/app/routes/dashboard.account.tsx
git commit -m "refactor(simse-cloud): remove credit_ledger from account deletion"
```

---

### Task 18: Remove Stripe files and dependencies from simse-cloud

**Files:**
- Delete: `simse-cloud/app/lib/stripe.server.ts`
- Delete: `simse-cloud/app/routes/api.stripe-webhook.tsx`
- Modify: `simse-cloud/app/routes.ts` (remove stripe-webhook route)
- Modify: `simse-cloud/package.json` (remove `stripe` dependency)

**Step 1: Delete stripe.server.ts**

Run: `rm simse-cloud/app/lib/stripe.server.ts`

**Step 2: Delete api.stripe-webhook.tsx**

Run: `rm simse-cloud/app/routes/api.stripe-webhook.tsx`

**Step 3: Update routes.ts — remove stripe-webhook route**

Remove the api prefix block:
```typescript
	...prefix('api', [
		route('stripe-webhook', './routes/api.stripe-webhook.tsx'),
	]),
```

**Step 4: Remove stripe from package.json**

Run: `cd simse-cloud && bun remove stripe`

**Step 5: Verify lint passes**

Run: `cd simse-cloud && bun run lint`

**Step 6: Commit**

```bash
git add -A simse-cloud/
git commit -m "refactor(simse-cloud): remove Stripe SDK and webhook handler"
```

---

### Task 19: Create simse-cloud D1 migration to drop payment columns

**Files:**
- Create: `simse-cloud/migrations/0002_remove_payment_columns.sql`

**Step 1: Create migration**

Note: SQLite doesn't support DROP COLUMN on older versions, but D1 supports it. Create the migration:

```sql
-- Remove Stripe-related columns from teams (now in simse-payments)
ALTER TABLE teams DROP COLUMN stripe_customer_id;
ALTER TABLE teams DROP COLUMN stripe_subscription_id;

-- Drop credit_ledger (now in simse-payments)
DROP TABLE IF EXISTS credit_ledger;
```

**Step 2: Commit**

```bash
git add simse-cloud/migrations/
git commit -m "refactor(simse-cloud): migration to drop payment columns and credit_ledger"
```

---

### Task 20: Deploy and verify

**Step 1: Deploy simse-payments**

Run: `cd simse-payments && bun run deploy`

**Step 2: Set secrets for simse-payments**

Run:
```bash
cd simse-payments
wrangler secret put STRIPE_SECRET_KEY
wrangler secret put STRIPE_WEBHOOK_SECRET
wrangler secret put API_SECRET
wrangler secret put MAILER_API_URL
wrangler secret put MAILER_API_SECRET
```

**Step 3: Run D1 migration for simse-payments**

Run: `cd simse-payments && bun run db:migrate:prod`

**Step 4: Set secrets for simse-cloud**

Run:
```bash
cd simse-cloud
wrangler secret put PAYMENTS_API_URL
wrangler secret put PAYMENTS_API_SECRET
```

**Step 5: Run D1 migration for simse-cloud**

Run: `cd simse-cloud && bun run db:migrate:prod`

**Step 6: Verify health endpoint**

Run: `curl https://simse-payments.hybridinnovations.workers.dev/health`
Expected: `{"ok":true}`

**Step 7: Update Stripe webhook URL in Stripe Dashboard**

Point Stripe webhook to: `https://simse-payments.hybridinnovations.workers.dev/webhooks/stripe`

**Step 8: Final commit**

```bash
git commit -m "feat: simse-payments deployed, simse-cloud payment code removed"
```
