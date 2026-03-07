# simse-analytics Service Design

## Overview

A new Cloudflare Worker that centralizes all analytics and audit event processing behind a single queue-based service. All 8 existing services stop writing directly to the Analytics Engine dataset and instead produce messages to `ANALYTICS_QUEUE`. The analytics service is the sole writer to the Analytics Engine and the sole processor of audit events (persisted in D1).

## Architecture

```
simse-api ──┐
simse-auth ─┤
simse-cdn ──┤
simse-cloud ┼──▶ ANALYTICS_QUEUE ──▶ simse-analytics ──▶ Analytics Engine (simse-analytics dataset)
simse-mailer┤                                        └──▶ D1 (audit_events table)
simse-pay ──┤
simse-land ─┤
simse-stat ─┘
```

## Message Types

Two message types on the queue:

### Datapoint (replaces direct ANALYTICS.writeDataPoint)

```typescript
{
  type: 'datapoint';
  service: string;          // e.g. 'simse-api'
  method: string;
  path: string;
  status: number;
  userId?: string;
  teamId?: string;
  country?: string;
  city?: string;
  continent?: string;
  userAgent?: string;
  referer?: string;
  contentType?: string;
  cfRay?: string;
  latencyMs: number;
  requestSize: number;
  responseSize: number;
  colo?: number;
}
```

### Audit Event (moves off COMMS_QUEUE)

```typescript
{
  type: 'audit';
  action: string;           // e.g. 'password.changed'
  userId: string;
  timestamp: string;        // ISO 8601
  [key: string]: string;    // additional metadata
}
```

## Database Schema (D1)

```sql
CREATE TABLE audit_events (
  id TEXT PRIMARY KEY,
  action TEXT NOT NULL,
  user_id TEXT NOT NULL,
  metadata TEXT,            -- JSON blob for extra fields
  created_at TEXT DEFAULT (datetime('now'))
);
CREATE INDEX idx_audit_user ON audit_events(user_id);
CREATE INDEX idx_audit_action ON audit_events(action);
CREATE INDEX idx_audit_created ON audit_events(created_at);
```

## Queue Handler

The `queue()` handler processes batches:

- **Datapoint messages**: Map to `ANALYTICS.writeDataPoint()` with the same blob/double schema used today, just centralized in one place
- **Audit messages**: Insert into `audit_events` D1 table, also write a datapoint to Analytics Engine with `indexes: ['audit']`

## Producer Changes (all 8 services)

Each service:

1. **wrangler.toml**: Replace `ANALYTICS` (AnalyticsEngineDataset) binding with `ANALYTICS_QUEUE` (Queue) producer binding
2. **Analytics middleware**: Replace `ANALYTICS.writeDataPoint({...})` with `ANALYTICS_QUEUE.send({type: 'datapoint', ...})`
3. **simse-auth additionally**: Change `sendAuditEvent` to send to `ANALYTICS_QUEUE` instead of `COMMS_QUEUE`

## HTTP Routes

- `GET /health` — health check
- `GET /audit/:userId` — fetch audit events for a user (service-auth gated)

## Error Handling

- Queue retries handled by CF Queues (3 retries by default)
- Analytics write failures logged but don't fail the batch
- Audit insert failures trigger retry (message stays on queue)

## Services Affected

| Service | Changes |
|---------|---------|
| simse-analytics | New service (worker, queue consumer, D1, Analytics Engine) |
| simse-api | Replace ANALYTICS binding with ANALYTICS_QUEUE, update middleware |
| simse-auth | Replace ANALYTICS with ANALYTICS_QUEUE, move audit from COMMS_QUEUE |
| simse-cdn | Replace ANALYTICS with ANALYTICS_QUEUE, update analytics write |
| simse-cloud | Replace ANALYTICS with ANALYTICS_QUEUE, update analytics write |
| simse-mailer | Replace ANALYTICS with ANALYTICS_QUEUE, update analytics writes, remove audit ack |
| simse-payments | Replace ANALYTICS with ANALYTICS_QUEUE, update middleware |
| simse-landing | Replace ANALYTICS with ANALYTICS_QUEUE, update analytics write |
| simse-status | Replace ANALYTICS with ANALYTICS_QUEUE, update analytics write |
