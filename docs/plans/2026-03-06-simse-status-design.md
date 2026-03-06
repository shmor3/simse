# simse-status Design

## Overview

A Cloudflare Pages app (React Router v7) at `status.simse.dev` that displays real-time health status for all 7 simse web services. A cron trigger checks each service every minute and stores results in D1. The page reads from the DB to show current status and 90-day uptime history.

## Services Monitored

| Service | Health URL | Expected Response |
|---------|-----------|-------------------|
| API Gateway | `https://api.simse.dev/health` | `{ ok: true }` |
| Auth | `https://auth.simse.dev/health` | `{ ok: true }` |
| Payments | internal worker URL `/health` | `{ ok: true }` |
| CDN | `https://cdn.simse.dev/health` | `ok` (text) |
| Mailer | internal worker URL `/health` | `{ ok: true }` |
| Cloud App | `https://app.simse.dev/health` | `{ ok: true }` |
| Landing | `https://simse.dev/health` | `{ ok: true }` |

## Architecture

- **Cron trigger** (every 1 min): Fetches all 7 `/health` endpoints in parallel, records status + latency in D1
- **D1 database**: Two tables - `services` (config) and `checks` (timestamped results)
- **Page loader**: Queries D1 for current status + 90-day history, renders SSR
- **Auto-purge**: Cron also deletes records older than 90 days

## Database Schema

```sql
CREATE TABLE services (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  url TEXT NOT NULL,
  expected_status INTEGER NOT NULL DEFAULT 200
);

CREATE TABLE checks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  service_id TEXT NOT NULL REFERENCES services(id),
  status TEXT NOT NULL, -- 'up', 'down', 'degraded'
  response_time_ms INTEGER,
  status_code INTEGER,
  error TEXT,
  checked_at TEXT NOT NULL DEFAULT (datetime('now')),
  FOREIGN KEY (service_id) REFERENCES services(id)
);

CREATE INDEX idx_checks_service_time ON checks(service_id, checked_at);
CREATE INDEX idx_checks_time ON checks(checked_at);
```

## UI

- Clean, minimal status page (GitHub Status style)
- Overall status banner (all operational / degraded / major outage)
- Per-service row: name, current status dot (green/yellow/red), response time, uptime % (90d)
- 90-day uptime bar chart per service (day-level granularity, green/yellow/red bars)

## Missing /health Endpoints

Need to add `/health` to:
- **simse-cloud**: Add a health API route
- **simse-landing**: Add a health handler in worker.ts

## Stack

- React Router v7 + `@react-router/cloudflare`
- D1 database for check history
- Tailwind CSS (consistent with simse-cloud/simse-landing)
- Cron trigger via wrangler.toml

## Health Check Logic

A service is considered:
- **up**: responds with expected status code within 10s
- **degraded**: responds with expected status code but takes > 5s
- **down**: does not respond, returns unexpected status code, or times out after 10s

## Cron Cleanup

Each cron invocation also runs:
```sql
DELETE FROM checks WHERE checked_at < datetime('now', '-90 days');
```
