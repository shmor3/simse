# simse Waitlist Landing Page — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a waitlist landing page SPA for simse, deployed on Cloudflare Pages with a D1-backed email signup endpoint.

**Architecture:** React + Vite + Tailwind v4 frontend in `simse-landing/`. Cloudflare Pages Functions provide a single `POST /api/waitlist` endpoint backed by D1 SQLite. Single deploy — no separate Worker.

**Tech Stack:** React 19, Vite, Tailwind CSS v4 (`@tailwindcss/vite`), Cloudflare Pages Functions, Cloudflare D1, TypeScript, wrangler

---

### Task 1: Scaffold project and install dependencies

**Files:**
- Create: `simse-landing/package.json`
- Create: `simse-landing/tsconfig.json`
- Create: `simse-landing/vite.config.ts`
- Create: `simse-landing/index.html`
- Create: `simse-landing/wrangler.toml`
- Create: `simse-landing/src/index.css`
- Create: `simse-landing/src/main.tsx`

**Step 1: Create `simse-landing/package.json`**

```json
{
  "name": "simse-landing",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "deploy": "wrangler pages deploy dist",
    "cf-typegen": "wrangler types"
  },
  "dependencies": {
    "react": "^19.2.0",
    "react-dom": "^19.2.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^5.1.4",
    "@tailwindcss/vite": "^4.2.1",
    "tailwindcss": "^4.2.1",
    "typescript": "^5.7.0",
    "vite": "^7.3.0",
    "wrangler": "^4.0.0"
  }
}
```

**Step 2: Create `simse-landing/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "jsx": "react-jsx",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true
  },
  "include": ["src"]
}
```

**Step 3: Create `simse-landing/vite.config.ts`**

```ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
});
```

**Step 4: Create `simse-landing/index.html`**

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>simse — Orchestrate AI Workflows</title>
    <link rel="stylesheet" href="/src/index.css" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

**Step 5: Create `simse-landing/src/index.css`**

```css
@import "tailwindcss";
```

**Step 6: Create `simse-landing/src/main.tsx`**

```tsx
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import App from './App';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
```

**Step 7: Create `simse-landing/wrangler.toml`**

```toml
name = "simse-landing"
pages_build_output_dir = "./dist"
compatibility_date = "2026-02-28"

[[d1_databases]]
binding = "DB"
database_name = "simse-waitlist"
database_id = "<placeholder-create-with-wrangler-d1-create>"
```

**Step 8: Install dependencies**

Run: `cd simse-landing && npm install`

**Step 9: Verify build scaffolding works**

Create a minimal `simse-landing/src/App.tsx`:

```tsx
export default function App() {
  return <h1>simse</h1>;
}
```

Run: `cd simse-landing && npm run build`
Expected: Successful build, `dist/` created with `index.html` and JS bundle.

**Step 10: Commit**

```bash
git add simse-landing/
git commit -m "feat(simse-landing): scaffold project with Vite + React + Tailwind v4"
```

---

### Task 2: D1 migration and Pages Function

**Files:**
- Create: `simse-landing/migrations/0001_create_waitlist.sql`
- Create: `simse-landing/functions/api/waitlist.ts`

**Step 1: Create the migration file `simse-landing/migrations/0001_create_waitlist.sql`**

```sql
CREATE TABLE IF NOT EXISTS waitlist (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  email TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

**Step 2: Create the Pages Function `simse-landing/functions/api/waitlist.ts`**

```ts
interface Env {
  DB: D1Database;
}

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export const onRequestPost: PagesFunction<Env> = async (context) => {
  let body: { email?: string };
  try {
    body = await context.request.json();
  } catch {
    return Response.json({ error: 'Invalid JSON' }, { status: 400 });
  }

  const email = body.email?.trim().toLowerCase();
  if (!email || !EMAIL_RE.test(email)) {
    return Response.json({ error: 'Invalid email' }, { status: 400 });
  }

  try {
    await context.env.DB.prepare(
      'INSERT INTO waitlist (email) VALUES (?) ON CONFLICT (email) DO NOTHING'
    )
      .bind(email)
      .run();
  } catch (err) {
    return Response.json({ error: 'Database error' }, { status: 500 });
  }

  return Response.json({ success: true });
};
```

**Step 3: Generate Cloudflare types**

Run: `cd simse-landing && npx wrangler types`
Expected: Generates a `worker-configuration.d.ts` file with `D1Database` and `PagesFunction` types.

Note: If `wrangler types` requires a valid `database_id`, you can skip this step locally and rely on the manual `Env` interface above.

**Step 4: Commit**

```bash
git add simse-landing/migrations/ simse-landing/functions/
git commit -m "feat(simse-landing): add D1 migration and waitlist POST endpoint"
```

---

### Task 3: WaitlistForm component

**Files:**
- Create: `simse-landing/src/components/WaitlistForm.tsx`

**Step 1: Create `simse-landing/src/components/WaitlistForm.tsx`**

```tsx
import { useState, type FormEvent } from 'react';

