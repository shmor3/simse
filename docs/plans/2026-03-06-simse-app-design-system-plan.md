# simse-app Design System & Dashboard Redesign — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rename simse-app to simse-app, create a formal design system in simse-brand, and redesign the dashboard with an icon rail + contextual nav panel + Claude-web-style chat interface.

**Architecture:** Three-column layout (IconRail 56px + NavPanel 220px + Main Content). Design tokens live in simse-brand as CSS custom properties, consumed by simse-app via Tailwind's @theme. Chat interface uses bottom-pinned input with markdown message bubbles. Settings gains Devices and Remotes tabs.

**Tech Stack:** React 19, React Router 7, Tailwind CSS 4, Cloudflare Pages, Biome, TypeScript 5.7

**Design doc:** `docs/plans/2026-03-06-simse-app-design-system.md`

---

### Task 1: Create Design System Tokens in simse-brand

**Files:**
- Create: `simse-brand/design-system/tokens.css`
- Create: `simse-brand/design-system/tokens.ts`
- Create: `simse-brand/design-system/README.md`
- Create: `simse-brand/design-system/components.md`

**Step 1: Create tokens.css**

Source of truth for all design tokens as CSS custom properties. Values come from `simse-brand/guidelines/brand-guide.md` and existing usage in `simse-app/app/styles/app.css`.

```css
:root {
  /* Colors — Primary */
  --color-emerald: #34d399;
  --color-dark: #0a0a0b;
  --color-white: #ffffff;

  /* Colors — Semantic */
  --color-success: #34d399;
  --color-error: #ff6568;
  --color-warning: #fbbf24;
  --color-info: #60a5fa;

  /* Colors — Zinc scale */
  --color-zinc-50: #fafafa;
  --color-zinc-100: #f4f4f5;
  --color-zinc-200: #e4e4e7;
  --color-zinc-300: #d4d4d8;
  --color-zinc-400: #a1a1aa;
  --color-zinc-500: #71717a;
  --color-zinc-600: #52525b;
  --color-zinc-700: #3f3f46;
  --color-zinc-800: #27272a;
  --color-zinc-900: #18181b;
  --color-zinc-950: #09090b;

  /* Typography — Families */
  --font-sans: 'DM Sans Variable', system-ui, sans-serif;
  --font-mono: 'Space Mono', ui-monospace, monospace;

  /* Typography — Scale (size / weight / tracking / line-height) */
  --text-h1-size: 64px;
  --text-h1-weight: 700;
  --text-h1-tracking: -0.02em;
  --text-h1-leading: 1.1;

  --text-h2-size: 36px;
  --text-h2-weight: 700;
  --text-h2-tracking: -0.02em;
  --text-h2-leading: 1.2;

  --text-h3-size: 24px;
  --text-h3-weight: 600;
  --text-h3-tracking: -0.01em;
  --text-h3-leading: 1.3;

  --text-body-size: 16px;
  --text-body-weight: 400;
  --text-body-tracking: normal;
  --text-body-leading: 1.5;

  --text-small-size: 14px;
  --text-small-weight: 400;
  --text-small-tracking: normal;
  --text-small-leading: 1.5;

  --text-label-size: 11px;
  --text-label-weight: 700;
  --text-label-tracking: 0.1em;
  --text-label-transform: uppercase;
  --text-label-family: var(--font-mono);

  /* Spacing — Border Radius */
  --radius-sm: 6px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-full: 9999px;

  /* Layout */
  --rail-width: 56px;
  --nav-width: 220px;
  --header-height: 56px;

  /* Animations — Durations */
  --duration-fast: 200ms;
  --duration-normal: 500ms;
  --duration-slow: 600ms;

  /* Animations — Easings */
  --ease-out: cubic-bezier(0.16, 1, 0.3, 1);
  --ease-default: ease-out;
}
```

**Step 2: Create tokens.ts**

TypeScript export of the same values for use in JS (e.g., canvas rendering in DotGrid).

