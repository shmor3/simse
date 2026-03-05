# Service Extraction Design: Auth, Payments, Email

**Date:** 2026-03-05
**Status:** Approved

## Overview

Extract auth, payments, and email logic from simse-cloud into dedicated services. simse-cloud becomes a thin frontend calling simse-api, which acts as a gateway proxy to backend services.

## Architecture

| Service | Owns | Platform |
|---------|------|----------|
| **simse-auth** (new) | Users, sessions, tokens, API keys, teams, members, invites, RBAC | Cloudflare Worker, own D1 |
| **simse-payments** (exists) | Subscriptions, credits, usage, billing | Existing service |
| **simse-mailer** (expanded) | All email templates, rendering, delivery, in-app notifications | Cloudflare Worker, own D1 |
| **simse-api** (simplified) | Gateway proxy — authenticates via simse-auth, routes to services | Cloudflare Worker, no DB |
| **simse-cloud** (thinned) | Frontend only — calls simse-api, manages cookies | Cloudflare Pages |

### Principles

- Each service owns its domain end-to-end
- Services own their own emails (auth emails in simse-auth templates rendered by simse-mailer, payment emails owned by simse-payments)
- simse-mailer is shared infrastructure for rendering + delivery
- simse-api is the single entry point for simse-cloud
- Fire-and-forget operations (emails, notifications) go through Cloudflare Queues; synchronous operations (auth, payments, reading data) use direct HTTP

### Cloudflare Queues

| Queue | Producer | Consumer | Message types |
|-------|----------|----------|---------------|
| `simse-auth-comms` | simse-auth | simse-mailer | emails + notifications |
| `simse-api-comms` | simse-api | simse-mailer | emails + notifications |
| `simse-landing-comms` | simse-landing | simse-mailer | emails |

---

## simse-auth

**Domain:** `auth.simse.dev`
**Database:** `simse-auth-db` (D1)

### Tables

- `users` — id, email, name, password_hash, email_verified, two_factor_enabled, two_factor_secret, created_at, updated_at
- `sessions` — id, user_id, expires_at, created_at
- `tokens` — id, user_id, type, code, expires_at, used, created_at
- `api_keys` — id, user_id, name, key_hash, key_prefix, last_used_at, created_at
- `teams` — id, name, plan, created_at
- `team_members` — team_id, user_id, role, joined_at
- `team_invites` — id, team_id, email, role, invited_by, expires_at, created_at

### Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | /auth/register | Create user + team + session |
| POST | /auth/login | Validate credentials, return session or 2FA pending |
| POST | /auth/2fa | Verify 2FA code, create session |
| POST | /auth/logout | Delete session |
| POST | /auth/reset-password | Create reset token, call simse-mailer |
| POST | /auth/new-password | Validate token, update password |
| POST | /auth/verify-email | Validate email verification token |
| GET | /auth/me | Get current user profile + team + role |
| POST | /auth/validate | Validate session/API key (called by gateway) |
| PUT | /users/me/name | Update name |
| PUT | /users/me/password | Change password |
| DELETE | /users/me | Delete account (cascade) |
| GET | /teams/me | Get team, members, invites |
| POST | /teams/me/invite | Create invite, call simse-mailer |
| PUT | /teams/me/members/:userId/role | Change member role |
| DELETE | /teams/me/invites/:inviteId | Revoke invite |
| POST | /api-keys | Create API key |
| GET | /api-keys | List API keys |
| DELETE | /api-keys/:id | Delete API key |

### /auth/validate

Called by simse-api gateway on every authenticated request. Accepts Bearer token (session or API key), returns:

```json
{ "userId": "...", "teamId": "...", "role": "owner" }
```

### Sends to simse-mailer (via `simse-auth-comms` queue)

All email and notification sends use `env.COMMS_QUEUE.send(message)` (not HTTP):

- Registration: `{ type: "email", template: "verify-email", to, props: { code } }`
- Login (2FA): `{ type: "email", template: "two-factor", to, props: { code } }`
- Password reset: `{ type: "email", template: "reset-password", to, props: { resetUrl } }`
- Team invite: `{ type: "email", template: "team-invite", to, props: { inviterName, teamName, inviteUrl } }`
- Role change: `{ type: "email", template: "role-change", to, props: { teamName, newRole } }`

---

## simse-mailer (expanded)

**Domain:** `mailer.simse.dev`
**Database:** `simse-mailer-db` (D1)

### New table

- `notifications` — id, user_id, type, title, body, read, link, created_at

### Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| POST | /send | Render template + deliver email |
| GET | /notifications/:userId | List notifications (last 100) |
| POST | /notifications | Create notification |
| PUT | /notifications/:id/read | Mark single as read |
| PUT | /notifications/:userId/read-all | Mark all as read |

### POST /send

```json
{
  "template": "verify-email",
  "to": "user@example.com",
  "props": { "code": "123456", "verifyUrl": "..." }
}
```

