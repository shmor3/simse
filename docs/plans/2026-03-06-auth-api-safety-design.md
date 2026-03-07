# Auth Service Improvements & API Gateway Safety Features

**Date:** 2026-03-06
**Status:** Approved

## Overview

Add refresh token rotation with JWT access tokens to the auth service, and add circuit breaker, timeout/retry, rate limiting, request validation, and security hardening to the API gateway.

## 1. Auth Service — Refresh Token Flow

### New Database Table: `refresh_tokens`

| Column | Type | Notes |
|--------|------|-------|
| `id` | TEXT PK | UUID |
| `user_id` | TEXT FK | References users |
| `family_id` | TEXT | Groups tokens from same login — reuse detection |
| `token_hash` | TEXT | SHA-256 hash of the refresh token |
| `expires_at` | TEXT | 30 days from creation |
| `revoked` | INTEGER | 0/1 flag |
| `created_at` | TEXT | ISO timestamp |

Indexes: `idx_refresh_tokens_user`, `idx_refresh_tokens_hash`, `idx_refresh_tokens_family`.

### Access Tokens

JWTs signed with HMAC-SHA256, 15-minute TTL.

Payload: `{ sub, tid, role, sid, exp, iat }` (userId, teamId, role, sessionId, expiry, issuedAt).

Signing secret stored in Cloudflare Secrets Store, shared with API gateway.

### New Auth Endpoints

- `POST /auth/refresh` — Takes refresh token, validates family, returns new access JWT + new refresh token. Marks old refresh token as revoked.
- `POST /auth/revoke` — Revokes a refresh token family (logout).

### Reuse Detection

If a revoked refresh token is presented, revoke the entire family (that session only). Return 401 with `TOKEN_REUSED` code.

### Login/Register Changes

Return `{ accessToken, refreshToken, expiresIn: 900 }` instead of just a session token.

## 2. API Gateway — JWT Validation

- Gateway reads `JWT_SECRET` from Secrets Store at startup (added to `secretsMiddleware`).
- Protected routes verify JWT signature + expiry locally using WebCrypto HMAC-SHA256. No HTTP call to auth service needed.
- Extracts `sub`, `tid`, `role`, `sid` from payload and sets `X-User-Id`, `X-Team-Id`, `X-Role`, `X-Session-Id` headers.
- If JWT is expired but structurally valid, return 401 with code `TOKEN_EXPIRED` (signals client to call `/auth/refresh`).
- Tokens starting with `sk_` still validated via HTTP call to auth service.
- `/auth/validate` endpoint kept for backwards compatibility and API key validation.

## 3. API Gateway — Circuit Breaker

Per-backend circuit breakers (auth, payments, mailer). In-memory state.

### States

| State | Behavior |
|-------|----------|
| Closed | Normal operation. Track failure count. |
| Open | Fail fast with 503 + `Retry-After`. No requests to backend. |
| Half-open | Allow 1 probe request. Success -> closed. Failure -> open. |

### Thresholds

- Open after: 5 consecutive failures within 60s window
- Cool-down: 30s before transitioning to half-open
- Reset: Single success in half-open resets to closed

### Degraded Mode (circuit open)

- `/health` still returns 200 with degraded backend status
- JWT validation still works locally
- Public auth routes return 503 with `Retry-After: 30`
- Routes to healthy backends continue working

## 4. API Gateway — Timeout, Retry & Backoff

- All proxy calls: `AbortController` + 5s timeout. Timeout counts as circuit breaker failure.
- Max 2 retries (3 total attempts). Delays: 1s, 2s (exponential).
- Only retry GET requests and token validation. Never retry POST/PUT/DELETE.
- Only retry on network errors and 502/503/504 responses. Never retry 4xx.
- Add +/-20% random jitter to backoff delays.
- Single `resilientFetch(url, options, circuitBreaker)` function replaces raw `fetch` in `proxyTo`.

## 5. API Gateway — Rate Limiting

### Per-IP (public routes)

| Route group | Limit | Window |
|-------------|-------|--------|
| `/auth/login`, `/auth/register` | 10 req | 1 min |
| `/auth/reset-password`, `/auth/new-password` | 5 req | 1 min |
| `/auth/2fa`, `/auth/verify-email` | 10 req | 1 min |
| `/auth/refresh` | 30 req | 1 min |

### Per-user (protected routes)

| Route group | Limit | Window |
|-------------|-------|--------|
| `/users/*`, `/teams/*`, `/api-keys/*` | 60 req | 1 min |
| `/payments/*` | 30 req | 1 min |
| `/notifications/*` | 20 req | 1 min |

In-memory sliding window counter. Keyed by IP (public) or userId (protected).

Response: `429 Too Many Requests` with `Retry-After`, `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` headers.

## 6. API Gateway — Request Validation & Security

### Request Validation

- Validate `Content-Type: application/json` on POST/PUT requests.
- Reject requests over 1MB (`Content-Length` check).
- Validate required headers before proxying.

### Correlation IDs

- Generate `X-Request-Id` (UUID) on every request if not provided.
- Pass through to all backend services.
- Include in all error responses.

### Security Headers

- `X-Content-Type-Options: nosniff`
- `X-Request-Id: <correlation-id>`
- Strip leaked backend headers (`Server`, `X-Powered-By`).

### Proxy Hardening

- Preserve original `Content-Type` from backend response (stop forcing `application/json`).
- Stream response body instead of buffering as text.

### Health Endpoint Enhancement

```json
{ "ok": true, "services": { "auth": "healthy", "payments": "degraded", "mailer": "open" } }
```
