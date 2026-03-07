# Auth & API Gateway Safety Improvements — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add JWT access tokens with refresh token rotation to auth service, and add circuit breaker, timeout/retry/backoff, rate limiting, request validation, and security hardening to the API gateway.

**Architecture:** Auth service issues short-lived JWT access tokens (15-min, HMAC-SHA256) paired with rotating refresh tokens (30-day). The API gateway validates JWTs locally without calling auth, falls back to HTTP validation for API keys (`sk_*`). Gateway gets per-backend circuit breakers, resilient fetch with timeout/retry/backoff, tiered rate limiting, request validation, correlation IDs, and security headers.

**Tech Stack:** TypeScript, Hono, Cloudflare Workers (D1, Secrets Store, Analytics Engine), WebCrypto API, Zod

---

## Task 1: Auth — Database Migration for Refresh Tokens

**Files:**
- Create: `simse-auth/migrations/0002_refresh_tokens.sql`

**Step 1: Write the migration**

```sql
-- Refresh tokens for JWT rotation
CREATE TABLE refresh_tokens (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL REFERENCES users(id),
  family_id TEXT NOT NULL,
  token_hash TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  revoked INTEGER DEFAULT 0,
  created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX idx_refresh_tokens_user ON refresh_tokens(user_id);
CREATE INDEX idx_refresh_tokens_hash ON refresh_tokens(token_hash);
CREATE INDEX idx_refresh_tokens_family ON refresh_tokens(family_id);
```

**Step 2: Run migration locally**

Run: `cd simse-auth && npm run db:migrate`
Expected: Migration applies successfully.

**Step 3: Commit**

```bash
git add simse-auth/migrations/0002_refresh_tokens.sql
git commit -m "feat(simse-auth): add refresh_tokens table migration"
```

---

## Task 2: Auth — JWT Sign/Verify Library

**Files:**
- Create: `simse-auth/src/lib/jwt.ts`
- Modify: `simse-auth/src/types.ts:1-10`
- Modify: `simse-auth/wrangler.toml:1-23`

**Step 1: Add SECRETS binding to wrangler.toml**

Add to `simse-auth/wrangler.toml`:

```toml
[[secrets_store.bindings]]
binding = "SECRETS"
store_id = "PLACEHOLDER_REPLACE_WITH_STORE_ID"
```

**Step 2: Update Env type**

Modify `simse-auth/src/types.ts` to add `SECRETS`:

```typescript
export interface Env {
	DB: D1Database;
	COMMS_QUEUE: Queue;
	ANALYTICS: AnalyticsEngineDataset;
	SECRETS: SecretsStoreNamespace;
}

export interface AuthContext {
	userId: string;
	sessionId?: string;
}
```

**Step 3: Create JWT library**

Create `simse-auth/src/lib/jwt.ts`:

```typescript
export interface JwtPayload {
	sub: string; // userId
	tid: string | null; // teamId
	role: string | null; // team role
	sid: string; // sessionId (family_id)
	exp: number; // expiry (unix seconds)
	iat: number; // issued at (unix seconds)
}

const ACCESS_TOKEN_TTL_SECONDS = 900; // 15 minutes
const ALGORITHM = { name: 'HMAC', hash: 'SHA-256' };

function base64UrlEncode(data: Uint8Array): string {
	return btoa(String.fromCharCode(...data))
		.replace(/\+/g, '-')
		.replace(/\//g, '_')
		.replace(/=+$/, '');
}

function base64UrlDecode(str: string): Uint8Array {
	const padded = str.replace(/-/g, '+').replace(/_/g, '/');
	const binary = atob(padded);
	return Uint8Array.from(binary, (c) => c.charCodeAt(0));
}

async function importKey(secret: string): Promise<CryptoKey> {
	const enc = new TextEncoder();
	return crypto.subtle.importKey('raw', enc.encode(secret), ALGORITHM, false, [
		'sign',
		'verify',
	]);
}

export async function signJwt(
	payload: Omit<JwtPayload, 'exp' | 'iat'>,
	secret: string,
): Promise<{ token: string; expiresIn: number }> {
	const now = Math.floor(Date.now() / 1000);
	const fullPayload: JwtPayload = {
		...payload,
		iat: now,
		exp: now + ACCESS_TOKEN_TTL_SECONDS,
	};

	const enc = new TextEncoder();
	const header = base64UrlEncode(
		enc.encode(JSON.stringify({ alg: 'HS256', typ: 'JWT' })),
	);
	const body = base64UrlEncode(enc.encode(JSON.stringify(fullPayload)));
	const signingInput = `${header}.${body}`;

	const key = await importKey(secret);
	const signature = await crypto.subtle.sign(
		'HMAC',
		key,
		enc.encode(signingInput),
	);

	const sig = base64UrlEncode(new Uint8Array(signature));
	return { token: `${signingInput}.${sig}`, expiresIn: ACCESS_TOKEN_TTL_SECONDS };
}

export async function verifyJwt(
	token: string,
	secret: string,
): Promise<JwtPayload | null> {
	const parts = token.split('.');
	if (parts.length !== 3) return null;

	const [header, body, sig] = parts;
	const enc = new TextEncoder();
	const signingInput = `${header}.${body}`;

	const key = await importKey(secret);
	const signatureBytes = base64UrlDecode(sig);

	const valid = await crypto.subtle.verify(
		'HMAC',
		key,
		signatureBytes,
		enc.encode(signingInput),
	);

	if (!valid) return null;

	const payload = JSON.parse(
		new TextDecoder().decode(base64UrlDecode(body)),
	) as JwtPayload;

	return payload;
}
```

**Step 4: Verify lint passes**

Run: `cd simse-auth && npm run lint`
Expected: No errors.

**Step 5: Commit**

```bash
git add simse-auth/src/lib/jwt.ts simse-auth/src/types.ts simse-auth/wrangler.toml
git commit -m "feat(simse-auth): add JWT sign/verify library with HMAC-SHA256"
```

---

