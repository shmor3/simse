# Dashboard Redesign Design

## Overview

Comprehensive visual and feature improvement of the simse-app dashboard. Covers the overview page, usage page, visual system, and new library page.

## 1. Overview Page Redesign

### Welcome Hero Section
- Greeting with user's name and time-of-day message ("Good morning, Alex")
- Subtle animated gradient border (emerald-400 to cyan-400)
- Getting started checklist for new users: connect first remote, start a session, add to library, invite a teammate
- Progress ring showing checklist completion percentage
- Dismissible once all steps complete

### Stat Cards with Sparklines
- Each stat card gets a 7-point SVG sparkline showing trend over last 7 days
- Sparkline uses emerald accent color
- Hover: subtle lift + emerald border glow + shadow
- Staggered entrance animation (fade-in-up with delay per card)

### Quick Actions with Icons
- Each card gets a distinctive icon in an emerald-tinted circle
- Remove generic "Quick action" label, replace with contextual category
- Right-arrow indicator appears on hover
- Three actions: New session, Browse library, Invite teammate

### Activity Feed
- Replace sessions table with a timeline of recent events
- Event types: session started/ended, library item added, remote connected/disconnected, team invite
- Type-colored icons with connecting vertical line
- Relative timestamps ("2 hours ago")
- Empty state with illustration and CTA

### Connected Remotes Widget
- Card showing connected remote machines
- Status dot (green=connected, gray=offline), name, uptime
- Quick-connect button per remote
- "Add remote" CTA if none connected

## 2. Usage Page Improvements

### Ring Gauge
- Circular SVG ring gauge replacing flat progress bar for headline usage metric
- Animated fill on mount using CSS stroke-dashoffset transition
- Color transitions: emerald (< 70%), amber (< 90%), red (>= 90%)
- Percentage displayed in center, credit balance below

### Enhanced Bar Chart
- Rounded-top bars with gradient fill (emerald-500 to emerald-400)
- Subtle horizontal grid lines in background
- Today's bar highlighted (brighter fill, small dot indicator)
- Smooth height animation on mount
- Period selector: 7d / 30d / 90d toggle buttons

### Usage Breakdown
- Category rows: Sessions, Library, Tools
- Each row: label, horizontal bar (proportional width), token count, percentage
- Zero-state shows categories with empty bars and "0" values
- Sorted by usage descending

## 3. Visual System Upgrades

### Entrance Animations
- Staggered fade-in-up using CSS `animation-delay` (each card 50ms offset)
- New `animate-stagger-1` through `animate-stagger-6` utility classes
- Sections animate in sequentially as page loads

### Hover & Interaction States
- Cards: emerald border glow on hover (`border-emerald-400/30`, `shadow-emerald-400/5`)
- Stat cards: slight scale(1.01) + shadow lift
- Buttons and interactive elements: micro-transitions (150ms)
- Quick action cards: arrow slides in from left on hover

### Gradient Accents
- Welcome section: gradient text on greeting (emerald-400 to cyan-400 via `bg-clip-text`)
- Key cards get optional gradient top border (replacing solid emerald)
- Subtle radial gradient on welcome card background

### Empty States
- Each empty state gets a unique inline SVG illustration (geometric/abstract style)
- More descriptive helper text with context
- Primary CTA button to take the relevant action
- Illustrations: chat bubbles (sessions), book/stack (library), globe (remotes), chart bars (usage)

### Loading Skeletons
- Shimmer effect across stat cards, chart bars, timeline items
- Skeleton shapes match actual content layout (sparkline placeholder, ring gauge placeholder)

## 4. New Pages

### Library Page (`/dashboard/library`)
- Search bar with placeholder text
- Grid/list view toggle
- Library item cards: title, topic tags (Badge), similarity score, date added, preview snippet
- Empty state with book illustration, "Add your first item" CTA
- Filter sidebar or top filter bar (by topic, date, type)

## Technical Notes

- All animations CSS-only (no JS animation libraries)
- SVG sparklines rendered inline (no charting library)
- Ring gauge is a CSS-animated SVG circle with `stroke-dasharray`/`stroke-dashoffset`
- Stagger animations use CSS custom properties for delay calculation
- All new components follow existing patterns: Tailwind classes, clsx, dark theme tokens
- Placeholder/mock data in loaders for all new features (no backend changes)
