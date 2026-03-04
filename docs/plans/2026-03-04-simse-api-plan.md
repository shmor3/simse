# simse-api Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a central API gateway (simse-api) that owns auth, users, teams, notifications, and proxies to simse-payments and simse-mailer.

**Architecture:** Cloudflare Worker (Hono) with own D1 database. Dual auth (session tokens for web, API keys for CLI). Gateway proxy to backend services.

**Tech Stack:** Hono, Zod, Cloudflare Workers + D1, TypeScript, Biome

---

### Task 1: Scaffold simse-api project

**Files:**
- Create: `simse-api/package.json`
- Create: `simse-api/tsconfig.json`
- Create: `simse-api/biome.json`
- Create: `simse-api/wrangler.toml`
- Create: `simse-api/moon.yml`
- Create: `simse-api/.gitignore`

**Step 1: Create package.json**

```json
{
	"name": "simse-api",
	"private": true,
	"type": "module",
	"scripts": {
		"dev": "wrangler dev",
		"build": "wrangler deploy --dry-run --outdir dist",
		"deploy": "wrangler deploy",
		"lint": "biome check .",
		"lint:fix": "biome check --write .",
		"db:migrate": "wrangler d1 migrations apply simse-api-db --local",
		"db:migrate:prod": "wrangler d1 migrations apply simse-api-db --remote"
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

**Step 2: Create tsconfig.json**

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

**Step 3: Create biome.json** (same as simse-payments)

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
name = "simse-api"
compatibility_date = "2025-04-01"
main = "src/index.ts"

[[d1_databases]]
binding = "DB"
database_name = "simse-api-db"
database_id = "placeholder-create-via-wrangler"

# Secrets (set via `wrangler secret put`):
# SESSION_SECRET
# PAYMENTS_API_URL
# PAYMENTS_API_SECRET
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

**Step 7: Install deps**

Run: `cd simse-api && bun install`

**Step 8: Commit**

```bash
git add simse-api/
git commit -m "feat(simse-api): scaffold cloudflare worker project"
```

---

### Task 2: D1 migration

**Files:**
- Create: `simse-api/migrations/0001_initial.sql`

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

-- Notifications
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
CREATE INDEX idx_notifications_user ON notifications(user_id, read);
CREATE INDEX idx_api_keys_user ON api_keys(user_id);
CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
```

**Step 2: Commit**

```bash
git add simse-api/migrations/
git commit -m "feat(simse-api): add D1 schema migration"
```

---

### Task 3: Types, DB helpers, and schemas

**Files:**
- Create: `simse-api/src/types.ts`
- Create: `simse-api/src/lib/db.ts`
- Create: `simse-api/src/schemas.ts`

**Step 1: Create types.ts**

```typescript
export interface Env {
	DB: D1Database;
	SESSION_SECRET: string;
	PAYMENTS_API_URL: string;
	PAYMENTS_API_SECRET: string;
	MAILER_API_URL: string;
	MAILER_API_SECRET: string;
}

export interface AuthContext {
	userId: string;
	sessionId?: string;
}
```

**Step 2: Create db.ts**

```typescript
export function generateId(): string {
	return crypto.randomUUID();
}
```

**Step 3: Create schemas.ts**

```typescript
import { z } from 'zod/v4';

export const registerSchema = z.object({
	name: z.string().min(2),
	email: z.email(),
	password: z.string().min(8),
});

export const loginSchema = z.object({
	email: z.email(),
	password: z.string(),
});

export const resetPasswordSchema = z.object({
	email: z.email(),
});

export const newPasswordSchema = z.object({
	token: z.string(),
	password: z.string().min(8),
});

export const twoFactorSchema = z.object({
	code: z.string().length(6),
	pendingToken: z.string(),
});

export const inviteSchema = z.object({
	email: z.email(),
	role: z.enum(['admin', 'member']),
});

export const updateNameSchema = z.object({
	name: z.string().min(2),
});

export const changePasswordSchema = z.object({
	currentPassword: z.string(),
	newPassword: z.string().min(8),
});

export const deleteAccountSchema = z.object({
	confirmEmail: z.string(),
});

export const createApiKeySchema = z.object({
	name: z.string().min(1).max(64),
});
```

