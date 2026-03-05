# Service Extraction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract auth, payments, and email logic from simse-cloud into dedicated services (simse-auth, expanded simse-mailer), with simse-api as a gateway proxy.

**Architecture:** Vertical slices — Slice 1 builds simse-auth + wires it through simse-api + updates simse-cloud. Slice 2 expands simse-mailer with templates + notifications. Slice 3 routes payments through the gateway and cleans up.

**Tech Stack:** TypeScript, Hono, Cloudflare Workers, D1 (SQLite), Zod v4, React Email, Resend

---

## Slice 1: Auth Service

### Task 1: Scaffold simse-auth worker

**Files:**
- Create: `simse-auth/package.json`
- Create: `simse-auth/tsconfig.json`
- Create: `simse-auth/biome.json`
- Create: `simse-auth/wrangler.toml`
- Create: `simse-auth/.gitignore`
- Create: `simse-auth/src/index.ts`
- Create: `simse-auth/src/types.ts`

**Step 1: Create package.json**

```json
{
	"name": "simse-auth",
	"private": true,
	"type": "module",
	"scripts": {
		"dev": "wrangler dev",
		"build": "wrangler deploy --dry-run --outdir dist",
		"deploy": "wrangler deploy",
		"lint": "biome check .",
		"lint:fix": "biome check --write .",
		"db:migrate": "wrangler d1 migrations apply simse-auth-db --local",
		"db:migrate:prod": "wrangler d1 migrations apply simse-auth-db --remote"
	},
	"dependencies": {
		"hono": "^4.7.0",
		"zod": "^4.3.6"
	},
	"devDependencies": {
		"@biomejs/biome": "^2.3.12",
		"@cloudflare/workers-types": "^4.20260305.0",
		"typescript": "^5.7.0",
		"wrangler": "^4.0.0"
	}
}
```

**Step 2: Create wrangler.toml**

```toml
name = "simse-auth"
compatibility_date = "2025-04-01"
main = "src/index.ts"

workers_dev = true

routes = [
  { pattern = "auth.simse.dev", custom_domain = true }
]

[[d1_databases]]
binding = "DB"
database_name = "simse-auth-db"
database_id = "PLACEHOLDER_FILL_AFTER_CREATION"

# Secrets (set via `wrangler secret put`):
# MAILER_API_URL
# MAILER_API_SECRET
```

**Step 3: Create tsconfig.json** (copy from simse-api)

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

**Step 4: Create biome.json** (copy from simse-api)

**Step 5: Create .gitignore**

```
node_modules/
dist/
.wrangler/
```

**Step 6: Create src/types.ts**

```typescript
export interface Env {
	DB: D1Database;
	MAILER_API_URL: string;
	MAILER_API_SECRET: string;
}

export interface AuthContext {
	userId: string;
	sessionId?: string;
}
```

**Step 7: Create src/index.ts** (minimal health check)

```typescript
import { Hono } from 'hono';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));

export default app;
```

**Step 8: Install dependencies**

Run: `cd simse-auth && bun install`

**Step 9: Commit**

```
feat(simse-auth): scaffold auth service worker
```

---

### Task 2: Create simse-auth database migration

**Files:**
- Create: `simse-auth/migrations/0001_initial.sql`

**Step 1: Create migration**

```sql
-- Users
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

-- Sessions
CREATE TABLE sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  expires_at TEXT NOT NULL,
  created_at TEXT DEFAULT (datetime('now'))
);

-- Tokens (email verify, password reset, 2FA)
CREATE TABLE tokens (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  type TEXT NOT NULL,
  code TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  used INTEGER DEFAULT 0,
  created_at TEXT DEFAULT (datetime('now'))
);

-- Teams
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

-- API Keys (for CLI)
CREATE TABLE api_keys (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  name TEXT NOT NULL,
  key_hash TEXT NOT NULL,
  key_prefix TEXT NOT NULL,
  last_used_at TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);

-- Indexes
CREATE INDEX idx_sessions_user ON sessions(user_id);
CREATE INDEX idx_sessions_expires ON sessions(expires_at);
CREATE INDEX idx_tokens_user_type ON tokens(user_id, type);
CREATE INDEX idx_tokens_code ON tokens(code);
CREATE INDEX idx_team_members_user ON team_members(user_id);
CREATE INDEX idx_team_invites_email ON team_invites(email);
CREATE INDEX idx_api_keys_user ON api_keys(user_id);
CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
```

**Step 2: Run migration locally**

Run: `cd simse-auth && bun run db:migrate`
Expected: Migration applied successfully

**Step 3: Commit**

```
feat(simse-auth): add database migration
```

---

### Task 3: Move lib helpers to simse-auth

**Files:**
- Create: `simse-auth/src/lib/db.ts` (from `simse-api/src/lib/db.ts`)
- Create: `simse-auth/src/lib/password.ts` (from `simse-api/src/lib/password.ts`)
- Create: `simse-auth/src/lib/token.ts` (from `simse-api/src/lib/token.ts`)
- Create: `simse-auth/src/lib/session.ts` (from `simse-api/src/lib/session.ts`)
- Create: `simse-auth/src/lib/api-key.ts` (from `simse-api/src/lib/api-key.ts`)
- Create: `simse-auth/src/lib/mailer.ts` (new — calls simse-mailer)
- Create: `simse-auth/src/schemas.ts` (from `simse-api/src/schemas.ts`)

**Step 1: Copy db.ts, password.ts, token.ts, session.ts, api-key.ts, schemas.ts**

