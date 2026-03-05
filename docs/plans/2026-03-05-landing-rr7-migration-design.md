# Migrate simse-landing to React Router v7 Framework Mode

**Date:** 2026-03-05
**Status:** Approved

## Overview

Replace plain Vite + React SPA setup with React Router v7 framework mode on Cloudflare Pages. Move Cloudflare Functions (waitlist API, unsubscribe) into RR7 loaders/actions.

## Changes

- Replace `@vitejs/plugin-react` with `@react-router/dev` + `@react-router/cloudflare`
- Replace `src/main.tsx` + `src/router.tsx` + `src/App.tsx` with RR7 file-based routing (`app/routes/`)
- Move `functions/api/waitlist.ts` → action on the home route
- Move `functions/unsubscribe.ts` → `app/routes/unsubscribe.tsx` (loader)
- Move `functions/lib/` and `functions/emails/` into `app/lib/`
- `vite.config.ts` uses `@react-router/cloudflare` plugin
- Add `react-router.config.ts`, `app/root.tsx`, `app/entry.server.tsx`
- D1 binding via RR7 context instead of Cloudflare Functions env
- Delete `functions/`, `src/`, `index.html`

## Route Structure

| Route | File | Purpose |
|-------|------|---------|
| `/` | `app/routes/home.tsx` | Landing page + waitlist action |
| `/unsubscribe` | `app/routes/unsubscribe.tsx` | Unsubscribe loader + confirmation page |

## Unchanged

- All React components (just relocated)
- Email templates and validation logic (just relocated)
- CSS, fonts, animations
- D1 database and migrations
- Public assets
- Cloudflare Pages deployment
