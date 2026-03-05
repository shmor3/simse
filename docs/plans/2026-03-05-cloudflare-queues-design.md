# Cloudflare Queues for Async Service Communication

**Date:** 2026-03-05
**Status:** Approved

## Overview

Add Cloudflare Queues between producers (simse-auth, simse-api, simse-landing) and simse-mailer for fire-and-forget operations (emails, notifications). Synchronous calls (auth validation, payments, reading notifications) stay as direct HTTP.

## Queues

| Queue | Producer | Consumer | Message types |
|-------|----------|----------|---------------|
| `simse-auth-comms` | simse-auth | simse-mailer | emails + notifications |
| `simse-api-comms` | simse-api | simse-mailer | emails + notifications |
| `simse-landing-comms` | simse-landing | simse-mailer | emails |

## Message Format

```typescript
// Email message
{ type: 'email', template: string, to: string, props: Record<string, string> }

// Notification message
{ type: 'notification', userId: string, kind: string, title: string, body: string, link?: string }
```

## What Uses Queues (fire-and-forget)

- simse-auth → simse-mailer: verify-email, two-factor, reset-password, team-invite, role-change
- simse-api → simse-mailer: notification creates (POST /notifications from simse-cloud)
- simse-landing → simse-mailer: waitlist-welcome email on signup

## What Stays as Direct HTTP (synchronous)

- simse-api → simse-auth: token validation, login, register, user/team management
- simse-api → simse-payments: subscription, credits, usage
- simse-api → simse-mailer: GET/PUT /notifications (reading data needs a response)

## Wrangler Config

**Producer (e.g., simse-auth):**
```toml
[[queues.producers]]
queue = "simse-auth-comms"
binding = "COMMS_QUEUE"
```

**Consumer (simse-mailer):**
```toml
[[queues.consumers]]
queue = "simse-auth-comms"

[[queues.consumers]]
queue = "simse-api-comms"

[[queues.consumers]]
queue = "simse-landing-comms"
```

## simse-mailer queue() handler

simse-mailer exports both `fetch` (HTTP) and `queue` (batch consumer) handlers. The queue handler renders + sends emails and stores notifications in D1.