These are direct copies from simse-api — identical code.

**Step 2: Create src/lib/mailer.ts**

```typescript
export async function sendTemplateEmail(
	mailerUrl: string,
	mailerSecret: string,
	template: string,
	to: string,
	props: Record<string, string>,
): Promise<void> {
	const res = await fetch(`${mailerUrl}/send`, {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${mailerSecret}`,
			'Content-Type': 'application/json',
		},
		body: JSON.stringify({ template, to, props }),
	});

	if (!res.ok) {
		const body = await res.text();
		console.error(`Mailer error (${res.status}): ${body}`);
	}
}
```

**Step 3: Commit**

```
feat(simse-auth): add lib helpers and schemas
```

---

### Task 4: Add auth routes to simse-auth

**Files:**
- Create: `simse-auth/src/routes/auth.ts`
- Modify: `simse-auth/src/index.ts`

**Step 1: Create src/routes/auth.ts**

Copy from `simse-api/src/routes/auth.ts` with these additions:

1. After registration, call simse-mailer with `verify-email` template
2. After 2FA token creation in login, call simse-mailer with `two-factor` template
3. After password reset token creation, call simse-mailer with `reset-password` template

```typescript
import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { sendTemplateEmail } from '../lib/mailer';
import { hashPassword, verifyPassword } from '../lib/password';
import { createSession, deleteSession } from '../lib/session';
import { createToken, generateCode, markTokenUsed, validateToken } from '../lib/token';
import {
	loginSchema,
	newPasswordSchema,
	registerSchema,
	resetPasswordSchema,
	twoFactorSchema,
} from '../schemas';
import type { AuthContext, Env } from '../types';

const auth = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// POST /auth/register
auth.post('/register', async (c) => {
	const body = await c.req.json();
	const parsed = registerSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const { name, email, password } = parsed.data;
	const normalizedEmail = email.toLowerCase();
	const db = c.env.DB;

	const existing = await db
		.prepare('SELECT id FROM users WHERE LOWER(email) = ?')
		.bind(normalizedEmail)
		.first();

	if (existing) {
		return c.json({ error: { code: 'EMAIL_EXISTS', message: 'Email already registered' } }, 409);
	}

	const userId = generateId();
	const passwordHash = await hashPassword(password);
	const teamId = generateId();
	const tokenId = generateId();
	const verifyCode = generateCode();
	const tokenExpires = new Date(Date.now() + 15 * 60 * 1000).toISOString();

	await db.batch([
		db.prepare('INSERT INTO users (id, email, name, password_hash) VALUES (?, ?, ?, ?)').bind(userId, normalizedEmail, name, passwordHash),
		db.prepare('INSERT INTO teams (id, name) VALUES (?, ?)').bind(teamId, `${name}'s Team`),
		db.prepare("INSERT INTO team_members (team_id, user_id, role) VALUES (?, ?, 'owner')").bind(teamId, userId),
		db.prepare('INSERT INTO tokens (id, user_id, type, code, expires_at) VALUES (?, ?, ?, ?, ?)').bind(tokenId, userId, 'email_verify', verifyCode, tokenExpires),
	]);

	const token = await createSession(db, userId);

	// Send verification email
	await sendTemplateEmail(c.env.MAILER_API_URL, c.env.MAILER_API_SECRET, 'verify-email', normalizedEmail, { code: verifyCode });

	return c.json({
		data: {
			token,
			user: { id: userId, email: normalizedEmail, name },
		},
	}, 201);
});

// POST /auth/login
auth.post('/login', async (c) => {
	const body = await c.req.json();
	const parsed = loginSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const { email, password } = parsed.data;
	const db = c.env.DB;

	const user = await db
		.prepare('SELECT id, email, password_hash, two_factor_enabled FROM users WHERE LOWER(email) = ?')
		.bind(email.toLowerCase())
		.first<{ id: string; email: string; password_hash: string; two_factor_enabled: number }>();

	if (!user || !(await verifyPassword(password, user.password_hash))) {
		return c.json({ error: { code: 'INVALID_CREDENTIALS', message: 'Invalid email or password' } }, 401);
	}

	if (user.two_factor_enabled) {
		const { id, code } = await createToken(db, user.id, '2fa', 10);
		// Send 2FA code via email
		await sendTemplateEmail(c.env.MAILER_API_URL, c.env.MAILER_API_SECRET, 'two-factor', user.email, { code });
		return c.json({ data: { requires2fa: true, pendingToken: id } });
	}

	const token = await createSession(db, user.id);
	return c.json({ data: { token, user: { id: user.id } } });
});

// POST /auth/2fa
auth.post('/2fa', async (c) => {
	const body = await c.req.json();
	const parsed = twoFactorSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const { code, pendingToken } = parsed.data;
	const db = c.env.DB;

	const pending = await db
		.prepare("SELECT user_id FROM tokens WHERE id = ? AND type = '2fa' AND used = 0 AND expires_at > datetime('now')")
		.bind(pendingToken)
		.first<{ user_id: string }>();

	if (!pending) {
		return c.json({ error: { code: 'INVALID_TOKEN', message: '2FA session expired' } }, 401);
	}

	const codeToken = await validateToken(db, code, '2fa');
	if (!codeToken || codeToken.userId !== pending.user_id) {
		return c.json({ error: { code: 'INVALID_CODE', message: 'Invalid 2FA code' } }, 401);
	}

	await markTokenUsed(db, pendingToken);
	await markTokenUsed(db, codeToken.id);

	const token = await createSession(db, pending.user_id);
	return c.json({ data: { token, user: { id: pending.user_id } } });
});