## Task 3: Auth — Refresh Token Library

**Files:**
- Create: `simse-auth/src/lib/refresh-token.ts`

**Step 1: Create refresh token library**

Create `simse-auth/src/lib/refresh-token.ts`:

```typescript
import { generateId } from './db';

const REFRESH_TOKEN_TTL_DAYS = 30;

function generateRefreshToken(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(32));
	const hex = Array.from(bytes)
		.map((b) => b.toString(16).padStart(2, '0'))
		.join('');
	return `rt_${hex}`;
}

async function hashToken(token: string): Promise<string> {
	const encoder = new TextEncoder();
	const data = encoder.encode(token);
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = new Uint8Array(hashBuffer);
	return btoa(String.fromCharCode(...hashArray));
}

export async function createRefreshToken(
	db: D1Database,
	userId: string,
	familyId?: string,
): Promise<{ token: string; familyId: string }> {
	const id = generateId();
	const family = familyId ?? generateId();
	const rawToken = generateRefreshToken();
	const tokenHash = await hashToken(rawToken);
	const expiresAt = new Date(
		Date.now() + REFRESH_TOKEN_TTL_DAYS * 24 * 60 * 60 * 1000,
	).toISOString();

	await db
		.prepare(
			'INSERT INTO refresh_tokens (id, user_id, family_id, token_hash, expires_at) VALUES (?, ?, ?, ?, ?)',
		)
		.bind(id, userId, family, tokenHash, expiresAt)
		.run();

	return { token: rawToken, familyId: family };
}

export async function rotateRefreshToken(
	db: D1Database,
	rawToken: string,
): Promise<{
	ok: true;
	userId: string;
	familyId: string;
	newToken: string;
} | {
	ok: false;
	code: 'INVALID_TOKEN' | 'TOKEN_REUSED';
}> {
	const tokenHash = await hashToken(rawToken);

	// Look up the token
	const row = await db
		.prepare(
			'SELECT id, user_id, family_id, revoked, expires_at FROM refresh_tokens WHERE token_hash = ?',
		)
		.bind(tokenHash)
		.first<{
			id: string;
			user_id: string;
			family_id: string;
			revoked: number;
			expires_at: string;
		}>();

	if (!row) {
		return { ok: false, code: 'INVALID_TOKEN' };
	}

	// Reuse detection: if token was already revoked, revoke entire family
	if (row.revoked) {
		await db
			.prepare(
				'UPDATE refresh_tokens SET revoked = 1 WHERE family_id = ?',
			)
			.bind(row.family_id)
			.run();
		return { ok: false, code: 'TOKEN_REUSED' };
	}

	// Check expiry
	if (new Date(row.expires_at) <= new Date()) {
		return { ok: false, code: 'INVALID_TOKEN' };
	}

	// Revoke old token
	await db
		.prepare('UPDATE refresh_tokens SET revoked = 1 WHERE id = ?')
		.bind(row.id)
		.run();

	// Issue new token in same family
	const result = await createRefreshToken(db, row.user_id, row.family_id);

	return {
		ok: true,
		userId: row.user_id,
		familyId: row.family_id,
		newToken: result.token,
	};
}

export async function revokeFamily(
	db: D1Database,
	familyId: string,
): Promise<void> {
	await db
		.prepare('UPDATE refresh_tokens SET revoked = 1 WHERE family_id = ?')
		.bind(familyId)
		.run();
}

export async function revokeAllUserTokens(
	db: D1Database,
	userId: string,
): Promise<void> {
	await db
		.prepare('UPDATE refresh_tokens SET revoked = 1 WHERE user_id = ?')
		.bind(userId)
		.run();
}
```

**Step 2: Verify lint passes**

Run: `cd simse-auth && npm run lint`
Expected: No errors.

**Step 3: Commit**

```bash
git add simse-auth/src/lib/refresh-token.ts
git commit -m "feat(simse-auth): add refresh token library with rotation and reuse detection"
```

---

## Task 4: Auth — Update Login/Register to Return JWTs

**Files:**
- Modify: `simse-auth/src/routes/auth.ts:1-98` (register + login)
- Modify: `simse-auth/src/schemas.ts` (add refreshSchema)

**Step 1: Add refresh schema to schemas.ts**

Add to `simse-auth/src/schemas.ts` after line 48:

```typescript
export const refreshSchema = z.object({
	refreshToken: z.string().startsWith('rt_'),
});

export const revokeSchema = z.object({
	refreshToken: z.string().startsWith('rt_'),
});
```

**Step 2: Update register endpoint**

In `simse-auth/src/routes/auth.ts`, update the register handler (lines 22-67).

Replace `createSession` import with JWT + refresh token imports. After creating the user, instead of calling `createSession`, sign a JWT and create a refresh token:

```typescript
// At top of file, update imports:
import { Hono } from 'hono';
import { generateId } from '../lib/db';
import { sendEmail } from '../lib/comms';
import { signJwt } from '../lib/jwt';
import { hashPassword, verifyPassword } from '../lib/password';
import { createRefreshToken, revokeFamily, rotateRefreshToken } from '../lib/refresh-token';
import { createSession, deleteSession } from '../lib/session';
import { createToken, generateCode, markTokenUsed, validateToken } from '../lib/token';
import {
	loginSchema,
	newPasswordSchema,
	refreshSchema,
	registerSchema,
	resetPasswordSchema,
	revokeSchema,
	twoFactorSchema,
} from '../schemas';
import type { AuthContext, Env } from '../types';
```

Update register handler response (replace lines 56-66):

