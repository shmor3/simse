# Cloudflare Secrets Store Design

**Date:** 2026-03-05
**Status:** Approved

## Overview

Replace per-worker `wrangler secret put` secrets and `[vars]` env values with a single shared Cloudflare Secrets Store (`simse-secrets`). Workers access secrets via an async namespace binding using a Hono middleware that fetches all needed secrets at request start.

## Architecture

**One shared store:** `simse-secrets` — created once via Cloudflare dashboard or Wrangler CLI. All workers that need secrets bind to it.

**Which services need the SECRETS binding:**

| Service | Secrets needed |
|---------|---------------|
| `simse-api` | `AUTH_API_URL`, `AUTH_API_SECRET`, `PAYMENTS_API_URL`, `PAYMENTS_API_SECRET`, `MAILER_API_URL` |
| `simse-mailer` | `RESEND_API_KEY`, `MAILER_API_SECRET` |
| `simse-auth` | None (uses only D1 + Queue) |
| `simse-landing` | None (uses only D1 + Queue) |
| `simse-cloud` | None |

**Services with only infrastructure bindings** (D1, Queue) — no secrets store binding needed.

## Secrets Registry

| Key | Value type | Used by |
|-----|-----------|---------|
| `AUTH_API_URL` | URL | simse-api |
| `AUTH_API_SECRET` | Token | simse-api |
| `PAYMENTS_API_URL` | URL | simse-api |
| `PAYMENTS_API_SECRET` | Token | simse-api |
| `MAILER_API_URL` | URL | simse-api |
| `RESEND_API_KEY` | API key | simse-mailer |
| `MAILER_API_SECRET` | Token | simse-mailer |

## wrangler.toml binding

Applied to simse-api and simse-mailer only:

```toml
[[secrets_store.bindings]]
binding = "SECRETS"
store_id = "PLACEHOLDER_FILL_AFTER_CREATION"
```

## TypeScript Env Interface

```typescript
// simse-api/src/types.ts
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
}

// simse-mailer/src/types.ts
export interface Env {
  DB: D1Database;
  SECRETS: SecretsStoreNamespace;
}

export interface MailerSecrets {
  resendApiKey: string;
  mailerApiSecret: string;
}
```

## Middleware Pattern

Each service has a `src/middleware/secrets.ts` that fetches all its secrets in parallel and sets them on the Hono context:

```typescript
// simse-api: src/middleware/secrets.ts
import { createMiddleware } from 'hono/factory';
import type { Env, ApiSecrets } from '../types';

export const secretsMiddleware = createMiddleware<{
  Bindings: Env;
  Variables: { secrets: ApiSecrets };
}>(async (c, next) => {
  const [authApiUrl, authApiSecret, paymentsApiUrl, paymentsApiSecret, mailerApiUrl] =
    await Promise.all([
      c.env.SECRETS.get('AUTH_API_URL'),
      c.env.SECRETS.get('AUTH_API_SECRET'),
      c.env.SECRETS.get('PAYMENTS_API_URL'),
      c.env.SECRETS.get('PAYMENTS_API_SECRET'),
      c.env.SECRETS.get('MAILER_API_URL'),
    ]);

  if (!authApiUrl || !authApiSecret || !paymentsApiUrl || !paymentsApiSecret || !mailerApiUrl) {
    return c.json({ error: 'Service misconfigured' }, 500);
  }

  c.set('secrets', { authApiUrl, authApiSecret, paymentsApiUrl, paymentsApiSecret, mailerApiUrl });
  await next();
});
```

Route handlers use `c.var.secrets.authApiUrl` instead of `c.env.AUTH_API_URL`.

## Migration from Current Plans

The existing service extraction plan uses `c.env.AUTH_API_URL` etc. as direct string properties. The secrets store plan adds:

1. Replace all `c.env.SECRET_NAME` references with `c.var.secrets.secretName`
2. Add `SECRETS: SecretsStoreNamespace` to Env interfaces, remove string secret fields
3. Add `[[secrets_store.bindings]]` to wrangler.toml, remove secret comments
4. Add `secretsMiddleware` to index.ts before route registration
5. Create `simse-secrets` store and populate secrets

## Principles

- Infrastructure bindings (D1, Queue) stay in wrangler.toml as-is — not secrets
- Secrets fetched in parallel per request (negligible overhead)
- Missing secrets return 500 immediately, not a partial request
- Store ID is a placeholder in code — filled after store creation in Cloudflare dashboard