// POST /auth/logout (requires auth — called via gateway with X-User-Id)
auth.post('/logout', async (c) => {
	const sessionId = c.req.header('X-Session-Id');
	if (sessionId) {
		await deleteSession(c.env.DB, sessionId);
	}
	return c.json({ data: { ok: true } });
});

// POST /auth/reset-password
auth.post('/reset-password', async (c) => {
	const body = await c.req.json();
	const parsed = resetPasswordSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const user = await db
		.prepare('SELECT id, email FROM users WHERE LOWER(email) = ?')
		.bind(parsed.data.email.toLowerCase())
		.first<{ id: string; email: string }>();

	if (user) {
		const { code } = await createToken(db, user.id, 'password_reset', 60);
		await sendTemplateEmail(c.env.MAILER_API_URL, c.env.MAILER_API_SECRET, 'reset-password', user.email, { code });
	}

	return c.json({ data: { ok: true } });
});

// POST /auth/new-password
auth.post('/new-password', async (c) => {
	const body = await c.req.json();
	const parsed = newPasswordSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const token = await validateToken(db, parsed.data.token, 'password_reset');
	if (!token) {
		return c.json({ error: { code: 'INVALID_TOKEN', message: 'Invalid or expired reset token' } }, 400);
	}

	const passwordHash = await hashPassword(parsed.data.password);
	await db.prepare('UPDATE users SET password_hash = ? WHERE id = ?').bind(passwordHash, token.userId).run();
	await markTokenUsed(db, token.id);

	return c.json({ data: { ok: true } });
});

// POST /auth/verify-email
auth.post('/verify-email', async (c) => {
	const body = await c.req.json<{ code: string }>();
	if (!body.code) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: 'Code required' } }, 400);
	}

	const db = c.env.DB;
	const token = await validateToken(db, body.code, 'email_verify');
	if (!token) {
		return c.json({ error: { code: 'INVALID_TOKEN', message: 'Invalid or expired code' } }, 400);
	}

	await db.prepare('UPDATE users SET email_verified = 1 WHERE id = ?').bind(token.userId).run();
	await markTokenUsed(db, token.id);

	return c.json({ data: { ok: true } });
});

// POST /auth/validate — called by simse-api gateway
auth.post('/validate', async (c) => {
	const body = await c.req.json<{ token: string }>();
	if (!body.token) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: 'Token required' } }, 400);
	}

	const db = c.env.DB;
	const token = body.token;
	let userId: string | null = null;
	let sessionId: string | undefined;

	if (token.startsWith('session_')) {
		const row = await db
			.prepare("SELECT user_id FROM sessions WHERE id = ? AND expires_at > datetime('now')")
			.bind(token)
			.first<{ user_id: string }>();
		if (row) {
			userId = row.user_id;
			sessionId = token;
		}
	} else if (token.startsWith('sk_')) {
		const encoder = new TextEncoder();
		const data = encoder.encode(token);
		const hashBuffer = await crypto.subtle.digest('SHA-256', data);
		const hashArray = new Uint8Array(hashBuffer);
		const keyHash = btoa(String.fromCharCode(...hashArray));

		const row = await db
			.prepare('SELECT user_id FROM api_keys WHERE key_hash = ?')
			.bind(keyHash)
			.first<{ user_id: string }>();

		if (row) {
			userId = row.user_id;
			await db.prepare("UPDATE api_keys SET last_used_at = datetime('now') WHERE key_hash = ?").bind(keyHash).run();
		}
	}

	if (!userId) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	// Get team info for RBAC
	const team = await db
		.prepare('SELECT t.id, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1')
		.bind(userId)
		.first<{ id: string; role: string }>();

	return c.json({
		data: {
			userId,
			sessionId,
			teamId: team?.id ?? null,
			role: team?.role ?? null,
		},
	});
});

// GET /auth/me — requires auth headers from gateway
auth.get('/me', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);
	}

	const db = c.env.DB;
	const user = await db
		.prepare('SELECT id, email, name, email_verified, two_factor_enabled, created_at FROM users WHERE id = ?')
		.bind(userId)
		.first<{
			id: string;
			email: string;
			name: string;
			email_verified: number;
			two_factor_enabled: number;
			created_at: string;
		}>();

	if (!user) {
		return c.json({ error: { code: 'NOT_FOUND', message: 'User not found' } }, 404);
	}

	const team = await db
		.prepare('SELECT t.id, t.name, t.plan, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1')
		.bind(userId)
		.first<{ id: string; name: string; plan: string; role: string }>();

	return c.json({
		data: {
			id: user.id,
			email: user.email,
			name: user.name,
			emailVerified: !!user.email_verified,
			twoFactorEnabled: !!user.two_factor_enabled,
			createdAt: user.created_at,
			team: team ? { id: team.id, name: team.name, plan: team.plan, role: team.role } : null,
		},
	});
});

export default auth;
```

**Step 2: Wire into index.ts**

```typescript
import { Hono } from 'hono';
import auth from './routes/auth';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));
app.route('/auth', auth);