```typescript
	const jwtSecret = await c.env.SECRETS.get('JWT_SECRET');
	if (!jwtSecret) {
		return c.json({ error: { code: 'MISCONFIGURED', message: 'Service misconfigured' } }, 500);
	}

	// Get team info for JWT
	const teamRow = await db
		.prepare('SELECT t.id, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1')
		.bind(userId)
		.first<{ id: string; role: string }>();

	const { token: accessToken, expiresIn } = await signJwt(
		{ sub: userId, tid: teamRow?.id ?? null, role: teamRow?.role ?? null, sid: generateId() },
		jwtSecret,
	);
	const { token: refreshToken } = await createRefreshToken(db, userId);

	await sendEmail(c.env.COMMS_QUEUE, 'verify-email', normalizedEmail, { code: verifyCode });

	return c.json({
		data: {
			accessToken,
			refreshToken,
			expiresIn,
			user: { id: userId, email: normalizedEmail, name },
		},
	}, 201);
```

**Step 3: Update login endpoint**

Update login handler (lines 69-98). Replace the session creation with JWT + refresh token:

For the non-2FA branch (replace lines 96-97):

```typescript
	const teamRow = await db
		.prepare('SELECT t.id, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1')
		.bind(user.id)
		.first<{ id: string; role: string }>();

	const jwtSecret = await c.env.SECRETS.get('JWT_SECRET');
	if (!jwtSecret) {
		return c.json({ error: { code: 'MISCONFIGURED', message: 'Service misconfigured' } }, 500);
	}

	const { token: accessToken, expiresIn } = await signJwt(
		{ sub: user.id, tid: teamRow?.id ?? null, role: teamRow?.role ?? null, sid: generateId() },
		jwtSecret,
	);
	const { token: refreshToken } = await createRefreshToken(db, user.id);

	return c.json({ data: { accessToken, refreshToken, expiresIn, user: { id: user.id } } });
```

**Step 4: Update 2FA endpoint**

Update the 2FA handler (lines 100-130). Replace `createSession` with JWT + refresh:

Replace lines 128-129:

```typescript
	const teamRow = await db
		.prepare('SELECT t.id, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1')
		.bind(pending.user_id)
		.first<{ id: string; role: string }>();

	const jwtSecret = await c.env.SECRETS.get('JWT_SECRET');
	if (!jwtSecret) {
		return c.json({ error: { code: 'MISCONFIGURED', message: 'Service misconfigured' } }, 500);
	}

	const { token: accessToken, expiresIn } = await signJwt(
		{ sub: pending.user_id, tid: teamRow?.id ?? null, role: teamRow?.role ?? null, sid: generateId() },
		jwtSecret,
	);
	const { token: refreshToken } = await createRefreshToken(db, pending.user_id);

	return c.json({ data: { accessToken, refreshToken, expiresIn, user: { id: pending.user_id } } });
```

**Step 5: Verify lint passes**

Run: `cd simse-auth && npm run lint`
Expected: No errors.

**Step 6: Commit**

```bash
git add simse-auth/src/routes/auth.ts simse-auth/src/schemas.ts
git commit -m "feat(simse-auth): return JWT access tokens + refresh tokens from login/register/2fa"
```

---

## Task 5: Auth — Add Refresh & Revoke Endpoints

**Files:**
- Modify: `simse-auth/src/routes/auth.ts` (add after verify-email endpoint, before validate)

**Step 1: Add POST /auth/refresh endpoint**

Add after the verify-email handler (after line 201):

```typescript
// POST /auth/refresh — rotate refresh token, issue new JWT
auth.post('/refresh', async (c) => {
	const body = await c.req.json();
	const parsed = refreshSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	const db = c.env.DB;
	const result = await rotateRefreshToken(db, parsed.data.refreshToken);

	if (!result.ok) {
		const status = result.code === 'TOKEN_REUSED' ? 401 : 401;
		return c.json({ error: { code: result.code, message: result.code === 'TOKEN_REUSED' ? 'Token reuse detected, session revoked' : 'Invalid refresh token' } }, status);
	}

	const jwtSecret = await c.env.SECRETS.get('JWT_SECRET');
	if (!jwtSecret) {
		return c.json({ error: { code: 'MISCONFIGURED', message: 'Service misconfigured' } }, 500);
	}

	const teamRow = await db
		.prepare('SELECT t.id, tm.role FROM teams t JOIN team_members tm ON t.id = tm.team_id WHERE tm.user_id = ? LIMIT 1')
		.bind(result.userId)
		.first<{ id: string; role: string }>();

	const { token: accessToken, expiresIn } = await signJwt(
		{ sub: result.userId, tid: teamRow?.id ?? null, role: teamRow?.role ?? null, sid: result.familyId },
		jwtSecret,
	);

	return c.json({ data: { accessToken, refreshToken: result.newToken, expiresIn } });
});

// POST /auth/revoke — revoke a refresh token family (explicit logout)
auth.post('/revoke', async (c) => {
	const body = await c.req.json();
	const parsed = revokeSchema.safeParse(body);
	if (!parsed.success) {
		return c.json({ error: { code: 'VALIDATION_ERROR', message: parsed.error.issues[0].message } }, 400);
	}

	// Hash and look up to get family_id, then revoke family
	const encoder = new TextEncoder();
	const data = encoder.encode(parsed.data.refreshToken);
	const hashBuffer = await crypto.subtle.digest('SHA-256', data);
	const hashArray = new Uint8Array(hashBuffer);
	const tokenHash = btoa(String.fromCharCode(...hashArray));

	const db = c.env.DB;
	const row = await db
		.prepare('SELECT family_id FROM refresh_tokens WHERE token_hash = ?')
		.bind(tokenHash)
		.first<{ family_id: string }>();

	if (row) {
		await revokeFamily(db, row.family_id);
	}

	// Always return ok (don't leak whether token existed)
	return c.json({ data: { ok: true } });
});
```

**Step 2: Update PUBLIC_AUTH_PATHS in API gateway**

This will be done in Task 8 when we update the gateway.

**Step 3: Update the /auth/validate endpoint to also accept JWTs**

The `/auth/validate` endpoint currently handles `session_*` and `sk_*` tokens. Update it to also verify JWTs for backwards compatibility. In the `validate` handler, add a JWT branch:

Add before `if (!userId)` (before line 242 in auth.ts):

