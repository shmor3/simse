# simse-api Design

**Date:** 2026-03-04
**Status:** Approved

## Overview

Central API gateway for simse. Owns authentication, user/team management, and notifications. Gates access to backend services (simse-payments, simse-mailer). Dual auth: session tokens for web (simse-cloud), API keys for CLI.

## Architecture

**Runtime:** Cloudflare Worker (Hono + D1 + KV)
**Database:** Own D1 instance (users, sessions, tokens, teams, team_members, team_invites, notifications, api_keys)
**Auth:** Session tokens (web) + API keys (CLI). Auth middleware resolves either to user context.
**Dependencies:** `hono`, `zod`

### Service Topology

```
simse-cloud (React frontend)
    |
    v
simse-api (this service) --- owns auth, users, teams, notifications
    |         |
    v         v
simse-payments   simse-mailer
```

simse-cloud becomes a pure frontend. All server-side logic moves to simse-api.

## API Surface

### Auth

| Method | Route | Purpose |
|--------|-------|---------|
| `POST` | `/auth/register` | Create account + default team |
| `POST` | `/auth/login` | Login, return session token |
| `POST` | `/auth/logout` | Invalidate session |
| `POST` | `/auth/verify-email` | Verify email with code |
| `POST` | `/auth/reset-password` | Request password reset |
| `POST` | `/auth/new-password` | Complete password reset |
| `POST` | `/auth/2fa` | Verify 2FA code |
| `GET` | `/auth/me` | Get current user + team |

### Users

| Method | Route | Purpose |
|--------|-------|---------|
| `PUT` | `/users/me/name` | Update display name |
| `PUT` | `/users/me/password` | Change password |
| `DELETE` | `/users/me` | Delete account (cascade) |

### Teams

| Method | Route | Purpose |
|--------|-------|---------|
| `GET` | `/teams/me` | Get user's team + members + invites |
| `POST` | `/teams/me/invite` | Invite team member |
| `PUT` | `/teams/me/members/:userId/role` | Change member role |
| `DELETE` | `/teams/me/invites/:inviteId` | Revoke invite |

### Notifications

| Method | Route | Purpose |
|--------|-------|---------|
| `GET` | `/notifications` | List notifications (limit 100) |
| `PUT` | `/notifications/:id/read` | Mark single as read |
| `PUT` | `/notifications/read-all` | Mark all as read |

### Gateway (proxy to backend services)

| Method | Route | Proxied to |
|--------|-------|------------|
| `*` | `/payments/*` | simse-payments (strips /payments prefix) |
| `POST` | `/emails/send` | simse-mailer /send |

### API Keys (for CLI)

| Method | Route | Purpose |
|--------|-------|---------|
| `POST` | `/api-keys` | Create API key |
| `GET` | `/api-keys` | List API keys |
| `DELETE` | `/api-keys/:id` | Revoke API key |

### Health

| Method | Route | Purpose |
|--------|-------|---------|
| `GET` | `/health` | Health check |

## Auth Model

### Web (simse-cloud)
1. `POST /auth/login` with email + password
2. Response: `{ token: "session_xxx", user: { ... } }`
3. simse-cloud stores token, sends `Authorization: Bearer session_xxx` on all requests

### CLI
1. User creates API key via web dashboard or `POST /api-keys`
2. CLI sends `Authorization: Bearer sk_xxx` on all requests

### Middleware
Auth middleware checks `Authorization` header:
- Starts with `session_` → validate against sessions table
- Starts with `sk_` → validate against api_keys table
- Either resolves to `{ userId, sessionId? }` context

## Database Schema (D1)