export default app;
```

**Step 3: Commit**

```
feat(simse-auth): add auth routes with email integration
```

---

### Task 5: Add user routes to simse-auth

**Files:**
- Create: `simse-auth/src/routes/users.ts`
- Modify: `simse-auth/src/index.ts`

**Step 1: Create src/routes/users.ts**

Copy from `simse-api/src/routes/users.ts` but use `X-User-Id` header instead of auth middleware context:

```typescript
import { Hono } from 'hono';
import { hashPassword, verifyPassword } from '../lib/password';
import { changePasswordSchema, deleteAccountSchema, updateNameSchema } from '../schemas';
import type { Env } from '../types';

const users = new Hono<{ Bindings: Env }>();

// PUT /users/me/name
users.put('/me/name', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const body = await c.req.json();
	const parsed = updateNameSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	await c.env.DB.prepare('UPDATE users SET name = ? WHERE id = ?').bind(parsed.data.name, userId).run();
	return c.json({ data: { ok: true } });
});

// PUT /users/me/password
users.put('/me/password', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const body = await c.req.json();
	const parsed = changePasswordSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const user = await db.prepare('SELECT password_hash FROM users WHERE id = ?').bind(userId).first<{ password_hash: string }>();
	if (!user || !(await verifyPassword(parsed.data.currentPassword, user.password_hash))) {
		return c.json({ error: { code: 'INVALID_PASSWORD', message: 'Current password is incorrect' } }, 400);
	}

	const newHash = await hashPassword(parsed.data.newPassword);
	await db.prepare('UPDATE users SET password_hash = ? WHERE id = ?').bind(newHash, userId).run();
	return c.json({ data: { ok: true } });
});

// DELETE /users/me
users.delete('/me', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const body = await c.req.json();
	const parsed = deleteAccountSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const user = await db.prepare('SELECT email FROM users WHERE id = ?').bind(userId).first<{ email: string }>();
	if (!user || parsed.data.confirmEmail.toLowerCase() !== user.email.toLowerCase()) {
		return c.json({ error: { code: 'EMAIL_MISMATCH', message: 'Email does not match' } }, 400);
	}

	await db.batch([
		db.prepare('DELETE FROM sessions WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM tokens WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM api_keys WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM team_invites WHERE invited_by = ?').bind(userId),
		db.prepare('DELETE FROM team_members WHERE user_id = ?').bind(userId),
		db.prepare('DELETE FROM users WHERE id = ?').bind(userId),
	]);

	return c.json({ data: { ok: true } });
});

export default users;
```

**Step 2: Wire into index.ts**

Add `import users from './routes/users';` and `app.route('/users', users);`

**Step 3: Commit**

```
feat(simse-auth): add user management routes
```

---

### Task 6: Add team routes to simse-auth

**Files:**
- Create: `simse-auth/src/routes/teams.ts`
- Modify: `simse-auth/src/index.ts`

**Step 1: Create src/routes/teams.ts**

Copy from `simse-api/src/routes/teams.ts` but use `X-User-Id` header and add mailer calls for invites:

```typescript
import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { sendTemplateEmail } from '../lib/mailer';
import { inviteSchema } from '../schemas';
import type { Env } from '../types';

const teams = new Hono<{ Bindings: Env }>();

// GET /teams/me
teams.get('/me', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const db = c.env.DB;
	const team = await db
		.prepare('SELECT t.id, t.name, t.plan FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1')
		.bind(userId)
		.first<{ id: string; name: string; plan: string }>();

	if (!team) {
		return c.json({ error: { code: 'NOT_FOUND', message: 'No team found' } }, 404);
	}

	const members = await db
		.prepare('SELECT u.id, u.name, u.email, tm.role, tm.joined_at FROM team_members tm JOIN users u ON tm.user_id = u.id WHERE tm.team_id = ?')
		.bind(team.id)
		.all<{ id: string; name: string; email: string; role: string; joined_at: string }>();

	const invites = await db
		.prepare("SELECT id, email, role, created_at FROM team_invites WHERE team_id = ? AND expires_at > datetime('now')")
		.bind(team.id)
		.all<{ id: string; email: string; role: string; created_at: string }>();

	return c.json({
		data: {
			id: team.id,
			name: team.name,
			plan: team.plan,
			members: members.results,
			invites: invites.results,
		},
	});
});

// POST /teams/me/invite
teams.post('/me/invite', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const body = await c.req.json();
	const parsed = inviteSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;

	const membership = await db
		.prepare("SELECT t.id as team_id, t.name as team_name FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1")
		.bind(userId)
		.first<{ team_id: string; team_name: string }>();

	if (!membership) {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Only owners and admins can invite' } }, 403);
	}

	const existingMember = await db
		.prepare('SELECT 1 FROM team_members tm JOIN users u ON tm.user_id = u.id WHERE tm.team_id = ? AND LOWER(u.email) = ?')
		.bind(membership.team_id, parsed.data.email.toLowerCase())
		.first();

	if (existingMember) {
		return c.json({ error: { code: 'ALREADY_MEMBER', message: 'User is already a team member' } }, 409);
	}

	const inviteId = generateId();
	const expiresAt = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString();

	await db
		.prepare('INSERT INTO team_invites (id, team_id, email, role, invited_by, expires_at) VALUES (?, ?, ?, ?, ?, ?)')
		.bind(inviteId, membership.team_id, parsed.data.email.toLowerCase(), parsed.data.role, userId, expiresAt)
		.run();

	// Get inviter name for email
	const inviter = await db.prepare('SELECT name FROM users WHERE id = ?').bind(userId).first<{ name: string }>();

	await sendTemplateEmail(c.env.MAILER_API_URL, c.env.MAILER_API_SECRET, 'team-invite', parsed.data.email, {
		inviterName: inviter?.name ?? 'A team member',
		teamName: membership.team_name,
		inviteUrl: `https://app.simse.dev/invite/${inviteId}`,
	});

	return c.json({ data: { id: inviteId } }, 201);
});