type Status = 'idle' | 'loading' | 'success' | 'error';

export default function WaitlistForm() {
  const [email, setEmail] = useState('');
  const [status, setStatus] = useState<Status>('idle');

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!email.trim()) return;

    setStatus('loading');
    try {
      const res = await fetch('/api/waitlist', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: email.trim() }),
      });
      if (!res.ok) throw new Error();
      setStatus('success');
    } catch {
      setStatus('error');
    }
  }

  if (status === 'success') {
    return (
      <p className="text-lg font-medium text-green-400">
        You're on the list! We'll be in touch.
      </p>
    );
  }

  return (
    <form onSubmit={handleSubmit} className="flex flex-col sm:flex-row gap-3 w-full max-w-md">
      <input
        type="email"
        required
        placeholder="you@example.com"
        value={email}
        onChange={(e) => setEmail(e.target.value)}
        className="flex-1 rounded-lg border border-zinc-700 bg-zinc-900 px-4 py-3 text-white placeholder:text-zinc-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
      />
      <button
        type="submit"
        disabled={status === 'loading'}
        className="rounded-lg bg-indigo-600 px-6 py-3 font-semibold text-white hover:bg-indigo-500 disabled:opacity-50 transition-colors cursor-pointer"
      >
        {status === 'loading' ? 'Joining...' : 'Join Waitlist'}
      </button>
      {status === 'error' && (
        <p className="text-sm text-red-400 sm:col-span-2">Something went wrong. Try again.</p>
      )}
    </form>
  );
}
```

**Step 2: Verify it compiles**

Run: `cd simse-landing && npx tsc --noEmit`
Expected: No errors.

**Step 3: Commit**

```bash
git add simse-landing/src/components/WaitlistForm.tsx
git commit -m "feat(simse-landing): add WaitlistForm component"
```

---

### Task 4: Hero component

**Files:**
- Create: `simse-landing/src/components/Hero.tsx`

**Step 1: Create `simse-landing/src/components/Hero.tsx`**

Use the `frontend-design` skill for this component. The hero should:
- Dark background (zinc-950)
- Large headline: "Orchestrate AI Workflows"
- Subtext: Brief description of simse (modular pipeline framework, ACP + MCP + vector memory)
- Embed the `<WaitlistForm />` component below the text
- Centered layout, generous vertical padding

```tsx
import WaitlistForm from './WaitlistForm';

export default function Hero() {
  return (
    <section className="flex min-h-[80vh] flex-col items-center justify-center px-6 py-24 text-center">
      <h1 className="text-5xl sm:text-7xl font-bold tracking-tight text-white">
        Orchestrate AI Workflows
      </h1>
      <p className="mt-6 max-w-2xl text-lg text-zinc-400">
        simse is a modular pipeline framework for multi-step AI workflows.
        Connect to AI backends via ACP, expose tools via MCP, and persist
        knowledge with vector memory.
      </p>
      <div className="mt-10">
        <WaitlistForm />
      </div>
    </section>
  );
}
```

**Step 2: Commit**

```bash
git add simse-landing/src/components/Hero.tsx
git commit -m "feat(simse-landing): add Hero component"
```

---

### Task 5: Features component

**Files:**
- Create: `simse-landing/src/components/Features.tsx`

**Step 1: Create `simse-landing/src/components/Features.tsx`**

Use the `frontend-design` skill for this component. Grid of 6 feature cards:

```tsx
const features = [
  { title: 'ACP Client', desc: 'Connect to AI backends via Agent Client Protocol. Streaming, sessions, permissions.' },
  { title: 'MCP Server', desc: 'Expose and consume tools, resources, and prompts via Model Context Protocol.' },
  { title: 'Agentic Loop', desc: 'Multi-turn tool-use loop with auto-compaction, stream retry, and doom-loop detection.' },
  { title: 'Vector Memory', desc: 'File-backed vector store with cosine search, deduplication, and compression.' },
  { title: 'Virtual Filesystem', desc: 'In-memory filesystem with history, diffing, snapshots, and disk persistence.' },
  { title: 'Resilience', desc: 'Circuit breaker, health monitor, retry with exponential backoff and jitter.' },
];

