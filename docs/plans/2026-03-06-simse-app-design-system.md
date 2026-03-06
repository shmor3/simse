# simse-app Design System & Dashboard Redesign

**Date:** 2026-03-06
**Status:** Approved

## Summary

Rename simse-cloud to simse-app, create a formal design system in simse-brand, and redesign the dashboard with a Discord/VS Code-style icon rail layout, Claude-web-style chat interface, ACP backend switcher, and new settings pages for devices and remotes.

## Layout Architecture

Three-column layout: Icon Rail (56px) + Nav Panel (220px) + Main Content.

### Icon Rail (`IconRail.tsx`)

- Fixed 56px wide, full height, `bg-zinc-950`, right border `zinc-800`
- Top: Home icon (simse logomark, always present)
- Middle: Connected remote icons (avatar/initials of machine name, emerald ring when active)
- Bottom: Add remote button (+ icon, links to remotes setup)
- Active icon has left emerald-400 border indicator (Discord-style)

### Nav Panel (`NavPanel.tsx`)

- 220px wide, `bg-zinc-950`, right border `zinc-800/50`
- Content changes based on rail icon selection:
  - **Home context**: Overview, Usage, Library + Settings at bottom
  - **Remote context**: Chat, Files, Shell, Network + Remote settings at bottom
- Header area shows context name ("simse" for home, machine name for remote)

### Main Content Area

- Flexible width, `bg-[#0a0a0b]`
- Header bar: ACP switcher (left), notifications bell + account dropdown (right)
- Content depends on route

### Mobile

- Icon rail hides
- Nav becomes a drawer with hamburger toggle in header

## Chat Interface

Route: `/dashboard/chat` (home) or `/dashboard/chat/:remoteId` (remote context).

### Layout

- ACP switcher in header (dropdown showing available backends with provider icons)
- Message area with auto-scroll
- Bottom-pinned textarea with auto-resize and send button

### Message Types

- **User messages**: subtle `zinc-800/50` background
- **Assistant messages**: no background (clean, like Claude web)
- **Tool calls**: collapsible card with icon, tool name, result. `zinc-900` bg, `zinc-800` border
- Markdown rendering with syntax highlighting

### Empty State

- Centered simse logo
- "What would you like to do?" prompt
- Optional quick-action chips

### ACP Switcher

- Top-left dropdown in header
- Shows available backends (e.g., Claude Sonnet 4.6, Ollama local)
- Each shows name + provider icon, selected has checkmark

## Settings Additions

Settings tab bar: General | Billing | Team | **Devices** | **Remotes**

### Devices Tab (`/dashboard/settings/devices`)

Browsers and apps signed into the account.

Per device: browser/OS (from user-agent), approximate location (from IP/geo), last active timestamp, current session indicator. Revoke action signs out that device. "Sign out all other devices" bulk action.

### Remotes Tab (`/dashboard/settings/remotes`)

simse-remote instances connected to the account.

Per remote: machine name, OS, connection status (green = connected, gray = offline), simse-core version, connected/last-seen timestamp. Actions: disconnect (live), remove (offline). "+ Connect" button opens modal with setup instructions.

## Design System (simse-brand)

### Structure

```
simse-brand/design-system/
  README.md        -- Overview and usage guide
  tokens.css       -- CSS custom properties (source of truth)
  tokens.ts        -- TypeScript export of all tokens
  components.md    -- Component specifications
```

### Token Categories

- **Colors**: emerald (#34d399), dark (#0a0a0b), white, error (#ff6568), warning (#fbbf24), info (#60a5fa), zinc scale (100-950)
- **Typography**: DM Sans Variable (body/UI), Space Mono (labels/code). Scale: h1 (64px), h2 (36px), h3 (24px), body (16px), small (14px), label (10-11px mono uppercase)
- **Spacing/Radius**: sm (6px), md (8px), lg (12px), full (9999px)
- **Animations**: fade-in (0.6s), fade-in-up (0.5s), blink (0.8s), shimmer (2s)

simse-app's `app.css` imports tokens and maps to Tailwind `@theme`. Tokens file is the single source of truth.

## Rename

- Directory: `simse-cloud/` -> `simse-app/`
- Update: `moon.yml`, `package.json` name, `wrangler.toml`, CLAUDE.md, CI workflows, deployment scripts

## Route Structure

```
/dashboard                       -- Overview
/dashboard/usage                 -- Usage analytics
/dashboard/library               -- Library browser (placeholder)
/dashboard/notifications         -- Full notifications page
/dashboard/account               -- Account profile
/dashboard/chat                  -- Chat (home context)
/dashboard/chat/:remoteId        -- Chat (remote context)
/dashboard/settings              -- Settings layout (tabs)
/dashboard/settings/             -- General
/dashboard/settings/billing      -- Billing
/dashboard/settings/billing/credit -- Credit top-up
/dashboard/settings/team         -- Team management
/dashboard/settings/team/invite  -- Invite members
/dashboard/settings/team/plans   -- Plan selection
/dashboard/settings/devices      -- Device management (NEW)
/dashboard/settings/remotes      -- Remote management (NEW)
```

## Components

| Component | Action |
|-----------|--------|
| `IconRail.tsx` | New -- icon rail with home + remotes |
| `NavPanel.tsx` | New -- context-aware nav panel |
| `ChatInterface.tsx` | New -- main chat UI |
| `AcpSwitcher.tsx` | New -- ACP backend dropdown |
| `MessageBubble.tsx` | New -- chat message rendering |
| `ToolCallCard.tsx` | New -- collapsible tool call display |
| `DashboardLayout.tsx` | Modified -- integrate rail + nav panel |
| `Sidebar.tsx` | Removed -- replaced by IconRail + NavPanel |
| `dashboard.settings.tsx` | Modified -- add Devices + Remotes tabs |
| `dashboard.settings.devices.tsx` | New -- devices settings page |
| `dashboard.settings.remotes.tsx` | New -- remotes settings page |
| `dashboard.chat.tsx` | New -- chat route |

## Terminology

- **Devices**: logged-in clients (browsers, apps) -- like Google's "Your devices"
- **Remotes**: simse-remote instances connected via relay -- aligns with crate name