### Templates (21 total — 18 from simse-cloud + 3 from simse-landing)

| Category | Templates |
|----------|-----------|
| Auth | verify, two-factor, reset-password, email-change, new-device, suspicious-activity |
| Onboarding | onboarding, re-engagement |
| Billing | payment-receipt, payment-failed, usage-warning, free-credit |
| Team | team-invite, role-change, invite-friend |
| Digest | weekly-digest |
| Product | feature-announcement |
| Waitlist | waitlist-welcome, waitlist-early-preview, waitlist-invite |

### Queue consumer

simse-mailer exports both `fetch` (HTTP) and `queue` (batch consumer) handlers. The queue handler processes all three producer queues (`simse-auth-comms`, `simse-api-comms`, `simse-landing-comms`). Each message is either `{ type: "email", template, to, props }` or `{ type: "notification", userId, kind, title, body, link }`.

### Auth between services

Notification reads (GET/PUT) accessed via simse-api gateway with `X-User-Id` header. Email/notification writes go through queues (infrastructure-level trust — no auth needed).

---

## simse-api (gateway)

**Domain:** `api.simse.dev`
**Database:** None (drops D1 binding)

### Auth flow

1. Extract `Authorization: Bearer <token>` header
2. Call `POST auth.simse.dev/auth/validate` with the token
3. If valid, attach `X-User-Id`, `X-Team-Id`, `X-Role` headers
4. Proxy to downstream service
5. If invalid, return 401

### Route map

| Incoming path | Proxies to | Service |
|---------------|-----------|---------|
| /auth/* | auth.simse.dev/auth/* | simse-auth |
| /users/* | auth.simse.dev/users/* | simse-auth |
| /teams/* | auth.simse.dev/teams/* | simse-auth |
| /api-keys/* | auth.simse.dev/api-keys/* | simse-auth |
| /payments/* | payments.simse.dev/payments/* | simse-payments |
| /notifications/* | mailer.simse.dev/notifications/* | simse-mailer |
| GET /health | Local 200 | -- |

### Public routes (no auth validation)

- POST /auth/register
- POST /auth/login
- POST /auth/2fa
- POST /auth/reset-password
- POST /auth/new-password
- POST /auth/verify-email

### Files to delete

- `src/lib/password.ts`, `token.ts`, `session.ts`, `api-key.ts`, `email.ts`, `db.ts`
- `src/middleware/auth.ts`
- `src/routes/auth.ts`, `users.ts`, `teams.ts`, `api-keys.ts`, `notifications.ts`
- `src/schemas.ts`

### What remains

- `src/index.ts` — route map + proxy logic
- `src/routes/gateway.ts` — generic proxy helper
- `src/types.ts` — simplified (service URLs/secrets only)

---

## simse-cloud (thin frontend)

### API client

New `app/lib/api.server.ts`:

```typescript
async function apiClient(request: Request, path: string, options?: RequestInit) {
  const session = getSessionFromCookie(request);
  return fetch(`${API_URL}${path}`, {
    ...options,
    headers: { Authorization: `Bearer ${session}`, ...options?.headers },
  });
}
```

### Session/cookie

`app/lib/session.server.ts` simplified to cookie read/write only. On login/register: API returns session token, store in httpOnly cookie. On logout: clear cookie + call API.

### Files to delete

- `app/lib/auth.server.ts`
- `app/lib/db.server.ts`
- `app/lib/security.server.ts`
- `app/lib/payments.server.ts`
- `app/lib/email.server.ts`
- `app/lib/schemas.ts`
- `app/emails/` (entire directory)

### Environment

```
API_URL=https://api.simse.dev
```

No more PAYMENTS_API_URL, PAYMENTS_API_SECRET, EMAIL_API_URL, EMAIL_API_SECRET, no D1 binding.

---

## Migration Strategy

### Slice 1: Auth

1. Create `simse-auth/` worker with its own D1
2. Migrate all auth/user/team/API key logic from simse-api
3. Add `/auth/validate` endpoint
4. Add simse-mailer calls for auth emails
5. Update simse-api to become gateway (proxy to simse-auth, delete old code)
6. Update simse-cloud auth routes to call simse-api
7. Delete dead code from simse-cloud

### Slice 2: Communications

1. Expand simse-mailer with notifications table + endpoints
2. Move all 18 email templates from simse-cloud to simse-mailer
3. Add simse-api gateway route for /notifications/*
4. Update simse-cloud notification routes to call simse-api
5. Delete `app/emails/` and `email.server.ts` from simse-cloud

### Slice 3: Payments + cleanup

1. Update simse-cloud billing/usage routes to call simse-api instead of payments service directly
2. Delete `payments.server.ts` from simse-cloud
3. Remove D1 binding from simse-cloud wrangler.toml
4. Remove all unused env vars
5. Final cleanup pass

### Data migration

User data in simse-api-db needs to be migrated to simse-auth-db via SQL export/import or one-time migration script.