```typescript
export const colors = {
  emerald: '#34d399',
  dark: '#0a0a0b',
  white: '#ffffff',
  success: '#34d399',
  error: '#ff6568',
  warning: '#fbbf24',
  info: '#60a5fa',
  zinc: {
    50: '#fafafa',
    100: '#f4f4f5',
    200: '#e4e4e7',
    300: '#d4d4d8',
    400: '#a1a1aa',
    500: '#71717a',
    600: '#52525b',
    700: '#3f3f46',
    800: '#27272a',
    900: '#18181b',
    950: '#09090b',
  },
} as const;

export const fonts = {
  sans: "'DM Sans Variable', system-ui, sans-serif",
  mono: "'Space Mono', ui-monospace, monospace",
} as const;

export const radius = {
  sm: '6px',
  md: '8px',
  lg: '12px',
  full: '9999px',
} as const;

export const layout = {
  railWidth: 56,
  navWidth: 220,
  headerHeight: 56,
} as const;

export const duration = {
  fast: 200,
  normal: 500,
  slow: 600,
} as const;
```

**Step 3: Create README.md**

Brief overview explaining the design system structure, how to consume tokens, and the relationship between tokens.css (source of truth) and simse-app's Tailwind theme.

**Step 4: Create components.md**

Document each UI component's variants, props, and usage. Reference existing components: Button (4 variants), Card (accent option), Badge (5 variants), Avatar (3 sizes), Input (label/error/icon), Modal, StatCard, ProgressBar, CodeInput. This is a reference doc, not Storybook.

**Step 5: Commit**

```bash
git add simse-brand/design-system/
git commit -m "feat(brand): add design system tokens and component specs"
```

---

### Task 2: Rename simse-app to simse-app

**Files:**
- Rename: `simse-app/` → `simse-app/`
- Modify: `simse-app/package.json` — name field
- Modify: `simse-app/wrangler.toml` — name field, env names
- Modify: `simse-app/moon.yml` — no changes needed (just language/tags)
- Modify: `CLAUDE.md` — all references to simse-app
- Modify: `.github/workflows/ci.yml` — deploy paths
- Modify: `deployment/setup.sh` — references
- Modify: `.github/dependabot.yml` — directory path

**Step 1: Rename directory**

```bash
mv simse-app simse-app
```

**Step 2: Update simse-app/package.json**

Change `"name": "simse-app"` to `"name": "simse-app"`.

**Step 3: Update simse-app/wrangler.toml**

- Top-level `name = "simse-app"` → `name = "simse-app"`
- `[env.staging]` name → `"simse-app-staging"`
- `[env.production]` name → `"simse-app"`
- `APP_URL` values stay the same (app.simse.dev)

**Step 4: Update CLAUDE.md**

Replace all occurrences of `simse-app` with `simse-app`. This appears in:
- Repository layout tree
- TypeScript services description
- CDN Worker section references
- Formatting section
- Analytics Engine section

**Step 5: Update .github/workflows/ci.yml**

In the deploy job, find the simse-app Pages deploy step and change:
- `working-directory: simse-app` → `working-directory: simse-app`
- Any references in deploy commands

**Step 6: Update deployment/setup.sh**

Replace `simse-app` references with `simse-app`.

**Step 7: Update .github/dependabot.yml**

Change `directory: "/simse-app"` to `directory: "/simse-app"`.

**Step 8: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 9: Commit**

```bash
git add -A
git commit -m "refactor: rename simse-app to simse-app"
```

---

### Task 3: Update app.css to Use Design Tokens

**Files:**
- Modify: `simse-app/app/styles/app.css`

**Step 1: Update app.css**

