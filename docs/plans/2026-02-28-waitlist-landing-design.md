# simse Waitlist Landing Page — Design

## Summary

Single-page waitlist SPA for simse, hosted on Cloudflare Pages with a Pages Function backend and D1 database for email collection.

## Decisions

- **Framework:** React + Vite + Tailwind CSS
- **Backend:** Cloudflare Pages Functions (single POST endpoint)
- **Storage:** Cloudflare D1 (SQLite)
- **Scope:** Email-only waitlist (no name/role/company)
- **Architecture:** Single deploy — frontend + API in `simse-landing/`
- **Monorepo:** Standalone folder, not added to Bun workspaces

## Project Structure

```
simse-landing/
  package.json
  wrangler.toml
  vite.config.ts
  tailwind.config.ts
  tsconfig.json
  index.html
  src/
    main.tsx
    App.tsx
    components/
      Hero.tsx
      Features.tsx
      Footer.tsx
      WaitlistForm.tsx
  functions/
    api/
      waitlist.ts
  migrations/
    0001_create_waitlist.sql
```

## Frontend

- Single page, scroll sections (no router)
- Hero: headline, description, email waitlist form
- Features: 4-6 card grid (ACP, MCP, vector memory, agentic loop, etc.)
- Footer: GitHub link, MIT license
- WaitlistForm: controlled input, POST to `/api/waitlist`, loading/success/error states

## Backend

`functions/api/waitlist.ts`:
- POST `{ email: string }`
- Validate email format
- Insert into D1 `waitlist` table (ON CONFLICT for duplicates)
- Return `{ success: true }` or error

## Database

```sql
CREATE TABLE waitlist (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  email TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

## Deployment

- `wrangler.toml` configures Pages project + D1 binding
- `wrangler d1 create simse-waitlist` → `wrangler d1 migrations apply`
- Vite builds to `dist/`, Cloudflare Pages deploys `dist/` + `functions/`