```typescript
	// JWT access token validation (for backwards compat / direct calls)
	if (!userId && token.includes('.')) {
		const jwtSecret = await c.env.SECRETS.get('JWT_SECRET');
		if (jwtSecret) {
			const { verifyJwt } = await import('../lib/jwt');
			const payload = await verifyJwt(token, jwtSecret);
			if (payload && payload.exp > Math.floor(Date.now() / 1000)) {
				userId = payload.sub;
				sessionId = payload.sid;
				// Get team info from JWT payload directly
				const teamFromJwt = payload.tid
					? { id: payload.tid, role: payload.role }
					: null;
				if (teamFromJwt) {
					return c.json({
						data: { userId, sessionId, teamId: teamFromJwt.id, role: teamFromJwt.role },
					});
				}
			}
		}
	}
```

**Step 4: Update account deletion to also delete refresh tokens**

In `simse-auth/src/routes/users.ts`, add to the batch delete (line 62-69), add:

```typescript
db.prepare('DELETE FROM refresh_tokens WHERE user_id = ?').bind(userId),
```

**Step 5: Verify lint passes**

Run: `cd simse-auth && npm run lint`
Expected: No errors.

**Step 6: Commit**

```bash
git add simse-auth/src/routes/auth.ts simse-auth/src/routes/users.ts
git commit -m "feat(simse-auth): add /auth/refresh and /auth/revoke endpoints with reuse detection"
```

---

## Task 6: API Gateway — JWT Validation Middleware

**Files:**
- Create: `simse-api/src/lib/jwt.ts`
- Modify: `simse-api/src/types.ts:1-23`
- Modify: `simse-api/src/middleware/secrets.ts:1-47`

**Step 1: Create JWT verify library (gateway copy — verify only)**

Create `simse-api/src/lib/jwt.ts`:

```typescript
export interface JwtPayload {
	sub: string;
	tid: string | null;
	role: string | null;
	sid: string;
	exp: number;
	iat: number;
}

const ALGORITHM = { name: 'HMAC', hash: 'SHA-256' };

function base64UrlDecode(str: string): Uint8Array {
	const padded = str.replace(/-/g, '+').replace(/_/g, '/');
	const binary = atob(padded);
	return Uint8Array.from(binary, (c) => c.charCodeAt(0));
}

async function importKey(secret: string): Promise<CryptoKey> {
	const enc = new TextEncoder();
	return crypto.subtle.importKey('raw', enc.encode(secret), ALGORITHM, false, [
		'verify',
	]);
}

export async function verifyJwt(
	token: string,
	secret: string,
): Promise<{ payload: JwtPayload; expired: boolean } | null> {
	const parts = token.split('.');
	if (parts.length !== 3) return null;

	const [header, body, sig] = parts;
	const enc = new TextEncoder();
	const signingInput = `${header}.${body}`;

	const key = await importKey(secret);
	const signatureBytes = base64UrlDecode(sig);

	const valid = await crypto.subtle.verify(
		'HMAC',
		key,
		signatureBytes,
		enc.encode(signingInput),
	);

	if (!valid) return null;

	const payload = JSON.parse(
		new TextDecoder().decode(base64UrlDecode(body)),
	) as JwtPayload;

	const now = Math.floor(Date.now() / 1000);
	return { payload, expired: payload.exp <= now };
}
```

**Step 2: Update ApiSecrets type**

Modify `simse-api/src/types.ts` — add `jwtSecret` to `ApiSecrets`:

```typescript
export interface Env {
	COMMS_QUEUE: Queue;
	SECRETS: SecretsStoreNamespace;
	ANALYTICS: AnalyticsEngineDataset;
}

export interface ApiSecrets {
	authApiUrl: string;
	authApiSecret: string;
	paymentsApiUrl: string;
	paymentsApiSecret: string;
	mailerApiUrl: string;
	mailerApiSecret: string;
	jwtSecret: string;
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

**Step 3: Update secrets middleware**

Modify `simse-api/src/middleware/secrets.ts` to also fetch `JWT_SECRET`:

```typescript
import { createMiddleware } from 'hono/factory';
import type { ApiSecrets, Env } from '../types';

export const secretsMiddleware = createMiddleware<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets };
}>(async (c, next) => {
	const [
		authApiUrl,
		authApiSecret,
		paymentsApiUrl,
		paymentsApiSecret,
		mailerApiUrl,
		mailerApiSecret,
		jwtSecret,
	] = await Promise.all([
		c.env.SECRETS.get('AUTH_API_URL'),
		c.env.SECRETS.get('AUTH_API_SECRET'),
		c.env.SECRETS.get('PAYMENTS_API_URL'),
		c.env.SECRETS.get('PAYMENTS_API_SECRET'),
		c.env.SECRETS.get('MAILER_API_URL'),
		c.env.SECRETS.get('MAILER_API_SECRET'),
		c.env.SECRETS.get('JWT_SECRET'),
	]);

	if (
		!authApiUrl ||
		!authApiSecret ||
		!paymentsApiUrl ||
		!paymentsApiSecret ||
		!mailerApiUrl ||
		!mailerApiSecret ||
		!jwtSecret
	) {
		return c.json(
			{ error: { code: 'MISCONFIGURED', message: 'Service misconfigured' } },
			500,
		);
	}

	c.set('secrets', {
		authApiUrl,
		authApiSecret,
		paymentsApiUrl,
		paymentsApiSecret,
		mailerApiUrl,
		mailerApiSecret,
		jwtSecret,
	});
	await next();
});
```

**Step 4: Verify lint passes**

Run: `cd simse-api && npm run lint`
Expected: No errors.

**Step 5: Commit**

```bash
git add simse-api/src/lib/jwt.ts simse-api/src/types.ts simse-api/src/middleware/secrets.ts
git commit -m "feat(simse-api): add local JWT verification and update secrets middleware"
```

---

## Task 7: API Gateway — Circuit Breaker

**Files:**
- Create: `simse-api/src/lib/circuit-breaker.ts`

**Step 1: Create circuit breaker class**

Create `simse-api/src/lib/circuit-breaker.ts`:

```typescript
type CircuitState = 'closed' | 'open' | 'half-open';