**Step 4: Commit**

```bash
git add simse-api/src/
git commit -m "feat(simse-api): add types, db helpers, and zod schemas"
```

---

### Task 4: Password and token helpers

**Files:**
- Create: `simse-api/src/lib/password.ts`
- Create: `simse-api/src/lib/token.ts`
- Create: `simse-api/src/lib/session.ts`

**Step 1: Create password.ts** (moved from simse-cloud auth.server.ts)

```typescript
export async function hashPassword(password: string): Promise<string> {
	const salt = crypto.getRandomValues(new Uint8Array(16));
	const key = await deriveKey(password, salt);
	const hash = await crypto.subtle.exportKey('raw', key);
	const hashArray = new Uint8Array(hash);

	const saltB64 = btoa(String.fromCharCode(...salt));
	const hashB64 = btoa(String.fromCharCode(...hashArray));
	return `${saltB64}:${hashB64}`;
}

export async function verifyPassword(
	password: string,
	stored: string,
): Promise<boolean> {
	const [saltB64, hashB64] = stored.split(':');
	const salt = Uint8Array.from(atob(saltB64), (c) => c.charCodeAt(0));
	const storedHash = Uint8Array.from(atob(hashB64), (c) => c.charCodeAt(0));

	const key = await deriveKey(password, salt);
	const hash = await crypto.subtle.exportKey('raw', key);
	const hashArray = new Uint8Array(hash);

	if (hashArray.length !== storedHash.length) return false;
	let diff = 0;
	for (let i = 0; i < hashArray.length; i++) {
		diff |= hashArray[i] ^ storedHash[i];
	}
	return diff === 0;
}

async function deriveKey(
	password: string,
	salt: Uint8Array,
): Promise<CryptoKey> {
	const enc = new TextEncoder();
	const keyMaterial = await crypto.subtle.importKey(
		'raw',
		enc.encode(password),
		'PBKDF2',
		false,
		['deriveBits', 'deriveKey'],
	);
	return crypto.subtle.deriveKey(
		{ name: 'PBKDF2', salt, iterations: 100_000, hash: 'SHA-256' },
		keyMaterial,
		{ name: 'AES-GCM', length: 256 },
		true,
		['encrypt'],
	);
}
```

**Step 2: Create token.ts**

```typescript
import { generateId } from './db';

export function generateCode(): string {
	const array = new Uint32Array(1);
	crypto.getRandomValues(array);
	return String(array[0] % 1_000_000).padStart(6, '0');
}

export async function createToken(
	db: D1Database,
	userId: string,
	type: string,
	expiresInMinutes: number,
): Promise<{ id: string; code: string }> {
	const id = generateId();
	const code = generateCode();
	const expiresAt = new Date(
		Date.now() + expiresInMinutes * 60 * 1000,
	).toISOString();

	await db
		.prepare(
			'INSERT INTO tokens (id, user_id, type, code, expires_at) VALUES (?, ?, ?, ?, ?)',
		)
		.bind(id, userId, type, code, expiresAt)
		.run();

	return { id, code };
}

export async function validateToken(
	db: D1Database,
	code: string,
	type: string,
): Promise<{ id: string; userId: string } | null> {
	const token = await db
		.prepare(
			"SELECT id, user_id FROM tokens WHERE code = ? AND type = ? AND used = 0 AND expires_at > datetime('now')",
		)
		.bind(code, type)
		.first<{ id: string; user_id: string }>();

	if (!token) return null;
	return { id: token.id, userId: token.user_id };
}

export async function markTokenUsed(
	db: D1Database,
	id: string,
): Promise<void> {
	await db
		.prepare('UPDATE tokens SET used = 1 WHERE id = ?')
		.bind(id)
		.run();
}
```

**Step 3: Create session.ts**