// PUT /teams/me/members/:userId/role
teams.put('/me/members/:userId/role', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const targetUserId = c.req.param('userId');
	const body = await c.req.json<{ role: string }>();

	if (!body.role || !['admin', 'member'].includes(body.role)) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: 'Role must be admin or member' } }, 400);
	}

	const db = c.env.DB;

	const membership = await db
		.prepare("SELECT t.id as team_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1")
		.bind(userId)
		.first<{ team_id: string }>();

	if (!membership) {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Insufficient permissions' } }, 403);
	}

	if (targetUserId === userId) {
		return c.json({ error: { code: 'INVALID_OPERATION', message: 'Cannot change your own role' } }, 400);
	}

	const targetMember = await db
		.prepare('SELECT role FROM team_members WHERE team_id = ? AND user_id = ?')
		.bind(membership.team_id, targetUserId)
		.first<{ role: string }>();

	if (targetMember?.role === 'owner') {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Cannot change the owner role' } }, 403);
	}

	await db
		.prepare('UPDATE team_members SET role = ? WHERE team_id = ? AND user_id = ?')
		.bind(body.role, membership.team_id, targetUserId)
		.run();

	return c.json({ data: { ok: true } });
});

// DELETE /teams/me/invites/:inviteId
teams.delete('/me/invites/:inviteId', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const inviteId = c.req.param('inviteId');
	const db = c.env.DB;

	const membership = await db
		.prepare("SELECT t.id as team_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1")
		.bind(userId)
		.first<{ team_id: string }>();

	if (!membership) {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Insufficient permissions' } }, 403);
	}

	await db
		.prepare('DELETE FROM team_invites WHERE id = ? AND team_id = ?')
		.bind(inviteId, membership.team_id)
		.run();

	return c.json({ data: { ok: true } });
});

export default teams;
```

**Step 2: Wire into index.ts**

Add `import teams from './routes/teams';` and `app.route('/teams', teams);`

**Step 3: Commit**

```
feat(simse-auth): add team management routes with invite emails
```

---

### Task 7: Add API key routes to simse-auth

**Files:**
- Create: `simse-auth/src/routes/api-keys.ts`
- Modify: `simse-auth/src/index.ts`

**Step 1: Create src/routes/api-keys.ts**

Copy from `simse-api/src/routes/api-keys.ts` but use `X-User-Id` header:

```typescript
import { Hono } from 'hono';
import { createApiKey } from '../lib/api-key';
import { createApiKeySchema } from '../schemas';
import type { Env } from '../types';

const apiKeys = new Hono<{ Bindings: Env }>();

// POST /api-keys
apiKeys.post('/', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const body = await c.req.json();
	const parsed = createApiKeySchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const result = await createApiKey(c.env.DB, userId, parsed.data.name);
	return c.json({ data: { id: result.id, key: result.key, prefix: result.prefix, name: parsed.data.name } }, 201);
});

// GET /api-keys
apiKeys.get('/', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const keys = await c.env.DB
		.prepare('SELECT id, name, key_prefix, last_used_at, created_at FROM api_keys WHERE user_id = ? ORDER BY created_at DESC')
		.bind(userId)
		.all<{ id: string; name: string; key_prefix: string; last_used_at: string | null; created_at: string }>();

	return c.json({ data: keys.results });
});

// DELETE /api-keys/:id
apiKeys.delete('/:id', async (c) => {
	const userId = c.req.header('X-User-Id');
	if (!userId) return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);

	const id = c.req.param('id');
	await c.env.DB.prepare('DELETE FROM api_keys WHERE id = ? AND user_id = ?').bind(id, userId).run();
	return c.json({ data: { ok: true } });
});

export default apiKeys;
```

**Step 2: Wire into index.ts — final version**

```typescript
import { Hono } from 'hono';
import apiKeys from './routes/api-keys';
import auth from './routes/auth';
import teams from './routes/teams';
import users from './routes/users';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));

// Auth routes (public — gateway forwards without auth check)
app.route('/auth', auth);

// Protected routes (gateway validates token first, passes X-User-Id)
app.route('/users', users);
app.route('/teams', teams);
app.route('/api-keys', apiKeys);