export class CircuitBreaker {
	readonly name: string;
	private state: CircuitState = 'closed';
	private failureCount = 0;
	private lastFailureTime = 0;
	private readonly failureThreshold: number;
	private readonly resetTimeoutMs: number;
	private readonly windowMs: number;

	constructor(
		name: string,
		options?: {
			failureThreshold?: number;
			resetTimeoutMs?: number;
			windowMs?: number;
		},
	) {
		this.name = name;
		this.failureThreshold = options?.failureThreshold ?? 5;
		this.resetTimeoutMs = options?.resetTimeoutMs ?? 30_000;
		this.windowMs = options?.windowMs ?? 60_000;
	}

	canRequest(): boolean {
		if (this.state === 'closed') return true;

		if (this.state === 'open') {
			// Check if cool-down has elapsed
			if (Date.now() - this.lastFailureTime >= this.resetTimeoutMs) {
				this.state = 'half-open';
				return true;
			}
			return false;
		}

		// half-open: allow one probe
		return true;
	}

	recordSuccess(): void {
		this.state = 'closed';
		this.failureCount = 0;
	}

	recordFailure(): void {
		const now = Date.now();

		// Reset count if outside window
		if (now - this.lastFailureTime > this.windowMs) {
			this.failureCount = 0;
		}

		this.failureCount++;
		this.lastFailureTime = now;

		if (this.state === 'half-open') {
			this.state = 'open';
			return;
		}

		if (this.failureCount >= this.failureThreshold) {
			this.state = 'open';
		}
	}

	getState(): CircuitState {
		// Re-evaluate open state for staleness
		if (
			this.state === 'open' &&
			Date.now() - this.lastFailureTime >= this.resetTimeoutMs
		) {
			this.state = 'half-open';
		}
		return this.state;
	}

	getStatus(): 'healthy' | 'degraded' | 'open' {
		const s = this.getState();
		if (s === 'closed') return 'healthy';
		if (s === 'half-open') return 'degraded';
		return 'open';
	}
}
```

**Step 2: Verify lint passes**

Run: `cd simse-api && npm run lint`
Expected: No errors.

**Step 3: Commit**

```bash
git add simse-api/src/lib/circuit-breaker.ts
git commit -m "feat(simse-api): add per-backend circuit breaker with 3-state machine"
```

---

## Task 8: API Gateway — Resilient Fetch (Timeout + Retry + Backoff)

**Files:**
- Create: `simse-api/src/lib/resilient-fetch.ts`

**Step 1: Create resilient fetch function**

Create `simse-api/src/lib/resilient-fetch.ts`:

```typescript
import type { CircuitBreaker } from './circuit-breaker';

const TIMEOUT_MS = 5_000;
const MAX_RETRIES = 2;
const BASE_DELAY_MS = 1_000;
const JITTER_FACTOR = 0.2;
const RETRYABLE_STATUSES = new Set([502, 503, 504]);

function jitteredDelay(baseMs: number): number {
	const jitter = baseMs * JITTER_FACTOR * (2 * Math.random() - 1);
	return Math.max(0, baseMs + jitter);
}

function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms));
}

export async function resilientFetch(
	url: string,
	init: RequestInit,
	breaker: CircuitBreaker,
): Promise<Response> {
	if (!breaker.canRequest()) {
		return new Response(
			JSON.stringify({
				error: {
					code: 'SERVICE_UNAVAILABLE',
					message: `${breaker.name} is temporarily unavailable`,
				},
			}),
			{
				status: 503,
				headers: {
					'Content-Type': 'application/json',
					'Retry-After': '30',
				},
			},
		);
	}

	const canRetry = init.method === 'GET' || init.method === 'HEAD';
	const maxAttempts = canRetry ? MAX_RETRIES + 1 : 1;

	let lastError: Error | null = null;
	let lastResponse: Response | null = null;

	for (let attempt = 0; attempt < maxAttempts; attempt++) {
		if (attempt > 0) {
			const delay = jitteredDelay(BASE_DELAY_MS * 2 ** (attempt - 1));
			await sleep(delay);

			// Re-check circuit breaker before retry
			if (!breaker.canRequest()) {
				break;
			}
		}

		try {
			const controller = new AbortController();
			const timeoutId = setTimeout(() => controller.abort(), TIMEOUT_MS);

			const response = await fetch(url, {
				...init,
				signal: controller.signal,
			});

			clearTimeout(timeoutId);

			if (RETRYABLE_STATUSES.has(response.status) && canRetry && attempt < maxAttempts - 1) {
				lastResponse = response;
				breaker.recordFailure();
				continue;
			}

			if (response.ok || response.status < 500) {
				breaker.recordSuccess();
			} else {
				breaker.recordFailure();
			}

			return response;
		} catch (error) {
			lastError = error as Error;
			breaker.recordFailure();

			if (!canRetry || attempt >= maxAttempts - 1) break;
		}
	}

	// All retries exhausted
	if (lastResponse) return lastResponse;

	const message = lastError?.name === 'AbortError' ? 'Request timeout' : 'Service unavailable';
	return new Response(
		JSON.stringify({
			error: { code: 'SERVICE_UNAVAILABLE', message },
		}),
		{
			status: 503,
			headers: { 'Content-Type': 'application/json', 'Retry-After': '30' },
		},
	);
}
```

**Step 2: Verify lint passes**

Run: `cd simse-api && npm run lint`
Expected: No errors.

**Step 3: Commit**

```bash
git add simse-api/src/lib/resilient-fetch.ts
git commit -m "feat(simse-api): add resilient fetch with 5s timeout, exponential backoff, and jitter"
```

---

## Task 9: API Gateway — Rate Limiter

**Files:**
- Create: `simse-api/src/lib/rate-limiter.ts`
- Create: `simse-api/src/middleware/rate-limit.ts`

**Step 1: Create rate limiter class**

Create `simse-api/src/lib/rate-limiter.ts`:

```typescript
interface WindowEntry {
	count: number;
	resetAt: number;
}