```typescript
import { generateId } from './db';

const SESSION_TTL_DAYS = 30;

export async function createSession(
	db: D1Database,
	userId: string,
): Promise<string> {
	const id = `session_${generateId()}`;
	const expiresAt = new Date(
		Date.now() + SESSION_TTL_DAYS * 24 * 60 * 60 * 1000,
	).toISOString();

	await db
		.prepare(
			'INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, ?)',
		)
		.bind(id, userId, expiresAt)
		.run();

	return id;
}

export async function validateSession(
	db: D1Database,
	sessionId: string,
): Promise<string | null> {
	const session = await db
		.prepare(
			"SELECT user_id FROM sessions WHERE id = ? AND expires_at > datetime('now')",
		)
		.bind(sessionId)
		.first<{ user_id: string }>();

	return session?.user_id ?? null;
}

export async function deleteSession(
	db: D1Database,
	sessionId: string,
): Promise<void> {
	await db
		.prepare('DELETE FROM sessions WHERE id = ?')
		.bind(sessionId)
		.run();
}
```

**Step 4: Commit**

```bash
git add simse-api/src/
git commit -m "feat(simse-api): add password, token, and session helpers"
```

---

### Task 5: API key helper

**Files:**
- Create: `simse-api/src/lib/api-key.ts`

**Step 1: Create api-key.ts**

```typescript
import { generateId } from './db';

export async function createApiKey(
	db: D1Database,
	userId: string,
	name: string,
): Promise<{ id: string; key: string; prefix: string }> {
	const id = generateId();
	const rawKey = `sk_${generateId().replace(/-/g, '')}`;
	const prefix = rawKey.slice(0, 7);

	// Hash the key for storage
	const encoder = new TextEncoder();
	const data = encoder.encode(rawKey);
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = new Uint8Array(hashBuffer);
	const keyHash = btoa(String.fromCharCode(...hashArray));

	await db
		.prepare(
			'INSERT INTO api_keys (id, user_id, name, key_hash, key_prefix) VALUES (?, ?, ?, ?, ?)',
		)
		.bind(id, userId, name, keyHash, prefix)
		.run();

	return { id, key: rawKey, prefix };
}

export async function validateApiKey(
	db: D1Database,
	rawKey: string,
): Promise<string | null> {
	const encoder = new TextEncoder();
	const data = encoder.encode(rawKey);
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = new Uint8Array(hashBuffer);
	const keyHash = btoa(String.fromCharCode(...hashArray));

	const row = await db
		.prepare('SELECT user_id FROM api_keys WHERE key_hash = ?')
		.bind(keyHash)
		.first<{ user_id: string }>();

	if (!row) return null;

	// Update last_used_at
	await db
		.prepare(
			"UPDATE api_keys SET last_used_at = datetime('now') WHERE key_hash = ?",
		)
		.bind(keyHash)
		.run();

	return row.user_id;
}
```

**Step 2: Commit**

```bash
git add simse-api/src/lib/api-key.ts
git commit -m "feat(simse-api): add API key creation and validation"
```

---

### Task 6: Email helper

**Files:**
- Create: `simse-api/src/lib/email.ts`

**Step 1: Create email.ts**

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

**Step 2: Commit**

```bash
git add simse-api/src/lib/email.ts
git commit -m "feat(simse-api): add email helper"
```

---

### Task 7: Auth middleware

**Files:**
- Create: `simse-api/src/middleware/auth.ts`

**Step 1: Create auth.ts**

```typescript
import type { Context, Next } from 'hono';
import { validateApiKey } from '../lib/api-key';
import { validateSession } from '../lib/session';
import type { AuthContext, Env } from '../types';

export async function authMiddleware(
	c: Context<{ Bindings: Env; Variables: { auth: AuthContext } }>,
	next: Next,
) {
	const authHeader = c.req.header('Authorization');
	if (!authHeader?.startsWith('Bearer ')) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Missing authorization' } }, 401);
	}

	const token = authHeader.slice(7);
	const db = c.env.DB;

	if (token.startsWith('session_')) {
		const userId = await validateSession(db, token);
		if (!userId) {
			return c.json({ error: { code: 'SESSION_EXPIRED', message: 'Session expired or invalid' } }, 401);
		}
		c.set('auth', { userId, sessionId: token });
	} else if (token.startsWith('sk_')) {
		const userId = await validateApiKey(db, token);
		if (!userId) {
			return c.json({ error: { code: 'INVALID_API_KEY', message: 'Invalid API key' } }, 401);
		}
		c.set('auth', { userId });
	} else {
		return c.json({ error: { code: 'INVALID_TOKEN', message: 'Unrecognized token format' } }, 401);
	}

	await next();
}
```