```sql
CREATE TABLE users (
  id TEXT PRIMARY KEY,
  email TEXT UNIQUE NOT NULL,
  name TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  email_verified INTEGER DEFAULT 0,
  two_factor_enabled INTEGER DEFAULT 0,
  two_factor_secret TEXT,
  created_at TEXT DEFAULT (datetime('now')),
  updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  expires_at TEXT NOT NULL,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE tokens (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  type TEXT NOT NULL,
  code TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used INTEGER DEFAULT 0,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE teams (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  plan TEXT DEFAULT 'free',
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE team_members (
  team_id TEXT NOT NULL REFERENCES teams(id),
  user_id TEXT NOT NULL REFERENCES users(id),
  role TEXT NOT NULL DEFAULT 'member',
  joined_at TEXT DEFAULT (datetime('now')),
  PRIMARY KEY (team_id, user_id)
);

CREATE TABLE team_invites (
  id TEXT PRIMARY KEY,
  team_id TEXT NOT NULL REFERENCES teams(id),
  email TEXT NOT NULL,
  role TEXT NOT NULL DEFAULT 'member',
  invited_by TEXT NOT NULL REFERENCES users(id),
  expires_at TEXT NOT NULL,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE notifications (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  type TEXT NOT NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  read INTEGER DEFAULT 0,
  link TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE api_keys (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  name TEXT NOT NULL,
  key_hash TEXT NOT NULL,
  key_prefix TEXT NOT NULL,
  last_used_at TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
CREATE INDEX idx_tokens_user_type ON tokens(user_id, type);
CREATE INDEX idx_tokens_code ON tokens(code);
CREATE INDEX idx_team_members_user ON team_members(user_id);
CREATE INDEX idx_team_invites_email ON team_invites(email);
CREATE INDEX idx_notifications_user ON notifications(user_id, read);
CREATE INDEX idx_api_keys_user ON api_keys(user_id);
CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
```

## Secrets

Set via `wrangler secret put`:
- `SESSION_SECRET` — HMAC key for session token generation
- `PAYMENTS_API_URL` — simse-payments endpoint
- `PAYMENTS_API_SECRET` — simse-payments auth token
- `MAILER_API_URL` — simse-mailer endpoint
- `MAILER_API_SECRET` — simse-mailer auth token

## Project Structure

```
simse-api/
├── src/
│   ├── index.ts              # Hono app, route assembly
│   ├── types.ts              # Env interface, AuthContext
│   ├── middleware/
│   │   └── auth.ts           # Auth middleware (session + API key)
│   ├── lib/
│   │   ├── db.ts             # generateId, query helpers
│   │   ├── password.ts       # hashPassword, verifyPassword (PBKDF2)
│   │   ├── session.ts        # createSession, validateSession
│   │   ├── token.ts          # createToken, validateToken, generateCode
│   │   ├── api-key.ts        # createApiKey, validateApiKey
│   │   ├── email.ts          # sendEmail via simse-mailer
│   │   └── proxy.ts          # Gateway proxy helper
│   ├── routes/
│   │   ├── auth.ts           # register, login, logout, verify, reset, 2fa, me
│   │   ├── users.ts          # update name, password, delete
│   │   ├── teams.ts          # team info, invite, role change, revoke
│   │   ├── notifications.ts  # list, mark read
│   │   ├── api-keys.ts       # create, list, revoke
│   │   └── gateway.ts        # proxy to payments + mailer
│   └── schemas.ts            # Zod validation schemas
├── migrations/
│   └── 0001_initial.sql
├── package.json
├── tsconfig.json
├── wrangler.toml
├── biome.json
└── moon.yml
```

## Response Format

All responses use consistent JSON format:

```json
// Success
{ "data": { ... } }

// Error
{ "error": { "code": "INVALID_CREDENTIALS", "message": "..." } }
```

## Gateway Proxy Behavior

The `/payments/*` and `/emails/*` routes proxy requests to backend services:
1. Strip the prefix (`/payments/subscriptions/123` → `/subscriptions/123`)
2. Add backend service auth header (`Authorization: Bearer <service-secret>`)
3. Forward request body and method
4. Return response as-is

This lets simse-cloud call `simse-api/payments/subscriptions/team123` and it transparently reaches simse-payments.

## Migration Notes

- simse-api gets its own D1 (fresh database)
- Data migration from simse-cloud D1 is a follow-up task
- simse-cloud frontend migration (calling simse-api instead of D1) is Phase 2
- During transition, both simse-cloud and simse-api can coexist
