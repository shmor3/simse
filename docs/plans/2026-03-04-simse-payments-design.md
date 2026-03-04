# simse-payments Design

**Date:** 2026-03-04
**Status:** Approved

## Overview

Extract all payment/billing logic from simse-cloud into a standalone Cloudflare Worker microservice (`simse-payments`). The service owns Stripe interactions, subscription state, and credit tracking via its own D1 database.

## Architecture

**Runtime:** Cloudflare Worker (Hono + TypeScript)
**Database:** Cloudflare D1 (own instance, separate from simse-cloud)
**Auth:** Bearer token (`API_SECRET`) on all API endpoints
**Dependencies:** `hono`, `stripe`, `@cloudflare/workers-types`, `wrangler`

### Service Interactions

```
simse-cloud (UI) --REST--> simse-payments --Stripe SDK--> Stripe API
                                |
                                +--> simse-mailer (email notifications)

Stripe --webhook--> simse-payments --update DB + send emails-->
```

## API Surface

| Method | Route | Purpose |
|--------|-------|---------|
| `GET` | `/health` | Health check |
| `POST` | `/customers` | Create or get Stripe customer by team ID |
| `POST` | `/checkout` | Create checkout session, returns redirect URL |
| `POST` | `/portal` | Create billing portal session, returns redirect URL |
| `GET` | `/subscriptions/:teamId` | Get current plan/subscription for a team |
| `GET` | `/credits/:userId` | Get credit balance and recent transactions |
| `POST` | `/credits` | Add/deduct credits |
| `POST` | `/webhooks/stripe` | Stripe webhook receiver (no auth, signature verified) |

### Request/Response Examples

**POST /customers**
```json
// Request
{ "teamId": "team_abc", "email": "user@example.com", "name": "Acme Inc" }
// Response
{ "customerId": "cus_xyz" }
```

**POST /checkout**
```json
// Request
{ "teamId": "team_abc", "priceId": "price_xxx", "appUrl": "https://app.simse.dev" }
// Response
{ "url": "https://checkout.stripe.com/..." }
```

**POST /portal**
```json
// Request
{ "teamId": "team_abc", "appUrl": "https://app.simse.dev" }
// Response
{ "url": "https://billing.stripe.com/..." }
```

**GET /subscriptions/:teamId**
```json
// Response
{ "teamId": "team_abc", "plan": "pro", "status": "active", "stripeSubscriptionId": "sub_xxx" }
```

**GET /credits/:userId**
```json
// Response
{ "balance": 42.50, "history": [{ "id": "cr_1", "amount": -1.20, "description": "Session tokens", "createdAt": "2026-03-04T12:00:00Z" }] }
```

**POST /credits**
```json
// Request
{ "userId": "user_abc", "amount": -1.20, "description": "Session tokens" }
// Response
{ "id": "cr_new", "balance": 41.30 }
```

## Database Schema (D1)

```sql
CREATE TABLE customers (
  team_id TEXT PRIMARY KEY,
  stripe_customer_id TEXT NOT NULL UNIQUE,
  email TEXT NOT NULL,
  name TEXT NOT NULL,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE subscriptions (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL UNIQUE REFERENCES customers(team_id),
  stripe_subscription_id TEXT UNIQUE,
  plan TEXT DEFAULT 'free',
  status TEXT DEFAULT 'active',
  created_at TEXT DEFAULT (datetime('now')),
  updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE credit_ledger (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  amount REAL NOT NULL,
  description TEXT NOT NULL,
  created_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_credit_ledger_user ON credit_ledger(user_id);
```

## Webhook Handling

Stripe sends events to `POST /webhooks/stripe`. The handler:

1. Verifies `Stripe-Signature` header
2. Processes events:
   - `customer.subscription.created/updated` → upsert subscription record, set plan from `lookup_key`
   - `customer.subscription.deleted` → set plan to `free`, clear subscription ID
   - `invoice.payment_succeeded` → call simse-mailer with payment-receipt template
   - `invoice.payment_failed` → call simse-mailer with payment-failed template
3. Returns 200

## Secrets

Set via `wrangler secret put`:
- `STRIPE_SECRET_KEY` — Stripe API key
- `STRIPE_WEBHOOK_SECRET` — Webhook signature secret
- `API_SECRET` — Bearer token for API auth
- `MAILER_API_URL` — simse-mailer endpoint
- `MAILER_API_SECRET` — simse-mailer auth token

## Changes to simse-cloud

### Remove
- `app/lib/stripe.server.ts`
- `app/routes/api.stripe-webhook.tsx`
- `stripe` from package.json
- `STRIPE_SECRET_KEY`, `STRIPE_WEBHOOK_SECRET` from wrangler.toml/secrets
- `stripe_customer_id`, `stripe_subscription_id` columns from teams table
- `credit_ledger` table

### Add
- `app/lib/payments.server.ts` — HTTP client for simse-payments API
- `PAYMENTS_API_URL`, `PAYMENTS_API_SECRET` env vars

### Modify
- `app/env.d.ts` — swap Stripe vars for payments API vars
- `dashboard.billing.tsx` — loader/action use payments client
- `dashboard.billing.credit.tsx` — loader uses payments client
- `dashboard.usage.tsx` — loader uses payments client
- `dashboard.team.plans.tsx` — loader uses payments client
- `wrangler.toml` — remove Stripe secrets, add payments API secrets

### Keep
- All billing/credit/usage/plans UI components (frontend stays in simse-cloud)
- Notification system (simse-cloud owns notifications DB)
- Email templates (simse-cloud keeps templates, simse-payments triggers sends via simse-mailer)

## Project Structure

```
simse-payments/
├── src/
│   ├── index.ts          # Hono app, route definitions, auth middleware
│   ├── routes/
│   │   ├── customers.ts  # POST /customers
│   │   ├── checkout.ts   # POST /checkout
│   │   ├── portal.ts     # POST /portal
│   │   ├── subscriptions.ts # GET /subscriptions/:teamId
│   │   ├── credits.ts    # GET/POST /credits
│   │   └── webhooks.ts   # POST /webhooks/stripe
│   ├── lib/
│   │   ├── stripe.ts     # Stripe SDK wrapper (moved from simse-cloud)
│   │   ├── mailer.ts     # simse-mailer HTTP client
│   │   └── db.ts         # ID generation, DB helpers
│   └── types.ts          # Env interface, shared types
├── migrations/
│   └── 0001_initial.sql  # D1 schema
├── package.json
├── tsconfig.json
├── wrangler.toml
├── biome.json
└── moon.yml
```