export class RateLimiter {
	private windows = new Map<string, WindowEntry>();
	private readonly windowMs: number;

	constructor(windowMs = 60_000) {
		this.windowMs = windowMs;
	}

	check(
		key: string,
		limit: number,
	): { allowed: boolean; remaining: number; resetAt: number } {
		const now = Date.now();
		const entry = this.windows.get(key);

		if (!entry || now >= entry.resetAt) {
			const resetAt = now + this.windowMs;
			this.windows.set(key, { count: 1, resetAt });
			return { allowed: true, remaining: limit - 1, resetAt };
		}

		entry.count++;

		if (entry.count > limit) {
			return {
				allowed: false,
				remaining: 0,
				resetAt: entry.resetAt,
			};
		}

		return {
			allowed: true,
			remaining: limit - entry.count,
			resetAt: entry.resetAt,
		};
	}

	/** Remove expired entries to prevent memory growth */
	prune(): void {
		const now = Date.now();
		for (const [key, entry] of this.windows) {
			if (now >= entry.resetAt) {
				this.windows.delete(key);
			}
		}
	}
}
```

**Step 2: Create rate limit middleware**

Create `simse-api/src/middleware/rate-limit.ts`:

```typescript
import { createMiddleware } from 'hono/factory';
import { RateLimiter } from '../lib/rate-limiter';
import type { Env } from '../types';

const limiter = new RateLimiter(60_000);

// Prune every 60s to prevent memory leak
let lastPrune = Date.now();

interface RateLimitRule {
	pattern: RegExp;
	limit: number;
	keyType: 'ip' | 'user';
}

const rules: RateLimitRule[] = [
	// Public routes — per-IP
	{ pattern: /^\/auth\/(login|register)$/, limit: 10, keyType: 'ip' },
	{ pattern: /^\/auth\/(reset-password|new-password)$/, limit: 5, keyType: 'ip' },
	{ pattern: /^\/auth\/(2fa|verify-email)$/, limit: 10, keyType: 'ip' },
	{ pattern: /^\/auth\/refresh$/, limit: 30, keyType: 'ip' },
	// Protected routes — per-user
	{ pattern: /^\/(users|teams|api-keys)(\/|$)/, limit: 60, keyType: 'user' },
	{ pattern: /^\/payments(\/|$)/, limit: 30, keyType: 'user' },
	{ pattern: /^\/notifications(\/|$)/, limit: 20, keyType: 'user' },
];

function getClientIp(c: any): string {
	return c.req.header('CF-Connecting-IP') ?? c.req.header('X-Forwarded-For')?.split(',')[0]?.trim() ?? 'unknown';
}

export const rateLimitMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	// Periodic prune
	const now = Date.now();
	if (now - lastPrune > 60_000) {
		limiter.prune();
		lastPrune = now;
	}

	const path = c.req.path;
	const rule = rules.find((r) => r.pattern.test(path));

	if (!rule) {
		await next();
		return;
	}

	const key =
		rule.keyType === 'ip'
			? `ip:${getClientIp(c)}:${rule.pattern.source}`
			: `user:${c.req.header('X-User-Id') ?? getClientIp(c)}:${rule.pattern.source}`;

	const result = limiter.check(key, rule.limit);

	c.header('X-RateLimit-Limit', String(rule.limit));
	c.header('X-RateLimit-Remaining', String(Math.max(0, result.remaining)));
	c.header('X-RateLimit-Reset', String(Math.ceil(result.resetAt / 1000)));

	if (!result.allowed) {
		const retryAfter = Math.ceil((result.resetAt - now) / 1000);
		c.header('Retry-After', String(retryAfter));
		return c.json(
			{ error: { code: 'RATE_LIMITED', message: 'Too many requests' } },
			429,
		);
	}

	await next();
});
```

**Step 3: Verify lint passes**

Run: `cd simse-api && npm run lint`
Expected: No errors.

**Step 4: Commit**

```bash
git add simse-api/src/lib/rate-limiter.ts simse-api/src/middleware/rate-limit.ts
git commit -m "feat(simse-api): add tiered rate limiting — per-IP for public, per-user for protected routes"
```

---

## Task 10: API Gateway — Request Validation & Security Middleware

**Files:**
- Create: `simse-api/src/middleware/security.ts`

**Step 1: Create security middleware**

Create `simse-api/src/middleware/security.ts`:

```typescript
import { createMiddleware } from 'hono/factory';
import type { Env } from '../types';

const MAX_BODY_SIZE = 1_048_576; // 1MB

export const requestValidationMiddleware = createMiddleware<{
	Bindings: Env;
}>(async (c, next) => {
	// Generate or pass through correlation ID
	const requestId = c.req.header('X-Request-Id') ?? crypto.randomUUID();
	c.set('requestId' as any, requestId);

	// Validate Content-Type on request bodies
	if (['POST', 'PUT', 'PATCH'].includes(c.req.method)) {
		const contentType = c.req.header('Content-Type');
		if (contentType && !contentType.includes('application/json')) {
			return c.json(
				{
					error: {
						code: 'UNSUPPORTED_MEDIA_TYPE',
						message: 'Content-Type must be application/json',
					},
					requestId,
				},
				415,
			);
		}

		// Check body size
		const contentLength = Number(c.req.header('Content-Length') ?? 0);
		if (contentLength > MAX_BODY_SIZE) {
			return c.json(
				{
					error: {
						code: 'PAYLOAD_TOO_LARGE',
						message: 'Request body exceeds 1MB limit',
					},
					requestId,
				},
				413,
			);
		}
	}

	await next();

	// Set security headers on all responses
	c.header('X-Request-Id', requestId);
	c.header('X-Content-Type-Options', 'nosniff');

	// Strip leaked backend headers
	c.res.headers.delete('Server');
	c.res.headers.delete('X-Powered-By');
});
```

**Step 2: Verify lint passes**

Run: `cd simse-api && npm run lint`
Expected: No errors.

**Step 3: Commit**

```bash
git add simse-api/src/middleware/security.ts
git commit -m "feat(simse-api): add request validation, correlation IDs, and security headers"
```

---

## Task 11: API Gateway — Rewrite Gateway with All Safety Features

**Files:**
- Modify: `simse-api/src/index.ts:1-14`
- Modify: `simse-api/src/routes/gateway.ts:1-170` (full rewrite)

**Step 1: Update index.ts to add new middleware**

Rewrite `simse-api/src/index.ts`:

```typescript
import { Hono } from 'hono';
import { CircuitBreaker } from './lib/circuit-breaker';
import { analyticsMiddleware } from './middleware/analytics';
import { rateLimitMiddleware } from './middleware/rate-limit';
import { secretsMiddleware } from './middleware/secrets';
import { requestValidationMiddleware } from './middleware/security';
import gateway from './routes/gateway';
import type { Env } from './types';