**Step 2: Commit**

```bash
git add simse-api/src/middleware/
git commit -m "feat(simse-api): add dual auth middleware (session + API key)"
```

---

### Task 8: Auth routes (register, login, logout, me)

**Files:**
- Create: `simse-api/src/routes/auth.ts`

**Step 1: Create auth.ts**

```typescript
import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { hashPassword, verifyPassword } from '../lib/password';
import { createSession, deleteSession } from '../lib/session';
import { createToken, markTokenUsed, validateToken } from '../lib/token';
import { loginSchema, newPasswordSchema, registerSchema, resetPasswordSchema, twoFactorSchema } from '../schemas';
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

	// Check email uniqueness
	const existing = await db
		.prepare('SELECT id FROM users WHERE LOWER(email) = ?')
		.bind(normalizedEmail)
		.first();

	if (existing) {
		return c.json({ error: { code: 'EMAIL_EXISTS', message: 'Email already registered' } }, 409);
	}

	const userId = generateId();
	const passwordHash = await hashPassword(password);

	// Create user
	await db
		.prepare(
			'INSERT INTO users (id, email, name, password_hash) VALUES (?, ?, ?, ?)',
		)
		.bind(userId, normalizedEmail, name, passwordHash)
		.run();

	// Create default team
	const teamId = generateId();
	await db
		.prepare('INSERT INTO teams (id, name) VALUES (?, ?)')
		.bind(teamId, `${name}'s Team`)
		.run();

	await db
		.prepare(
			"INSERT INTO team_members (team_id, user_id, role) VALUES (?, ?, 'owner')",
		)
		.bind(teamId, userId)
		.run();

	// Create verification token
	await createToken(db, userId, 'email_verify', 15);

	// Create session
	const token = await createSession(db, userId);

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
		.prepare(
			'SELECT id, password_hash, two_factor_enabled FROM users WHERE LOWER(email) = ?',
		)
		.bind(email.toLowerCase())
		.first<{
			id: string;
			password_hash: string;
			two_factor_enabled: number;
		}>();

	if (!user || !(await verifyPassword(password, user.password_hash))) {
		return c.json({ error: { code: 'INVALID_CREDENTIALS', message: 'Invalid email or password' } }, 401);
	}

	// 2FA flow
	if (user.two_factor_enabled) {
		const { id } = await createToken(db, user.id, '2fa', 10);
		return c.json({ data: { requires2fa: true, pendingToken: id } });
	}

	const token = await createSession(db, user.id);

	return c.json({
		data: {
			token,
			user: { id: user.id },
		},
	});
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

	// Validate pending token
	const pending = await db
		.prepare(
			"SELECT user_id FROM tokens WHERE id = ? AND type = '2fa' AND used = 0 AND expires_at > datetime('now')",
		)
		.bind(pendingToken)
		.first<{ user_id: string }>();

	if (!pending) {
		return c.json({ error: { code: 'INVALID_TOKEN', message: '2FA session expired' } }, 401);
	}

	// Validate code
	const codeToken = await validateToken(db, code, '2fa');
	if (!codeToken || codeToken.userId !== pending.user_id) {
		return c.json({ error: { code: 'INVALID_CODE', message: 'Invalid 2FA code' } }, 401);
	}

	// Mark tokens used
	await markTokenUsed(db, pendingToken);
	await markTokenUsed(db, codeToken.id);

	const token = await createSession(db, pending.user_id);

	return c.json({ data: { token, user: { id: pending.user_id } } });
});

// POST /auth/logout (requires auth)
auth.post('/logout', async (c) => {
	const auth = c.get('auth');
	if (!auth?.sessionId) {
		return c.json({ data: { ok: true } });
	}

	await deleteSession(c.env.DB, auth.sessionId);
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
		.prepare('SELECT id FROM users WHERE LOWER(email) = ?')
		.bind(parsed.data.email.toLowerCase())
		.first<{ id: string }>();

	if (user) {
		await createToken(db, user.id, 'password_reset', 60);
		// TODO: send email with reset code
	}

	// Always return success to prevent email enumeration
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
	await db
		.prepare('UPDATE users SET password_hash = ? WHERE id = ?')
		.bind(passwordHash, token.userId)
		.run();

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

	await db
		.prepare('UPDATE users SET email_verified = 1 WHERE id = ?')
		.bind(token.userId)
		.run();

	await markTokenUsed(db, token.id);

	return c.json({ data: { ok: true } });
});

