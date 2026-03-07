# Cloudflare Workers Analytics Engine

**Date:** 2026-03-06
**Status:** Approved

## Overview

Add Cloudflare Workers Analytics Engine to all 7 Cloudflare Workers to track request volume, latency, user behavior, and geographic distribution during preview.

## Approach

Per-worker inline analytics — no shared package. Each worker writes a data point on every request to a single shared dataset (`simse-analytics`), differentiated by the `service` blob/index.

## Workers

| Service | Framework | Type |
|---------|-----------|------|
| simse-api | Hono | Worker |
| simse-cdn | Raw fetch | Worker |
| simse-auth | Hono | Worker |
| simse-payments | Hono | Worker |
| simse-mailer | Hono | Worker |
| simse-landing | React Router | Pages |
| simse-app | React Router | Pages |

## Changes per worker type

**Hono workers** (simse-api, simse-auth, simse-payments, simse-mailer):
- Add `ANALYTICS: AnalyticsEngineDataset` binding to `wrangler.toml` and `Env` type
- Add analytics middleware: captures start time, awaits `next()`, writes data point

**Raw fetch worker** (simse-cdn):
- Add `ANALYTICS` binding to `wrangler.toml` and `Env` type
- Wrap existing fetch handler to capture timing and write data point after response

**Pages workers** (simse-app, simse-landing):
- Add `ANALYTICS` binding to `wrangler.toml` and env type
- Add `onRequest` middleware in worker/server entry that writes data point

## Dataset

Single dataset: `simse-analytics`. All 7 workers write to it. The `service` index differentiates them.

## Data point schema

| Type | Position | Field |
|------|----------|-------|
| **Index** | 1 | service |
| **Blob 1** | | method |
| **Blob 2** | | path |
| **Blob 3** | | status |
| **Blob 4** | | service |
| **Blob 5** | | userId |
| **Blob 6** | | teamId |
| **Blob 7** | | country |
| **Blob 8** | | city |
| **Blob 9** | | continent |
| **Blob 10** | | userAgent (truncated 256 chars) |
| **Blob 11** | | referer |
| **Blob 12** | | contentType |
| **Blob 13** | | cfRay |
| **Double 1** | | latencyMs |
| **Double 2** | | statusCode |
| **Double 3** | | requestSize |
| **Double 4** | | responseSize |
| **Double 5** | | cfColo |

## What this gives us

- **Who**: userId, teamId
- **Where**: country, city, continent, colo
- **What**: method, path, status, contentType
- **How**: latency, request/response sizes, userAgent
- **Tracing**: cfRay

## Notes

- userId/teamId are null for unauthenticated requests and Pages workers
- cfColo is a numeric datacenter ID from `cf.colo`
- userAgent truncated to 256 chars to fit blob limits
- Analytics Engine has no cost for writes; query via GraphQL or dashboard