// Per-backend circuit breakers (shared across requests within worker instance)
export const breakers = {
	auth: new CircuitBreaker('auth'),
	payments: new CircuitBreaker('payments'),
	mailer: new CircuitBreaker('mailer'),
};

const app = new Hono<{ Bindings: Env }>();

app.use('*', analyticsMiddleware);
app.use('*', requestValidationMiddleware);
app.use('*', rateLimitMiddleware);

app.get('/health', (c) => {
	return c.json({
		ok: true,
		services: {
			auth: breakers.auth.getStatus(),
			payments: breakers.payments.getStatus(),
			mailer: breakers.mailer.getStatus(),
		},
	});
});

app.use('*', secretsMiddleware);
app.route('', gateway);

export default app;
```

**Step 2: Rewrite gateway with JWT validation + resilient fetch**

Rewrite `simse-api/src/routes/gateway.ts`:

```typescript
import { Hono } from 'hono';
import { breakers } from '../index';
import { verifyJwt } from '../lib/jwt';
import { resilientFetch } from '../lib/resilient-fetch';
import type { ApiSecrets, Env, ValidateResponse } from '../types';

const gateway = new Hono<{
	Bindings: Env;
	Variables: { secrets: ApiSecrets; requestId: string };
}>();

const PUBLIC_AUTH_PATHS = [
	'/register',
	'/login',
	'/2fa',
	'/reset-password',
	'/new-password',
	'/verify-email',
	'/refresh',
	'/revoke',
];