// GET /auth/me (requires auth)
auth.get('/me', async (c) => {
	const auth = c.get('auth');
	if (!auth) {
		return c.json({ error: { code: 'UNAUTHORIZED', message: 'Not authenticated' } }, 401);
	}

	const db = c.env.DB;
	const user = await db
		.prepare(
			'SELECT id, email, name, email_verified, two_factor_enabled, created_at FROM users WHERE id = ?',
		)
		.bind(auth.userId)
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
		.prepare(
			"SELECT t.id, t.name, t.plan, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1",
		)
		.bind(auth.userId)
		.first<{ id: string; name: string; plan: string; role: string }>();

	return c.json({
		data: {
			id: user.id,
			email: user.email,
			name: user.name,
			emailVerified: !!user.email_verified,
			twoFactorEnabled: !!user.two_factor_enabled,
			createdAt: user.created_at,
			team: team
				? { id: team.id, name: team.name, plan: team.plan, role: team.role }
				: null,
		},
	});
});

export default auth;
```

**Step 2: Commit**

```bash
git add simse-api/src/
git commit -m "feat(simse-api): add auth routes (register, login, logout, 2fa, reset, verify, me)"
```

---

### Task 9: User routes

**Files:**
- Create: `simse-api/src/routes/users.ts`

**Step 1: Create users.ts**

```typescript
import { Hono } from 'hono';
import { hashPassword, verifyPassword } from '../lib/password';
import { changePasswordSchema, deleteAccountSchema, updateNameSchema } from '../schemas';
import type { AuthContext, Env } from '../types';

const users = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// PUT /users/me/name
users.put('/me/name', async (c) => {
	const auth = c.get('auth');
	const body = await c.req.json();
	const parsed = updateNameSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	await c.env.DB
		.prepare('UPDATE users SET name = ? WHERE id = ?')
		.bind(parsed.data.name, auth.userId)
		.run();

	return c.json({ data: { ok: true } });
});

// PUT /users/me/password
users.put('/me/password', async (c) => {
	const auth = c.get('auth');
	const body = await c.req.json();
	const parsed = changePasswordSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const user = await db
		.prepare('SELECT password_hash FROM users WHERE id = ?')
		.bind(auth.userId)
		.first<{ password_hash: string }>();

	if (!user || !(await verifyPassword(parsed.data.currentPassword, user.password_hash))) {
		return c.json({ error: { code: 'INVALID_PASSWORD', message: 'Current password is incorrect' } }, 400);
	}

	const newHash = await hashPassword(parsed.data.newPassword);
	await db
		.prepare('UPDATE users SET password_hash = ? WHERE id = ?')
		.bind(newHash, auth.userId)
		.run();

	return c.json({ data: { ok: true } });
});

// DELETE /users/me
users.delete('/me', async (c) => {
	const auth = c.get('auth');
	const body = await c.req.json();
	const parsed = deleteAccountSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const user = await db
		.prepare('SELECT email FROM users WHERE id = ?')
		.bind(auth.userId)
		.first<{ email: string }>();

	if (!user || parsed.data.confirmEmail.toLowerCase() !== user.email.toLowerCase()) {
		return c.json({ error: { code: 'EMAIL_MISMATCH', message: 'Email does not match' } }, 400);
	}

	// Cascade delete
	await db.prepare('DELETE FROM sessions WHERE user_id = ?').bind(auth.userId).run();
	await db.prepare('DELETE FROM tokens WHERE user_id = ?').bind(auth.userId).run();
	await db.prepare('DELETE FROM notifications WHERE user_id = ?').bind(auth.userId).run();
	await db.prepare('DELETE FROM api_keys WHERE user_id = ?').bind(auth.userId).run();
	await db.prepare('DELETE FROM team_members WHERE user_id = ?').bind(auth.userId).run();
	await db.prepare('DELETE FROM users WHERE id = ?').bind(auth.userId).run();

	return c.json({ data: { ok: true } });
});

