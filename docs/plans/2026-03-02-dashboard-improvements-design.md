# Dashboard Improvements Design

## Overview

Improve simse-cloud dashboard with a header bar + account dropdown, a full account settings page, polish across existing pages, and responsive mobile support.

## 1. Dashboard Header Bar + Account Dropdown

Add a persistent header bar spanning the top of the main content area (right of sidebar).

**Layout:**
```
┌──────────┬──────────────────────────────────┐
│          │  [breadcrumb]       [Avatar ▾]   │
│ Sidebar  │──────────────────────────────────│
│          │                                   │
│          │  Page content                     │
│          │                                   │
└──────────┴──────────────────────────────────┘
```

**Header contents:**
- Left: empty initially (reserved for breadcrumbs)
- Right: Avatar + user name, clickable to open dropdown

**Dropdown menu items:**
1. User name + email (display only, top section with divider)
2. Account — links to `/dashboard/account`
3. Help — external link to `https://simse.dev/docs`
4. Divider
5. Sign out — form POST to `/auth/logout`

**Component:** `AccountDropdown` in `components/ui/`. Click-outside-to-close pattern using React state + effect. No external deps.

**Sidebar change:** Replace bottom "Sign out" button with avatar + name (visual context only). Sign out moves to the dropdown.

## 2. Account Settings Page

New route: `/dashboard/account`

**Sections (single scrollable page):**

### Profile
- Display: avatar (initials), name, email
- Edit name form (inline save)
- Email shown as read-only with "Change email" action (sends confirmation via email API)

### Security
- Change password form: current password, new password, confirm new password
- 2FA section: enable/disable toggle with code verification flow

### Preferences
- Notification email toggles (all default on):
  - Billing alerts
  - Weekly digest
  - Product updates
  - Security alerts

### Danger Zone
- Red-bordered card
- "Delete account" button → confirmation modal
- User must type their email to confirm deletion

**Nav:** "Account" added to sidebar nav between Notifications and the bottom section.

## 3. Polish Existing Pages

### Empty States
- Better empty states for sessions table, notifications, usage charts
- Add illustrative SVG icons and clearer CTAs with action buttons

### Dashboard Quick Actions
- Make cards clickable links (new session, browse library, team invite)

### Loading States
- Skeleton shimmer on StatCards and tables using existing `shimmer` CSS animation

### Micro-interactions
- Hover lift on cards (`hover:-translate-y-0.5`)
- Smooth nav active state transitions

### Spacing Consistency
- Standardize gap between PageHeader and first content section

### Usage Chart
- Add hover tooltips on bar chart bars
- Smoother height transitions

## 4. Responsive / Mobile

- Sidebar collapses to off-screen on small viewports (`<768px`)
- Hamburger menu button in header bar to toggle sidebar
- Sidebar overlays content with backdrop when open on mobile
- Cards stack vertically, stats grid goes 2-col then 1-col

## Technical Approach

All changes use existing architecture. No new libraries. Pure React state for dropdown/sidebar toggle. Click-outside hooks built inline. Account page follows React Router loader/action pattern. Mobile sidebar uses CSS transforms + toggle state in DashboardLayout.

## Files to Create
- `components/ui/AccountDropdown.tsx`
- `routes/dashboard.account.tsx`

## Files to Modify
- `components/layout/DashboardLayout.tsx` — add header bar, mobile toggle
- `components/layout/Sidebar.tsx` — add Account nav item, replace bottom sign-out with avatar, mobile overlay
- `components/layout/PageHeader.tsx` — spacing adjustments
- `components/ui/StatCard.tsx` — loading skeleton variant
- `components/ui/Card.tsx` — hover lift option
- `routes/dashboard.tsx` — pass user name/email to layout
- `routes/dashboard._index.tsx` — clickable quick actions, better empty states
- `routes/dashboard.usage.tsx` — chart hover tooltips
- `routes/dashboard.notifications.tsx` — better empty state
- `styles/app.css` — mobile sidebar transitions, skeleton styles