// --- Auth routes ---
gateway.all('/auth/*', async (c) => {
	const subpath = c.req.path.replace('/auth', '');
	const isPublic = PUBLIC_AUTH_PATHS.some((p) => subpath === p);

	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	headers.set('X-Request-Id', c.get('requestId' as any) ?? '');

	if (!isPublic) {
		const auth = await authenticateRequest(c);
		if (!auth) {
			return c.json(
				{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
				401,
			);
		}
		setAuthHeaders(headers, auth);
	}

	return proxyTo(c, `${c.var.secrets.authApiUrl}${c.req.path}`, headers, breakers.auth);
});

// --- Protected service routes ---
for (const prefix of ['/users', '/teams', '/api-keys']) {
	gateway.all(`${prefix}/*`, async (c) => {
		const auth = await authenticateRequest(c);
		if (!auth) {
			return c.json(
				{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
				401,
			);
		}

		const headers = serviceHeaders(auth, c);
		return proxyTo(c, `${c.var.secrets.authApiUrl}${c.req.path}`, headers, breakers.auth);
	});
}

// --- Payments proxy ---
gateway.all('/payments/*', async (c) => {
	const auth = await authenticateRequest(c);
	if (!auth) {
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
			401,
		);
	}

	const path = c.req.path.replace('/payments', '');
	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.var.secrets.paymentsApiSecret}`);
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);
	headers.set('X-Request-Id', c.get('requestId' as any) ?? '');
	if (auth.teamId) headers.set('X-Team-Id', auth.teamId);

	return proxyTo(c, `${c.var.secrets.paymentsApiUrl}${path}`, headers, breakers.payments);
});

// --- Notifications proxy ---
gateway.all('/notifications', proxyNotifications);
gateway.all('/notifications/*', proxyNotifications);

async function proxyNotifications(c: any) {
	const auth = await authenticateRequest(c);
	if (!auth) {
		return c.json(
			{ error: { code: 'UNAUTHORIZED', message: 'Invalid token' } },
			401,
		);
	}

	if (c.req.method === 'POST') {
		const body = await c.req.json();
		await c.env.COMMS_QUEUE.send({
			type: 'notification',
			userId: auth.userId,
			...body,
		});
		return c.json({ data: { ok: true } });
	}

	const headers = new Headers();
	headers.set('Authorization', `Bearer ${c.var.secrets.mailerApiSecret}`);
	headers.set('Content-Type', 'application/json');
	headers.set('X-User-Id', auth.userId);
	headers.set('X-Request-Id', c.get('requestId' as any) ?? '');

	return proxyTo(c, `${c.var.secrets.mailerApiUrl}${c.req.path}`, headers, breakers.mailer);
}

// --- Helpers ---

interface AuthResult {
	userId: string;
	sessionId?: string;
	teamId: string | null;
	role: string | null;
}

async function authenticateRequest(c: any): Promise<AuthResult | null> {
	const authHeader = c.req.header('Authorization');
	if (!authHeader?.startsWith('Bearer ')) return null;

	const token = authHeader.slice(7);

	// API keys — validate via auth service
	if (token.startsWith('sk_')) {
		return validateTokenViaService(c, token);
	}

	// JWT access token — validate locally
	const jwtSecret = c.var.secrets.jwtSecret;
	const result = await verifyJwt(token, jwtSecret);

	if (!result) {
		// Not a valid JWT — try legacy session token validation
		if (token.startsWith('session_')) {
			return validateTokenViaService(c, token);
		}
		return null;
	}

	if (result.expired) {
		// Return a special response to signal token expiry
		// We throw to be caught in the route handler
		c.set('tokenExpired' as any, true);
		return null;
	}

	return {
		userId: result.payload.sub,
		sessionId: result.payload.sid,
		teamId: result.payload.tid,
		role: result.payload.role,
	};
}

async function validateTokenViaService(
	c: any,
	token: string,
): Promise<AuthResult | null> {
	const init: RequestInit = {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ token }),
	};

	const res = await resilientFetch(
		`${c.var.secrets.authApiUrl}/auth/validate`,
		init,
		breakers.auth,
	);

	if (!res.ok) return null;

	const json = (await res.json()) as ValidateResponse;
	return json.data;
}

function setAuthHeaders(headers: Headers, auth: AuthResult): void {
	headers.set('X-User-Id', auth.userId);
	if (auth.sessionId) headers.set('X-Session-Id', auth.sessionId);
	if (auth.teamId) headers.set('X-Team-Id', auth.teamId);
	if (auth.role) headers.set('X-Role', auth.role);
}

function serviceHeaders(auth: AuthResult, c: any): Headers {
	const headers = new Headers();
	headers.set('Content-Type', 'application/json');
	headers.set('X-Request-Id', c.get('requestId' as any) ?? '');
	setAuthHeaders(headers, auth);
	return headers;
}

async function proxyTo(
	c: any,
	url: string,
	headers: Headers,
	breaker: any,
): Promise<Response> {
	const init: RequestInit = {
		method: c.req.method,
		headers,
	};

	if (!['GET', 'HEAD'].includes(c.req.method)) {
		init.body = await c.req.text();
	}

	const res = await resilientFetch(url, init, breaker);

	// Stream response, preserve original Content-Type
	const contentType = res.headers.get('Content-Type') ?? 'application/json';
	return new Response(res.body, {
		status: res.status,
		headers: { 'Content-Type': contentType },
	});
}

export default gateway;
```

**Step 3: Verify lint passes**

Run: `cd simse-api && npm run lint`
Expected: No errors.

**Step 4: Commit**

```bash
git add simse-api/src/index.ts simse-api/src/routes/gateway.ts
git commit -m "feat(simse-api): rewrite gateway with JWT auth, circuit breakers, resilient fetch, and security middleware"
```

---

## Task 12: Auth — Update Logout to Revoke Refresh Tokens

**Files:**
- Modify: `simse-auth/src/routes/auth.ts` (logout handler)

**Step 1: Update logout handler**

The current logout handler (around line 133) deletes the session. Update it to also accept a refresh token and revoke its family:

Replace the logout handler with:

```typescript
// POST /auth/logout (requires auth — called via gateway with X-User-Id)
auth.post('/logout', async (c) => {
	const sessionId = c.req.header('X-Session-Id');
	const db = c.env.DB;

	if (sessionId) {
		await deleteSession(db, sessionId);
	}

	// Also revoke refresh token if provided
	try {
		const body = await c.req.json<{ refreshToken?: string }>();
		if (body.refreshToken?.startsWith('rt_')) {
			const encoder = new TextEncoder();
			const data = encoder.encode(body.refreshToken);
			const hashBuffer = await crypto.subtle.digest('SHA-256', data);
			const hashArray = new Uint8Array(hashBuffer);
			const tokenHash = btoa(String.fromCharCode(...hashArray));

			const row = await db
				.prepare('SELECT family_id FROM refresh_tokens WHERE token_hash = ?')
				.bind(tokenHash)
				.first<{ family_id: string }>();

			if (row) {
				await revokeFamily(db, row.family_id);
			}
		}
	} catch {
		// Body parsing may fail if no body sent — that's ok
	}

	return c.json({ data: { ok: true } });
});
```

**Step 2: Verify lint passes**

Run: `cd simse-auth && npm run lint`
Expected: No errors.

**Step 3: Commit**

```bash
git add simse-auth/src/routes/auth.ts
git commit -m "feat(simse-auth): revoke refresh token family on logout"
```

---

## Task 13: Verify Everything Builds

**Step 1: Build auth service**

Run: `cd simse-auth && npx wrangler deploy --dry-run --outdir dist`
Expected: Build succeeds.

**Step 2: Build API gateway**

Run: `cd simse-api && npx wrangler deploy --dry-run --outdir dist`
Expected: Build succeeds.

**Step 3: Lint both services**

Run: `cd simse-auth && npm run lint && cd ../simse-api && npm run lint`
Expected: No errors.

**Step 4: Commit any remaining fixes**

If any lint/build issues arise, fix and commit.

---

## Summary of All New/Modified Files

### simse-auth (new/modified)
| File | Action |
|------|--------|
| `migrations/0002_refresh_tokens.sql` | Create |
| `src/lib/jwt.ts` | Create |
| `src/lib/refresh-token.ts` | Create |
| `src/types.ts` | Modify (add SECRETS) |
| `src/schemas.ts` | Modify (add refresh/revoke schemas) |
| `src/routes/auth.ts` | Modify (JWT + refresh in login/register/2fa, add refresh/revoke endpoints, update validate/logout) |
| `src/routes/users.ts` | Modify (delete refresh_tokens on account deletion) |
| `wrangler.toml` | Modify (add SECRETS binding) |

### simse-api (new/modified)
| File | Action |
|------|--------|
| `src/lib/jwt.ts` | Create |
| `src/lib/circuit-breaker.ts` | Create |
| `src/lib/resilient-fetch.ts` | Create |
| `src/lib/rate-limiter.ts` | Create |
| `src/middleware/rate-limit.ts` | Create |
| `src/middleware/security.ts` | Create |
| `src/middleware/secrets.ts` | Modify (add jwtSecret) |
| `src/types.ts` | Modify (add jwtSecret to ApiSecrets) |
| `src/index.ts` | Modify (add middleware, circuit breakers, enhanced health) |
| `src/routes/gateway.ts` | Rewrite (JWT auth, resilient fetch, circuit breakers) |