export default users;
```

**Step 2: Commit**

```bash
git add simse-api/src/routes/users.ts
git commit -m "feat(simse-api): add user routes (update name, change password, delete)"
```

---

### Task 10: Team routes

**Files:**
- Create: `simse-api/src/routes/teams.ts`

**Step 1: Create teams.ts**

```typescript
import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { inviteSchema } from '../schemas';
import type { AuthContext, Env } from '../types';

const teams = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// GET /teams/me — team info + members + invites
teams.get('/me', async (c) => {
	const auth = c.get('auth');
	const db = c.env.DB;

	const team = await db
		.prepare(
			'SELECT t.id, t.name, t.plan FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1',
		)
		.bind(auth.userId)
		.first<{ id: string; name: string; plan: string }>();

	if (!team) {
		return c.json({ error: { code: 'NOT_FOUND', message: 'No team found' } }, 404);
	}

	const members = await db
		.prepare(
			'SELECT u.id, u.name, u.email, tm.role, tm.joined_at FROM team_members tm JOIN users u ON tm.user_id = u.id WHERE tm.team_id = ?',
		)
		.bind(team.id)
		.all<{
			id: string;
			name: string;
			email: string;
			role: string;
			joined_at: string;
		}>();

	const invites = await db
		.prepare(
			"SELECT id, email, role, created_at FROM team_invites WHERE team_id = ? AND expires_at > datetime('now')",
		)
		.bind(team.id)
		.all<{
			id: string;
			email: string;
			role: string;
			created_at: string;
		}>();

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
	const auth = c.get('auth');
	const body = await c.req.json();
	const parsed = inviteSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;

	// Check user has owner/admin role
	const membership = await db
		.prepare(
			"SELECT t.id as team_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1",
		)
		.bind(auth.userId)
		.first<{ team_id: string }>();

	if (!membership) {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Only owners and admins can invite' } }, 403);
	}

	// Check if already a member
	const existingMember = await db
		.prepare(
			'SELECT 1 FROM team_members tm JOIN users u ON tm.user_id = u.id WHERE tm.team_id = ? AND LOWER(u.email) = ?',
		)
		.bind(membership.team_id, parsed.data.email.toLowerCase())
		.first();

	if (existingMember) {
		return c.json({ error: { code: 'ALREADY_MEMBER', message: 'User is already a team member' } }, 409);
	}

	const inviteId = generateId();
	const expiresAt = new Date(Date.now() + 7 * 24 * 60 * 60 * 1000).toISOString();

	await db
		.prepare(
			'INSERT INTO team_invites (id, team_id, email, role, invited_by, expires_at) VALUES (?, ?, ?, ?, ?, ?)',
		)
		.bind(inviteId, membership.team_id, parsed.data.email.toLowerCase(), parsed.data.role, auth.userId, expiresAt)
		.run();

	return c.json({ data: { id: inviteId } }, 201);
});

// PUT /teams/me/members/:userId/role
teams.put('/me/members/:userId/role', async (c) => {
	const auth = c.get('auth');
	const targetUserId = c.req.param('userId');
	const body = await c.req.json<{ role: string }>();

	if (!body.role || !['admin', 'member'].includes(body.role)) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: 'Role must be admin or member' } }, 400);
	}

	const db = c.env.DB;

	const membership = await db
		.prepare(
			"SELECT t.id as team_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1",
		)
		.bind(auth.userId)
		.first<{ team_id: string }>();

	if (!membership) {
		return c.json({ error: { code: 'FORBIDDEN', message: 'Insufficient permissions' } }, 403);
	}

	await db
		.prepare('UPDATE team_members SET role = ? WHERE team_id = ? AND user_id = ?')
		.bind(body.role, membership.team_id, targetUserId)
		.run();

	return c.json({ data: { ok: true } });
});