The existing app.css defines fonts and animations in a `@theme` block. Update it to reference the design system tokens conceptually (Tailwind v4 doesn't import external CSS vars into @theme — the @theme block IS the token definition for Tailwind). Keep the existing @theme block but ensure values match tokens.css exactly. Add any missing token values.

The key changes:
- Verify `--font-sans` and `--font-mono` match tokens.css
- Verify body background `#0a0a0b` matches `--color-dark`
- Verify body text color `#e4e4e7` matches `--color-zinc-200`
- Add a comment at the top referencing the design system as source of truth
- No functional changes — this is an alignment verification step

**Step 2: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 3: Commit**

```bash
git add simse-app/app/styles/app.css
git commit -m "style(app): align app.css with design system tokens"
```

---

### Task 4: Create IconRail Component

**Files:**
- Create: `simse-app/app/components/layout/IconRail.tsx`

**Step 1: Create the component**

```tsx
import clsx from 'clsx';
import { NavLink } from 'react-router';
import SimseLogo from '../ui/SimseLogo';

interface Remote {
	id: string;
	name: string;
	status: 'connected' | 'offline';
}

interface IconRailProps {
	remotes: Remote[];
	activeId: string | null; // null = home context
	onSelect: (id: string | null) => void;
}

export default function IconRail({ remotes, activeId, onSelect }: IconRailProps) {
	return (
		<div className="flex h-screen w-14 flex-col items-center border-r border-zinc-800 bg-zinc-950 py-3">
			{/* Home icon */}
			<button
				type="button"
				onClick={() => onSelect(null)}
				className={clsx(
					'relative flex h-10 w-10 items-center justify-center rounded-xl transition-colors',
					activeId === null
						? 'bg-emerald-400/10 text-emerald-400'
						: 'text-zinc-500 hover:bg-zinc-800/50 hover:text-zinc-300',
				)}
			>
				{activeId === null && (
					<span className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-r bg-emerald-400" />
				)}
				<SimseLogo size={20} />
			</button>

			{/* Separator */}
			{remotes.length > 0 && (
				<div className="my-2 h-px w-8 bg-zinc-800" />
			)}

			{/* Remote icons */}
			<div className="flex flex-1 flex-col items-center gap-2 overflow-y-auto">
				{remotes.map((remote) => (
					<button
						key={remote.id}
						type="button"
						onClick={() => onSelect(remote.id)}
						title={remote.name}
						className={clsx(
							'relative flex h-10 w-10 items-center justify-center rounded-xl font-mono text-xs font-bold transition-colors',
							activeId === remote.id
								? 'bg-emerald-400/10 text-emerald-400'
								: 'text-zinc-500 hover:bg-zinc-800/50 hover:text-zinc-300',
						)}
					>
						{activeId === remote.id && (
							<span className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-r bg-emerald-400" />
						)}
						<span>{remote.name.slice(0, 2).toUpperCase()}</span>
						{remote.status === 'connected' && (
							<span className="absolute bottom-1 right-1 h-2 w-2 rounded-full bg-emerald-400" />
						)}
					</button>
				))}
			</div>

			{/* Add remote */}
			<NavLink
				to="/dashboard/settings/remotes"
				className="flex h-10 w-10 items-center justify-center rounded-xl text-zinc-600 transition-colors hover:bg-zinc-800/50 hover:text-zinc-400"
			>
				<svg className="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
					<path strokeLinecap="round" strokeLinejoin="round" d="M12 4v16m8-8H4" />
				</svg>
			</NavLink>
		</div>
	);
}
```

**Step 2: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 3: Commit**

```bash
git add simse-app/app/components/layout/IconRail.tsx
git commit -m "feat(app): add IconRail component for remote session navigation"
```

---

### Task 5: Create NavPanel Component

**Files:**
- Create: `simse-app/app/components/layout/NavPanel.tsx`

**Step 1: Create the component**

The nav panel shows different items based on context (home vs remote). Uses NavLink for active state styling, matching existing Sidebar patterns.

```tsx
import clsx from 'clsx';
import { NavLink } from 'react-router';

interface NavPanelProps {
	context: 'home' | 'remote';
	contextName: string;
	remoteId?: string;
	onClose?: () => void;
}

const homeNav = [
	{
		label: 'Overview',
		to: '/dashboard',
		icon: 'M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-4 0a1 1 0 01-1-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 01-1 1',
	},
	{
		label: 'Usage',
		to: '/dashboard/usage',
		icon: 'M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z',
	},
	{
		label: 'Library',
		to: '/dashboard/library',
		icon: 'M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253',
	},
];

const remoteNav = (remoteId: string) => [
	{
		label: 'Chat',
		to: `/dashboard/chat/${remoteId}`,
		icon: 'M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z',
	},
	{
		label: 'Files',
		to: `/dashboard/remote/${remoteId}/files`,
		icon: 'M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z',
	},
	{
		label: 'Shell',
		to: `/dashboard/remote/${remoteId}/shell`,
		icon: 'M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z',
	},
	{
		label: 'Network',
		to: `/dashboard/remote/${remoteId}/network`,
		icon: 'M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9',
	},
];

export default function NavPanel({ context, contextName, remoteId, onClose }: NavPanelProps) {
	const items = context === 'home' ? homeNav : remoteNav(remoteId!);
	const settingsTo =
		context === 'home' ? '/dashboard/settings' : `/dashboard/remote/${remoteId}/settings`;

	return (
		<aside className="flex h-screen w-55 flex-col border-r border-zinc-800/50 bg-zinc-950">
			{/* Context header */}
			<div className="px-4 pt-5 pb-3">
				<p className="font-mono text-[10px] font-bold uppercase tracking-[0.25em] text-zinc-500">
					{contextName}
				</p>
			</div>

			{/* Nav items */}
			<nav className="flex-1 space-y-0.5 px-2">
				{items.map((item) => (
					<NavLink
						key={item.to}
						to={item.to}
						end={item.to === '/dashboard'}
						onClick={() => onClose?.()}
						className={({ isActive }) =>
							clsx(
								'flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors',
								isActive
									? 'bg-zinc-800/80 text-white'
									: 'text-zinc-500 hover:bg-zinc-800/40 hover:text-zinc-300',
							)
						}
					>
						<svg
							className="h-4 w-4"
							fill="none"
							viewBox="0 0 24 24"
							stroke="currentColor"
							strokeWidth={2}
						>
							<path strokeLinecap="round" strokeLinejoin="round" d={item.icon} />
						</svg>
						<span>{item.label}</span>
					</NavLink>
				))}
			</nav>

			{/* Settings */}
			<div className="border-t border-zinc-800 p-2">
				<NavLink
					to={settingsTo}
					onClick={() => onClose?.()}
					className={({ isActive }) =>
						clsx(
							'flex items-center gap-3 rounded-lg px-3 py-2 text-sm transition-colors',
							isActive
								? 'bg-zinc-800/80 text-white'
								: 'text-zinc-500 hover:bg-zinc-800/40 hover:text-zinc-300',
						)
					}
				>
					<svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
						<path
							strokeLinecap="round"
							strokeLinejoin="round"
							d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
						/>
						<path strokeLinecap="round" strokeLinejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
					</svg>
					<span>Settings</span>
				</NavLink>
			</div>
		</aside>
	);
}
```

**Step 2: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 3: Commit**

```bash
git add simse-app/app/components/layout/NavPanel.tsx
git commit -m "feat(app): add NavPanel component with context-aware navigation"
```

---

### Task 6: Create AcpSwitcher Component

**Files:**
- Create: `simse-app/app/components/ui/AcpSwitcher.tsx`

**Step 1: Create the component**

Dropdown to select ACP backend. Uses same click-outside/ESC pattern as AccountDropdown and NotificationsBell.

```tsx
import clsx from 'clsx';
import { useEffect, useRef, useState } from 'react';

interface AcpBackend {
	id: string;
	name: string;
	provider: string;
}

interface AcpSwitcherProps {
	backends: AcpBackend[];
	activeId: string;
	onSelect: (id: string) => void;
}

export default function AcpSwitcher({ backends, activeId, onSelect }: AcpSwitcherProps) {
	const [open, setOpen] = useState(false);
	const ref = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (!open) return;
		const handleClick = (e: MouseEvent) => {
			if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
		};
		const handleKey = (e: KeyboardEvent) => {
			if (e.key === 'Escape') setOpen(false);
		};
		document.addEventListener('mousedown', handleClick);
		document.addEventListener('keydown', handleKey);
		return () => {
			document.removeEventListener('mousedown', handleClick);
			document.removeEventListener('keydown', handleKey);
		};
	}, [open]);

	const active = backends.find((b) => b.id === activeId);

	return (
		<div ref={ref} className="relative">
			<button
				type="button"
				onClick={() => setOpen(!open)}
				className="flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm text-zinc-400 transition-colors hover:bg-zinc-800/50 hover:text-zinc-200"
			>
				<span className="font-medium">{active?.name ?? 'Select model'}</span>
				<svg className="h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
					<path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
				</svg>
			</button>

			{open && (
				<div className="absolute left-0 top-full z-50 mt-1 w-64 animate-fade-in rounded-xl border border-zinc-800 bg-zinc-900 py-1 shadow-2xl">
					{backends.map((backend) => (
						<button
							key={backend.id}
							type="button"
							onClick={() => {
								onSelect(backend.id);
								setOpen(false);
							}}
							className={clsx(
								'flex w-full items-center justify-between px-4 py-2.5 text-left text-sm transition-colors hover:bg-zinc-800/50',
								backend.id === activeId ? 'text-white' : 'text-zinc-400',
							)}
						>
							<div>
								<p className="font-medium">{backend.name}</p>
								<p className="text-[12px] text-zinc-600">{backend.provider}</p>
							</div>
							{backend.id === activeId && (
								<svg className="h-4 w-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
									<path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
								</svg>
							)}
						</button>
					))}
				</div>
			)}
		</div>
	);
}
```

**Step 2: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 3: Commit**

```bash
git add simse-app/app/components/ui/AcpSwitcher.tsx
git commit -m "feat(app): add AcpSwitcher dropdown for AI backend selection"
```

---

### Task 7: Create Chat Components (MessageBubble + ToolCallCard)

**Files:**
- Create: `simse-app/app/components/chat/MessageBubble.tsx`
- Create: `simse-app/app/components/chat/ToolCallCard.tsx`

**Step 1: Create MessageBubble**

Renders a single chat message — user, assistant, or system.

```tsx
import clsx from 'clsx';

interface MessageBubbleProps {
	role: 'user' | 'assistant' | 'system';
	content: string;
}

export default function MessageBubble({ role, content }: MessageBubbleProps) {
	return (
		<div
			className={clsx('px-4 py-3', role === 'user' && 'bg-zinc-800/30')}
		>
			<div className="mx-auto max-w-3xl">
				<p
					className={clsx(
						'mb-1 font-mono text-[10px] font-bold uppercase tracking-[0.15em]',
						role === 'user' ? 'text-zinc-500' : 'text-emerald-400/70',
					)}
				>
					{role === 'user' ? 'You' : role === 'assistant' ? 'simse' : 'System'}
				</p>
				<div className="prose prose-invert prose-sm max-w-none text-zinc-300">
					{content}
				</div>
			</div>
		</div>
	);
}
```

**Step 2: Create ToolCallCard**

Collapsible card showing a tool invocation and its result.

```tsx
import { useState } from 'react';
import clsx from 'clsx';

interface ToolCallCardProps {
	name: string;
	input: string;
	output?: string;
	status: 'running' | 'completed' | 'error';
}

export default function ToolCallCard({ name, input, output, status }: ToolCallCardProps) {
	const [expanded, setExpanded] = useState(false);

	return (
		<div className="mx-auto max-w-3xl px-4 py-2">
			<button
				type="button"
				onClick={() => setExpanded(!expanded)}
				className="flex w-full items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-900 px-3 py-2 text-left text-sm transition-colors hover:border-zinc-700"
			>
				{status === 'running' ? (
					<span className="h-3 w-3 animate-spin rounded-full border-2 border-emerald-400 border-t-transparent" />
				) : status === 'error' ? (
					<span className="h-3 w-3 rounded-full bg-red-400" />
				) : (
					<span className="h-3 w-3 rounded-full bg-emerald-400" />
				)}
				<span className="font-mono text-[12px] text-zinc-400">{name}</span>
				<svg
					className={clsx('ml-auto h-3.5 w-3.5 text-zinc-600 transition-transform', expanded && 'rotate-180')}
					fill="none"
					viewBox="0 0 24 24"
					stroke="currentColor"
					strokeWidth={2}
				>
					<path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
				</svg>
			</button>
			{expanded && (
				<div className="mt-1 rounded-b-lg border border-t-0 border-zinc-800 bg-zinc-900/50 p-3">
					<p className="font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">Input</p>
					<pre className="mt-1 overflow-x-auto text-[12px] text-zinc-400">{input}</pre>
					{output && (
						<>
							<p className="mt-3 font-mono text-[10px] font-bold uppercase tracking-[0.15em] text-zinc-600">Output</p>
							<pre className="mt-1 overflow-x-auto text-[12px] text-zinc-400">{output}</pre>
						</>
					)}
				</div>
			)}
		</div>
	);
}
```

**Step 3: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 4: Commit**

```bash
git add simse-app/app/components/chat/
git commit -m "feat(app): add MessageBubble and ToolCallCard chat components"
```

---

### Task 8: Create ChatInterface Component

**Files:**
- Create: `simse-app/app/components/chat/ChatInterface.tsx`

**Step 1: Create the component**

Full chat UI with message list, auto-scroll, and input area. This is a presentational component — actual message sending will be wired up when the ACP integration is built.

```tsx
import { useEffect, useRef, useState } from 'react';
import SimseLogo from '../ui/SimseLogo';
import MessageBubble from './MessageBubble';
import ToolCallCard from './ToolCallCard';

interface Message {
	id: string;
	role: 'user' | 'assistant' | 'system';
	content: string;
}

interface ToolCall {
	id: string;
	name: string;
	input: string;
	output?: string;
	status: 'running' | 'completed' | 'error';
}

interface ChatInterfaceProps {
	messages: Message[];
	toolCalls: ToolCall[];
	onSend: (message: string) => void;
	isStreaming?: boolean;
}

export default function ChatInterface({ messages, toolCalls, onSend, isStreaming }: ChatInterfaceProps) {
	const [input, setInput] = useState('');
	const bottomRef = useRef<HTMLDivElement>(null);
	const textareaRef = useRef<HTMLTextAreaElement>(null);

	useEffect(() => {
		bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
	}, [messages.length, toolCalls.length]);

	const handleSubmit = () => {
		const trimmed = input.trim();
		if (!trimmed || isStreaming) return;
		onSend(trimmed);
		setInput('');
		if (textareaRef.current) textareaRef.current.style.height = 'auto';
	};

	const handleKeyDown = (e: React.KeyboardEvent) => {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSubmit();
		}
	};

	const handleInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
		setInput(e.target.value);
		const el = e.target;
		el.style.height = 'auto';
		el.style.height = `${Math.min(el.scrollHeight, 200)}px`;
	};

	const isEmpty = messages.length === 0;

	return (
		<div className="flex h-full flex-col">
			{isEmpty ? (
				<div className="flex flex-1 flex-col items-center justify-center gap-4">
					<SimseLogo size={48} className="text-zinc-700" />
					<p className="text-lg text-zinc-500">What would you like to do?</p>
				</div>
			) : (
				<div className="flex-1 overflow-y-auto">
					<div className="py-6">
						{messages.map((msg) => (
							<MessageBubble key={msg.id} role={msg.role} content={msg.content} />
						))}
						{toolCalls.map((tc) => (
							<ToolCallCard key={tc.id} {...tc} />
						))}
						{isStreaming && (
							<div className="px-4 py-3">
								<div className="mx-auto max-w-3xl">
									<span className="inline-block h-4 w-1 animate-blink bg-emerald-400" />
								</div>
							</div>
						)}
					</div>
					<div ref={bottomRef} />
				</div>
			)}

			{/* Input area */}
			<div className="border-t border-zinc-800/50 p-4">
				<div className="mx-auto flex max-w-3xl items-end gap-2">
					<textarea
						ref={textareaRef}
						value={input}
						onChange={handleInput}
						onKeyDown={handleKeyDown}
						placeholder="Message simse..."
						rows={1}
						className="flex-1 resize-none rounded-xl border border-zinc-800 bg-zinc-900 px-4 py-3 text-sm text-zinc-200 placeholder-zinc-600 outline-none transition-colors focus:border-zinc-700 focus:ring-1 focus:ring-emerald-400/50"
					/>
					<button
						type="button"
						onClick={handleSubmit}
						disabled={!input.trim() || isStreaming}
						className="flex h-11 w-11 items-center justify-center rounded-xl bg-emerald-400 text-zinc-950 transition-colors hover:bg-emerald-300 disabled:opacity-40 disabled:hover:bg-emerald-400"
					>
						<svg className="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
							<path strokeLinecap="round" strokeLinejoin="round" d="M5 12h14M12 5l7 7-7 7" />
						</svg>
					</button>
				</div>
			</div>
		</div>
	);
}
```

**Step 2: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 3: Commit**

```bash
git add simse-app/app/components/chat/ChatInterface.tsx
git commit -m "feat(app): add ChatInterface component with message list and input"
```

---

### Task 9: Rewrite DashboardLayout with IconRail + NavPanel

**Files:**
- Modify: `simse-app/app/components/layout/DashboardLayout.tsx`
- Delete: `simse-app/app/components/layout/Sidebar.tsx`

**Step 1: Read the existing DashboardLayout.tsx**

Read `simse-app/app/components/layout/DashboardLayout.tsx` to understand the current props interface and mobile handling.

**Step 2: Rewrite DashboardLayout**

Replace the single-sidebar layout with IconRail + NavPanel. Keep the header bar with notifications and account dropdown. Add AcpSwitcher to the header.

The new layout structure:
- `h-screen flex` container
- IconRail (always visible on desktop, hidden on mobile)
- NavPanel (always visible on desktop, drawer on mobile)
- Main content column with header + scrollable Outlet

Props change: add `remotes` array, `activeRemoteId`, `onRemoteSelect`, and `acpBackends` / `activeAcpId` / `onAcpSelect`.

Key details:
- Import IconRail, NavPanel, AcpSwitcher instead of Sidebar
- Mobile: hamburger opens NavPanel as drawer (no icon rail on mobile)
- The `context` and `contextName` for NavPanel are derived from `activeRemoteId`
- Keep existing NotificationsBell and AccountDropdown in header
- AcpSwitcher goes on the left side of the header bar

**Step 3: Delete Sidebar.tsx**

Remove `simse-app/app/components/layout/Sidebar.tsx` — it's fully replaced.

**Step 4: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 5: Commit**

```bash
git add simse-app/app/components/layout/
git commit -m "feat(app): replace Sidebar with IconRail + NavPanel layout"
```

---

### Task 10: Update dashboard.tsx Route (Loader + Layout Props)

**Files:**
- Modify: `simse-app/app/routes/dashboard.tsx`

**Step 1: Read the existing file**

Read `simse-app/app/routes/dashboard.tsx` to see the current loader and how it passes props to DashboardLayout.

**Step 2: Update the loader and component**

The loader currently fetches user info and notifications. Add:
- Fetch connected remotes from `/remotes` API (with try/catch, default to empty array)
- Fetch available ACP backends from `/acp/backends` API (with try/catch, default to empty array)
- Return remotes and backends alongside existing data

The component needs to:
- Track `activeRemoteId` with useState (null = home context)
- Track `activeAcpId` with useState (first backend or empty)
- Pass new props to DashboardLayout
- Derive context/contextName from activeRemoteId

**Step 3: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 4: Commit**

```bash
git add simse-app/app/routes/dashboard.tsx
git commit -m "feat(app): update dashboard route with remotes and ACP state"
```

---

### Task 11: Add Chat Route

**Files:**
- Create: `simse-app/app/routes/dashboard.chat.tsx`
- Create: `simse-app/app/routes/dashboard.chat.$remoteId.tsx`
- Modify: `simse-app/app/routes.ts`

**Step 1: Create dashboard.chat.tsx**

Home-context chat page. Renders ChatInterface with empty state. Messages are placeholder/mock for now — real ACP integration comes later.

```tsx
import ChatInterface from '~/components/chat/ChatInterface';

export default function Chat() {
	// Placeholder — real message state will come from ACP integration
	return (
		<ChatInterface
			messages={[]}
			toolCalls={[]}
			onSend={(msg) => console.log('send:', msg)}
		/>
	);
}
```

**Step 2: Create dashboard.chat.$remoteId.tsx**

Remote-context chat page. Same structure, but receives remoteId from params.

```tsx
import ChatInterface from '~/components/chat/ChatInterface';
import type { Route } from './+types/dashboard.chat.$remoteId';

export default function RemoteChat({ params }: Route.ComponentProps) {
	return (
		<ChatInterface
			messages={[]}
			toolCalls={[]}
			onSend={(msg) => console.log('send to', params.remoteId, ':', msg)}
		/>
	);
}
```

**Step 3: Update routes.ts**

Add chat routes inside the dashboard layout:

```typescript
route('dashboard/chat', './routes/dashboard.chat.tsx'),
route('dashboard/chat/:remoteId', './routes/dashboard.chat.$remoteId.tsx'),
```

**Step 4: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 5: Commit**

```bash
git add simse-app/app/routes/dashboard.chat.tsx simse-app/app/routes/dashboard.chat.\$remoteId.tsx simse-app/app/routes.ts
git commit -m "feat(app): add chat routes for home and remote contexts"
```

---

### Task 12: Add Devices Settings Page

**Files:**
- Create: `simse-app/app/routes/dashboard.settings.devices.tsx`
- Modify: `simse-app/app/routes/dashboard.settings.tsx` — add Devices tab
- Modify: `simse-app/app/routes.ts` — add devices route

**Step 1: Create the devices settings page**

Loader fetches from `/auth/devices` API. Shows current session + other devices. Revoke action posts to the route action handler.

Structure:
- PageHeader with "Devices" title and "Browsers and apps signed into your account" description
- "Current session" card (highlighted with accent border)
- Other devices list in a Card with dividers
- Each device row: icon (browser/OS), name, location, last active, Revoke button (Form with POST)
- "Sign out all other devices" button at bottom (Form with POST, intent="revoke-all")

Action handler handles `intent: "revoke"` (single device by deviceId) and `intent: "revoke-all"`.

**Step 2: Update dashboard.settings.tsx**

Read the file first. Add "Devices" to the tabs array:

```typescript
{ label: 'Devices', to: '/dashboard/settings/devices' },
```

**Step 3: Update routes.ts**

Add inside the settings layout:

```typescript
route('dashboard/settings/devices', './routes/dashboard.settings.devices.tsx'),
```

**Step 4: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 5: Commit**

```bash
git add simse-app/app/routes/dashboard.settings.devices.tsx simse-app/app/routes/dashboard.settings.tsx simse-app/app/routes.ts
git commit -m "feat(app): add device management settings page"
```

---

### Task 13: Add Remotes Settings Page

**Files:**
- Create: `simse-app/app/routes/dashboard.settings.remotes.tsx`
- Modify: `simse-app/app/routes/dashboard.settings.tsx` — add Remotes tab
- Modify: `simse-app/app/routes.ts` — add remotes route

**Step 1: Create the remotes settings page**

Loader fetches from `/remotes` API. Shows connected and offline remotes.

Structure:
- PageHeader with "Remotes" title, "+ Connect" button linking to a modal or instructions
- Connected remotes Card section with green status dots
- Offline remotes Card section with gray dots
- Each remote row: status dot, machine name, OS, simse-core version, connected/last-seen time, Disconnect/Remove button
- "+ Connect" button opens a Modal with setup instructions (install simse-remote, run auth login, connect)

Action handler handles `intent: "disconnect"` (by remoteId) and `intent: "remove"` (by remoteId).

**Step 2: Update dashboard.settings.tsx**

Add "Remotes" to the tabs array:

```typescript
{ label: 'Remotes', to: '/dashboard/settings/remotes' },
```

**Step 3: Update routes.ts**

Add inside the settings layout:

```typescript
route('dashboard/settings/remotes', './routes/dashboard.settings.remotes.tsx'),
```

**Step 4: Verify**

```bash
cd /workspaces/simse/simse-app && bun run typecheck && bun run lint
```

**Step 5: Commit**

```bash
git add simse-app/app/routes/dashboard.settings.remotes.tsx simse-app/app/routes/dashboard.settings.tsx simse-app/app/routes.ts
git commit -m "feat(app): add remotes management settings page"
```

---

### Task 14: Final Verification and Cleanup

**Files:**
- All modified files

**Step 1: Full typecheck**

```bash
cd /workspaces/simse/simse-app && bun run typecheck
```

Fix any type errors.

**Step 2: Full lint**

```bash
cd /workspaces/simse/simse-app && bun run lint
```

Fix any lint errors.

**Step 3: Verify routes.ts is complete**

Read `simse-app/app/routes.ts` and confirm the final route tree matches the design:

```
/ → redirect
/auth/* → auth routes
/dashboard → overview
/dashboard/usage → usage
/dashboard/library → library (placeholder)
/dashboard/notifications → notifications
/dashboard/account → account
/dashboard/chat → chat (home)
/dashboard/chat/:remoteId → chat (remote)
/dashboard/settings → settings layout
/dashboard/settings/ → general
/dashboard/settings/billing → billing
/dashboard/settings/billing/credit → credit
/dashboard/settings/team → team
/dashboard/settings/team/invite → invite
/dashboard/settings/team/plans → plans
/dashboard/settings/devices → devices (NEW)
/dashboard/settings/remotes → remotes (NEW)
```

**Step 4: Verify no dead imports**

Check that Sidebar.tsx is not imported anywhere. Check that all new components are properly imported where used.

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix(app): resolve typecheck and lint issues from dashboard redesign"
```

**Step 6: Push**

```bash
git push
```