export default function Features() {
  return (
    <section className="px-6 py-24">
      <h2 className="text-3xl font-bold text-white text-center mb-16">
        Everything you need
      </h2>
      <div className="mx-auto grid max-w-5xl grid-cols-1 gap-8 sm:grid-cols-2 lg:grid-cols-3">
        {features.map((f) => (
          <div key={f.title} className="rounded-xl border border-zinc-800 bg-zinc-900/50 p-6">
            <h3 className="text-lg font-semibold text-white">{f.title}</h3>
            <p className="mt-2 text-sm text-zinc-400">{f.desc}</p>
          </div>
        ))}
      </div>
    </section>
  );
}
```

**Step 2: Commit**

```bash
git add simse-landing/src/components/Features.tsx
git commit -m "feat(simse-landing): add Features component"
```

---

### Task 6: Footer component

**Files:**
- Create: `simse-landing/src/components/Footer.tsx`

**Step 1: Create `simse-landing/src/components/Footer.tsx`**

```tsx
export default function Footer() {
  return (
    <footer className="border-t border-zinc-800 px-6 py-8 text-center text-sm text-zinc-500">
      <a
        href="https://github.com/restaadiputra/simse"
        target="_blank"
        rel="noopener noreferrer"
        className="hover:text-zinc-300 transition-colors"
      >
        GitHub
      </a>
      <span className="mx-3">·</span>
      <span>MIT License</span>
    </footer>
  );
}
```

Note: Replace the GitHub URL with the actual repository URL.

**Step 2: Commit**

```bash
git add simse-landing/src/components/Footer.tsx
git commit -m "feat(simse-landing): add Footer component"
```

---

### Task 7: Wire up App.tsx and polish

**Files:**
- Modify: `simse-landing/src/App.tsx`
- Modify: `simse-landing/src/index.css` (add base body styles)

**Step 1: Update `simse-landing/src/index.css`**

```css
@import "tailwindcss";

body {
  background-color: #09090b; /* zinc-950 */
  color: #fafafa;            /* zinc-50 */
  font-family: system-ui, -apple-system, sans-serif;
}
```

**Step 2: Update `simse-landing/src/App.tsx`**

Use the `frontend-design` skill for the final composition. Wire all components together:

```tsx
import Hero from './components/Hero';
import Features from './components/Features';
import Footer from './components/Footer';

export default function App() {
  return (
    <div className="min-h-screen">
      <Hero />
      <Features />
      <Footer />
    </div>
  );
}
```

**Step 3: Verify full build**

Run: `cd simse-landing && npm run build`
Expected: Successful build, `dist/` contains `index.html` and bundled JS/CSS.

**Step 4: Commit**

```bash
git add simse-landing/src/App.tsx simse-landing/src/index.css
git commit -m "feat(simse-landing): wire up App with Hero, Features, Footer"
```

---

### Task 8: Local dev verification

**Step 1: Start local dev server**

Run: `cd simse-landing && npx wrangler pages dev`

This starts a local server that serves the Vite build, runs Pages Functions locally, and creates a local D1 database.

Note: You may need to run `npm run build` first since `wrangler pages dev` serves from `dist/`. Alternatively use `npx wrangler pages dev -- npx vite` for HMR.

**Step 2: Verify the page loads**

Open `http://localhost:8788` in a browser. Check:
- Hero section renders with headline and waitlist form
- Features grid shows 6 cards
- Footer shows GitHub link

**Step 3: Test the waitlist endpoint**

First apply the migration locally:
```bash
cd simse-landing && npx wrangler d1 execute simse-waitlist --local --file=./migrations/0001_create_waitlist.sql
```

Then test the endpoint:
```bash
curl -X POST http://localhost:8788/api/waitlist \
  -H "Content-Type: application/json" \
  -d '{"email":"test@example.com"}'
```

Expected: `{"success":true}`

Test duplicate:
```bash
curl -X POST http://localhost:8788/api/waitlist \
  -H "Content-Type: application/json" \
  -d '{"email":"test@example.com"}'
```

Expected: `{"success":true}` (ON CONFLICT DO NOTHING — no error)

Test invalid email:
```bash
curl -X POST http://localhost:8788/api/waitlist \
  -H "Content-Type: application/json" \
  -d '{"email":"not-an-email"}'
```

Expected: `{"error":"Invalid email"}` with 400 status.

**Step 4: Commit any fixes**

If any fixes were needed, commit them.

---

### Task 9: Final production build and deploy prep

**Step 1: Run production build**

Run: `cd simse-landing && npm run build`
Expected: Clean build with no warnings.

**Step 2: Add `.gitignore`**

Create `simse-landing/.gitignore`:

```
node_modules/
dist/
.wrangler/
worker-configuration.d.ts
```

**Step 3: Final commit**

```bash
git add simse-landing/.gitignore
git commit -m "chore(simse-landing): add gitignore"
```

**Step 4: Deploy instructions (manual)**

To deploy to Cloudflare Pages:

```bash
# Create the D1 database (one-time)
cd simse-landing && npx wrangler d1 create simse-waitlist

# Update wrangler.toml with the returned database_id

# Apply migration to production D1
npx wrangler d1 migrations apply simse-waitlist --remote

# Deploy
npx wrangler pages deploy dist
```