// DELETE /teams/me/invites/:inviteId
teams.delete('/me/invites/:inviteId', async (c) => {
	const auth = c.get('auth');
	const inviteId = c.req.param('inviteId');
	const db = c.env.DB;

	const membership = await db
		.prepare(
			"SELECT t.id as team_id FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? AND tm.role IN ('owner', 'admin') LIMIT 1",
		)
		.bind(auth.userId)
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

**Step 2: Commit**

```bash
git add simse-api/src/routes/teams.ts
git commit -m "feat(simse-api): add team routes (info, invite, role change, revoke)"
```

---

### Task 11: Notification routes

**Files:**
- Create: `simse-api/src/routes/notifications.ts`

**Step 1: Create notifications.ts**

```typescript
import { Hono } from 'hono';
import type { AuthContext, Env } from '../types';

const notifications = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// GET /notifications
notifications.get('/', async (c) => {
	const auth = c.get('auth');
	const db = c.env.DB;

	const results = await db
		.prepare(
			'SELECT id, type, title, body, read, link, created_at FROM notifications WHERE user_id = ? ORDER BY created_at DESC LIMIT 100',
		)
		.bind(auth.userId)
		.all<{
			id: string;
			type: string;
			title: string;
			body: string;
			read: number;
			link: string | null;
			created_at: string;
		}>();

	return c.json({ data: results.results });
});

// PUT /notifications/:id/read
notifications.put('/:id/read', async (c) => {
	const auth = c.get('auth');
	const id = c.req.param('id');

	await c.env.DB
		.prepare('UPDATE notifications SET read = 1 WHERE id = ? AND user_id = ?')
		.bind(id, auth.userId)
		.run();

	return c.json({ data: { ok: true } });
});

// PUT /notifications/read-all
notifications.put('/read-all', async (c) => {
	const auth = c.get('auth');

	await c.env.DB
		.prepare('UPDATE notifications SET read = 1 WHERE user_id = ? AND read = 0')
		.bind(auth.userId)
		.run();

	return c.json({ data: { ok: true } });
});

export default notifications;
```

**Step 2: Commit**

```bash
git add simse-api/src/routes/notifications.ts
git commit -m "feat(simse-api): add notification routes"
```

---

### Task 12: API key routes

**Files:**
- Create: `simse-api/src/routes/api-keys.ts`

**Step 1: Create api-keys.ts**

```typescript
import { Hono } from 'hono';
import { createApiKey } from '../lib/api-key';
import { createApiKeySchema } from '../schemas';
import type { AuthContext, Env } from '../types';

const apiKeys = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// POST /api-keys
apiKeys.post('/', async (c) => {
	const auth = c.get('auth');
	const body = await c.req.json();
	const parsed = createApiKeySchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const result = await createApiKey(c.env.DB, auth.userId, parsed.data.name);

	// Return the raw key only once — it's hashed in storage
	return c.json({
		data: {
			id: result.id,
			key: result.key,
			prefix: result.prefix,
			name: parsed.data.name,
		},
	}, 201);
});

// GET /api-keys
apiKeys.get('/', async (c) => {
	const auth = c.get('auth');

	const keys = await c.env.DB
		.prepare(
			'SELECT id, name, key_prefix, last_used_at, created_at FROM api_keys WHERE user_id = ? ORDER BY created_at DESC',
		)
		.bind(auth.userId)
		.all<{
			id: string;
			name: string;
			key_prefix: string;
			last_used_at: string | null;
			created_at: string;
		}>();

	return c.json({ data: keys.results });
});

// DELETE /api-keys/:id
apiKeys.delete('/:id', async (c) => {
	const auth = c.get('auth');
	const id = c.req.param('id');

	await c.env.DB
		.prepare('DELETE FROM api_keys WHERE id = ? AND user_id = ?')
		.bind(id, auth.userId)
		.run();

	return c.json({ data: { ok: true } });
});

export default apiKeys;
```

**Step 2: Commit**

```bash
git add simse-api/src/routes/api-keys.ts
git commit -m "feat(simse-api): add API key management routes"
```

---

### Task 13: Gateway proxy routes

**Files:**
- Create: `simse-api/src/routes/gateway.ts`

**Step 1: Create gateway.ts**

```typescript
import { Hono } from 'hono';
import type { AuthContext, Env } from '../types';

const gateway = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// Proxy to simse-payments: /payments/* → simse-payments/*
gateway.all('/payments/*', async (c) => {
	const path = c.req.path.replace('/payments', '');
	const url = `${c.env.PAYMENTS_API_URL}${path}`;

	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.env.PAYMENTS_API_SECRET}`);
	headers.set('Content-Type', 'application/json');

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
});