export default app;
```

**Step 3: Verify build**

Run: `cd simse-auth && bun run build`
Expected: Build succeeds

**Step 4: Commit**

```
feat(simse-auth): add API key routes, complete auth service
```

---

### Task 8: Rewrite simse-api as gateway proxy

**Files:**
- Modify: `simse-api/src/index.ts`
- Modify: `simse-api/src/types.ts`
- Modify: `simse-api/src/routes/gateway.ts`
- Delete: `simse-api/src/routes/auth.ts`
- Delete: `simse-api/src/routes/users.ts`
- Delete: `simse-api/src/routes/teams.ts`
- Delete: `simse-api/src/routes/api-keys.ts`
- Delete: `simse-api/src/routes/notifications.ts`
- Delete: `simse-api/src/middleware/auth.ts`
- Delete: `simse-api/src/lib/password.ts`
- Delete: `simse-api/src/lib/token.ts`
- Delete: `simse-api/src/lib/session.ts`
- Delete: `simse-api/src/lib/api-key.ts`
- Delete: `simse-api/src/lib/email.ts`
- Delete: `simse-api/src/lib/db.ts`
- Delete: `simse-api/src/schemas.ts`
- Modify: `simse-api/wrangler.toml`

**Step 1: Update src/types.ts**

```typescript
export interface Env {
	AUTH_API_URL: string;
	AUTH_API_SECRET: string;
	PAYMENTS_API_URL: string;
	PAYMENTS_API_SECRET: string;
	MAILER_API_URL: string;
	MAILER_API_SECRET: string;
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

**Step 2: Rewrite src/routes/gateway.ts**

```typescript
import { Hono } from 'hono';
import type { Env, ValidateResponse } from '../types';

const gateway = new Hono<{ Bindings: Env }>();

// Public auth routes — proxy directly without validation
const PUBLIC_AUTH_PATHS = ['/register', '/login', '/2fa', '/reset-password', '/new-password', '/verify-email'];

gateway.all('/auth/*', async (c) => {
	const subpath = c.req.path.replace('/auth', '');
	const isPublic = PUBLIC_AUTH_PATHS.some((p) => subpath === p);

	const headers = new Headers();
	headers.set('Content-Type', 'application/json');

	// For protected auth routes, validate first
	if (!isPublic) {
		const auth = await validateToken(c);
		if (!auth) {
			return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
		}
		headers.set('X-User-Id', auth.userId);
		if (auth.sessionId) headers.set('X-Session-Id', auth.sessionId);
		if (auth.teamId) headers.set('X-Team-Id', auth.teamId);
		if (auth.role) headers.set('X-Role', auth.role);
	}

	return proxyTo(c, `${c.env.AUTH_API_URL}${c.req.path}`, headers);
});

// Protected service routes
for (const prefix of ['/users', '/teams', '/api-keys']) {
	gateway.all(`${prefix}/*`, async (c) => {
		const auth = await validateToken(c);
		if (!auth) {
			return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
		}

		const headers = serviceHeaders(auth);
		return proxyTo(c, `${c.env.AUTH_API_URL}${c.req.path}`, headers);
	});
}

// Payments proxy
gateway.all('/payments/*', async (c) => {
	const auth = await validateToken(c);
	if (!auth) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	const path = c.req.path.replace('/payments', '');
	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.env.PAYMENTS_API_SECRET}`);
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);
	if (auth.teamId) headers.set('X-Team-Id', auth.teamId);

	return proxyTo(c, `${c.env.PAYMENTS_API_URL}${path}`, headers);
});

// Notifications proxy (to mailer)
gateway.all('/notifications', async (c) => {
	return proxyNotifications(c);
});
gateway.all('/notifications/*', async (c) => {
	return proxyNotifications(c);
});

async function proxyNotifications(c: any) {
	const auth = await validateToken(c);
	if (!auth) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } }, 401);
	}

	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.env.MAILER_API_SECRET}`);
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);

	return proxyTo(c, `${c.env.MAILER_API_URL}${c.req.path}`, headers);
}

// --- Helpers ---

async function validateToken(c: any): Promise<ValidateResponse['data'] | null> {
	const authHeader = c.req.header('Authorization');
	if (!authHeader?.startsWith('Bearer ')) return null;

	const token = authHeader.slice(7);

	const res = await fetch(`${c.env.AUTH_API_URL}/auth/validate`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	});

	if (!res.ok) return null;

	const json = await res.json() as ValidateResponse;
	return json.data;
}

function serviceHeaders(auth: ValidateResponse['data']): Headers {
	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);
	if (auth.sessionId) headers.set('X-Session-Id', auth.sessionId);
	if (auth.teamId) headers.set('X-Team-Id', auth.teamId);
	if (auth.role) headers.set('X-Role', auth.role);
	return headers;
}

async function proxyTo(c: any, url: string, headers: Headers): Promise<Response> {
	const init: RequestInit = {
		method: c.req.method,
		headers,
	};

	if (!['GET', 'HEAD'].includes(c.req.method)) {
		init.body = await c.req.text();
	}

	const res = await fetch(url, init);
	const body = await res.text();

	return new Response(body, {
		status: res.status,
		headers: { 'Content-Type': 'application/json' },
	});
}

export default gateway;
```

**Step 3: Rewrite src/index.ts**

```typescript
import { Hono } from 'hono';
import gateway from './routes/gateway';
import type { Env } from './types';

const app = new Hono<{ Bindings: Env }>();

app.get('/health', (c) => c.json({ ok: true }));
app.route('', gateway);

export default app;
```

**Step 4: Delete old files**

Delete these files:
- `simse-api/src/routes/auth.ts`
- `simse-api/src/routes/users.ts`
- `simse-api/src/routes/teams.ts`
- `simse-api/src/routes/api-keys.ts`
- `simse-api/src/routes/notifications.ts`
- `simse-api/src/middleware/auth.ts`
- `simse-api/src/lib/password.ts`
- `simse-api/src/lib/token.ts`
- `simse-api/src/lib/session.ts`
- `simse-api/src/lib/api-key.ts`
- `simse-api/src/lib/email.ts`
- `simse-api/src/lib/db.ts`
- `simse-api/src/schemas.ts`

**Step 5: Update wrangler.toml**

```toml
name = "simse-api"
compatibility_date = "2025-04-01"
main = "src/index.ts"

workers_dev = true

routes = [
  { pattern = "api.simse.dev", custom_domain = true }
]

# No database — pure gateway

# Secrets (set via `wrangler secret put`):
# AUTH_API_URL
# AUTH_API_SECRET
# PAYMENTS_API_URL
# PAYMENTS_API_SECRET
# MAILER_API_URL
# MAILER_API_SECRET
```

**Step 6: Verify build**

Run: `cd simse-api && bun run build`
Expected: Build succeeds

**Step 7: Commit**

```
refactor(simse-api): convert to gateway proxy, remove auth/user/team logic
```

---

### Task 9: Update simse-cloud to call simse-api

**Files:**
- Create: `simse-cloud/app/lib/api.server.ts`
- Modify: `simse-cloud/app/lib/session.server.ts`
- Modify: `simse-cloud/app/routes/auth.login.tsx` (action only)
- Modify: `simse-cloud/app/routes/auth.register.tsx` (action only)
- Modify: `simse-cloud/app/routes/auth.2fa.tsx` (action only)
- Modify: `simse-cloud/app/routes/auth.logout.tsx` (action only)
- Modify: `simse-cloud/app/routes/auth.reset-password.tsx` (action only)
- Modify: `simse-cloud/app/routes/auth.new-password.tsx` (action only)
- Modify: `simse-cloud/app/routes/dashboard.account.tsx` (action only)
- Modify: `simse-cloud/app/routes/dashboard.team.tsx` (loader + action)
- Modify: `simse-cloud/app/routes/dashboard.team.invite.tsx` (action only)
- Modify: `simse-cloud/app/routes/dashboard.billing.tsx` (loader + action)
- Modify: `simse-cloud/app/routes/dashboard.billing.credit.tsx` (loader only)
- Modify: `simse-cloud/app/routes/dashboard.usage.tsx` (loader only)
- Modify: `simse-cloud/app/routes/dashboard.notifications.tsx` (loader + action)
- Modify: `simse-cloud/app/routes/dashboard._index.tsx` (loader only)
- Modify: `simse-cloud/app/routes/dashboard.tsx` (loader only)

**Note:** This task only rewrites server-side loaders/actions. UI components stay unchanged. Read each route file before modifying — only change the loader/action functions, not the React components.

**Step 1: Create app/lib/api.server.ts**

```typescript
const API_URL = 'https://api.simse.dev';

export async function api(path: string, options?: RequestInit & { token?: string }): Promise<Response> {
	const headers = new Headers(options?.headers);
	headers.set('Content-Type', 'application/json');
	if (options?.token) {
		headers.set('Authorization', `Bearer ${options.token}`);
	}

	return fetch(`${API_URL}${path}`, {
		...options,
		headers,
	});
}

export async function authenticatedApi(request: Request, path: string, options?: RequestInit): Promise<Response> {
	const token = getTokenFromCookie(request);
	if (!token) {
		throw new Response(null, { status: 302, headers: { Location: '/auth/login' } });
	}
	return api(path, { ...options, token });
}

function getTokenFromCookie(request: Request): string | null {
	const cookie = request.headers.get('Cookie');
	if (!cookie) return null;
	const match = cookie.match(/simse_session=([^;]+)/);
	return match?.[1] ?? null;
}
```

**Step 2: Simplify app/lib/session.server.ts**

```typescript
const COOKIE_NAME = 'simse_session';

export function setSessionCookie(sessionToken: string): string {
	return `${COOKIE_NAME}=${sessionToken}; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=${30 * 24 * 60 * 60}`;
}

export function clearSessionCookie(): string {
	return `${COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Secure; Max-Age=0`;
}
```

**Step 3: Rewrite each route's action/loader**

For each route file listed above: read the file first, then replace the loader/action with API calls using `api()` or `authenticatedApi()`. Keep all React component code unchanged.

Example pattern for auth routes:
```typescript
// Before (direct DB):
const user = await env.DB.prepare('SELECT ...').bind(...).first();

// After (API call):
const res = await api('/auth/login', {
  method: 'POST',
  body: JSON.stringify({ email, password }),
});
const json = await res.json();
```

Example pattern for dashboard routes:
```typescript
// Before (direct DB):
const user = await getSession(request, env);
const data = await env.DB.prepare('SELECT ...').bind(user.userId).first();

// After (API call):
const res = await authenticatedApi(request, '/auth/me');
const json = await res.json();
```

**Step 4: Commit**

```
refactor(simse-cloud): replace direct DB queries with API calls
```

---

### Task 10: Delete dead code from simse-cloud

**Files:**
- Delete: `simse-cloud/app/lib/auth.server.ts`
- Delete: `simse-cloud/app/lib/db.server.ts`
- Delete: `simse-cloud/app/lib/security.server.ts`
- Delete: `simse-cloud/app/lib/payments.server.ts`
- Delete: `simse-cloud/app/lib/email.server.ts`
- Delete: `simse-cloud/app/lib/schemas.ts`
- Modify: `simse-cloud/wrangler.toml` — remove D1 binding, KV binding, payment/email secrets

**Step 1: Delete files**

Delete all 6 files listed above.

**Step 2: Update wrangler.toml**

```toml
name = "simse-cloud"
compatibility_date = "2025-04-01"
pages_build_output_dir = "./build/client"

[vars]
APP_URL = "https://app.simse.dev"

routes = [{ pattern = "app.simse.dev", custom_domain = true }]

# No database — calls simse-api
# No secrets — simse-api handles service auth
```

**Step 3: Verify build**

Run: `cd simse-cloud && bun run build`
Expected: Build succeeds with no imports of deleted files

**Step 4: Commit**

```
refactor(simse-cloud): remove dead server-side code, drop D1/KV bindings
```

---

## Slice 2: Communications (simse-mailer expansion)

### Task 11: Add email templates to simse-mailer

**Files:**
- Copy: all files from `simse-cloud/app/emails/` to `simse-mailer/src/emails/`
- Modify: `simse-mailer/src/index.ts`
- Modify: `simse-mailer/package.json` (add @react-email dependencies)

**Step 1: Add @react-email dependencies to simse-mailer**

```json
"dependencies": {
  "hono": "^4.7.0",
  "@react-email/components": "latest",
  "@react-email/render": "latest",
  "react": "^19.0.0"
}
```

**Step 2: Copy email templates**

Copy entire `simse-cloud/app/emails/` directory to `simse-mailer/src/emails/`.

Also copy the 3 waitlist templates from `simse-landing/functions/emails/` to `simse-mailer/src/emails/`:
- `welcome.tsx` → `simse-mailer/src/emails/waitlist-welcome.tsx`
- `early-preview.tsx` → `simse-mailer/src/emails/waitlist-early-preview.tsx`
- `invite.tsx` → `simse-mailer/src/emails/waitlist-invite.tsx`
- `simse-logo.tsx` → `simse-mailer/src/emails/simse-logo.tsx` (shared component)
- `tailwind-config.ts` → `simse-mailer/src/emails/tailwind-config.ts` (shared config)

**Step 3: Update POST /send endpoint**

Change from accepting raw HTML to accepting `{ template, to, props }`:

```typescript
app.post('/send', async (c) => {
	const authHeader = c.req.header('Authorization');
	if (authHeader !== `Bearer ${c.env.API_SECRET}`) {
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

	await sendEmail(c.env.RESEND_API_KEY, { to: body.to, subject, html });
	return c.json({ success: true });
});
```

**Step 4: Create template renderer** (`src/render.ts`)

Import the template registry from the copied email templates, look up by name, render to HTML, extract subject.

**Step 5: Commit**

```
feat(simse-mailer): add email template rendering with React Email
```

---

### Task 12: Add notifications to simse-mailer

**Files:**
- Create: `simse-mailer/migrations/0001_notifications.sql`
- Create: `simse-mailer/src/routes/notifications.ts`
- Modify: `simse-mailer/src/index.ts`
- Modify: `simse-mailer/wrangler.toml` (add D1 binding)

**Step 1: Create migration**

```sql
CREATE TABLE notifications (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  type TEXT NOT NULL,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  read INTEGER DEFAULT 0,
  link TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_notifications_user ON notifications(user_id, read);
```

**Step 2: Create notification routes**

```typescript
// GET /notifications/:userId — list (last 100)
// POST /notifications — create
// PUT /notifications/:id/read — mark read
// PUT /notifications/:userId/read-all — mark all read
```

Auth: check `Authorization: Bearer <API_SECRET>` or `X-User-Id` from gateway.

**Step 3: Wire routes and update wrangler.toml with D1 binding**

**Step 4: Commit**

```
feat(simse-mailer): add notification storage and endpoints
```

---

### Task 13: Delete email templates from simse-cloud

**Files:**
- Delete: `simse-cloud/app/emails/` (entire directory)

**Step 1: Delete the directory**

**Step 2: Remove any remaining imports of email templates from route files**

**Step 3: Verify build**

Run: `cd simse-cloud && bun run build`

**Step 4: Commit**

```
refactor(simse-cloud): remove email templates (moved to simse-mailer)
```

---

## Slice 3: Payments + Cleanup

### Task 14: Verify payments proxy works through gateway

**Step 1: Check simse-cloud billing routes use `authenticatedApi()`**

All billing routes should already call `/payments/*` via `authenticatedApi()` from Task 9. Verify no direct calls to `PAYMENTS_API_URL` remain.

**Step 2: Grep for leftover direct service calls**

Run grep across simse-cloud for `PAYMENTS_API`, `EMAIL_API`, `env.DB` — should find zero matches.

**Step 3: Commit (if any fixes needed)**

```
fix(simse-cloud): remove remaining direct service calls
```

---

### Task 15: Remove notifications table from simse-api-db

**Files:**
- Create: `simse-api/migrations/0002_remove_all_tables.sql`

**Step 1: Create migration to drop all tables**

Since simse-api is now a stateless gateway, drop all tables from simse-api-db. This should only run after data migration to simse-auth-db is complete.

```sql
-- Only run after data is migrated to simse-auth-db and simse-mailer-db
DROP TABLE IF EXISTS api_keys;
DROP TABLE IF EXISTS team_invites;
DROP TABLE IF EXISTS team_members;
DROP TABLE IF EXISTS teams;
DROP TABLE IF EXISTS tokens;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS notifications;
DROP TABLE IF EXISTS users;
```

**Step 2: Commit**

```
chore(simse-api): drop tables from gateway db (data migrated to services)
```

---

### Task 16: Final verification

**Step 1: Build all services**

```bash
cd simse-auth && bun run build
cd simse-api && bun run build
cd simse-mailer && bun run build
cd simse-cloud && bun run build
```

All should succeed.

**Step 2: Grep for dead references**

Search entire repo for:
- `env.DB` in simse-cloud (should be zero)
- `env.DB` in simse-api (should be zero)
- `PAYMENTS_API_URL` in simse-cloud (should be zero)
- `EMAIL_API_URL` in simse-cloud (should be zero)
- Imports of deleted files

**Step 3: Final commit**

```
chore: complete service extraction (auth, mailer, gateway)
```