// Proxy to simse-mailer: /emails/send → simse-mailer/send
gateway.post('/emails/send', async (c) => {
	const body = await c.req.text();

	const res = await fetch(`${c.env.MAILER_API_URL}/send`, {
		method: 'POST',
		headers: {
			Authorization: `Bearer ${c.env.MAILER_API_SECRET}`,
			'Content-Type': 'application/json',
		},
		body,
	});

	const responseBody = await res.text();

	return new Response(responseBody, {
		status: res.status,
		headers: { 'Content-Type': 'application/json' },
	});
});

export default gateway;
```

**Step 2: Commit**

```bash
git add simse-api/src/routes/gateway.ts
git commit -m "feat(simse-api): add gateway proxy routes (payments + mailer)"
```

---

### Task 14: Final index.ts assembly, lint, build

**Files:**
- Create: `simse-api/src/index.ts`

**Step 1: Create index.ts**

```typescript
import { Hono } from 'hono';
import { authMiddleware } from './middleware/auth';
import apiKeys from './routes/api-keys';
import auth from './routes/auth';
import gateway from './routes/gateway';
import notifications from './routes/notifications';
import teams from './routes/teams';
import users from './routes/users';
import type { AuthContext, Env } from './types';

const app = new Hono<{
	Bindings: Env;
	Variables: { auth: AuthContext };
}>();

// Health check
app.get('/health', (c) => c.json({ ok: true }));

// Public auth routes (no middleware)
app.route('/auth', auth);

// Auth middleware for all protected routes
app.use('/users/*', authMiddleware);
app.use('/teams/*', authMiddleware);
app.use('/notifications', authMiddleware);
app.use('/notifications/*', authMiddleware);
app.use('/api-keys', authMiddleware);
app.use('/api-keys/*', authMiddleware);
app.use('/payments/*', authMiddleware);
app.use('/emails/*', authMiddleware);

// Logout needs auth context
app.use('/auth/logout', authMiddleware);
app.use('/auth/me', authMiddleware);

// Protected routes
app.route('/users', users);
app.route('/teams', teams);
app.route('/notifications', notifications);
app.route('/api-keys', apiKeys);

// Gateway
app.route('', gateway);

export default app;
```

**Step 2: Run lint**

Run: `cd simse-api && bun run lint`
Expected: No errors.

**Step 3: Run build**

Run: `cd simse-api && bun run build`
Expected: Build succeeds.

**Step 4: Commit**

```bash
git add simse-api/src/index.ts
git commit -m "feat(simse-api): assemble all routes in index"
```

---

### Task 15: Create D1 database, deploy, and verify

**Step 1: Create D1 database**

Run: `cd simse-api && CLOUDFLARE_ACCOUNT_ID=61b9dcd1967be6feb3d0e0f5cab64045 npx wrangler d1 create simse-api-db`
Update `wrangler.toml` with the real database_id.

**Step 2: Deploy**

Run: `cd simse-api && CLOUDFLARE_ACCOUNT_ID=61b9dcd1967be6feb3d0e0f5cab64045 bun run deploy`

**Step 3: Run migration**

Run: `cd simse-api && CLOUDFLARE_ACCOUNT_ID=61b9dcd1967be6feb3d0e0f5cab64045 npx wrangler d1 migrations apply simse-api-db --remote`

**Step 4: Verify health**

Run: `curl https://simse-api.<subdomain>.workers.dev/health`
Expected: `{"ok":true}`

**Step 5: Set secrets**

Run:
```bash
cd simse-api
wrangler secret put SESSION_SECRET
wrangler secret put PAYMENTS_API_URL
wrangler secret put PAYMENTS_API_SECRET
wrangler secret put MAILER_API_URL
wrangler secret put MAILER_API_SECRET
```

**Step 6: Commit**

```bash
git add simse-api/wrangler.toml
git commit -m "feat(simse-api): deploy with D1 database"
```
