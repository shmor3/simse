# Component Specifications

UI components from `simse-app/app/components/ui/`. Each component follows the simse design tokens and brand guidelines.

---

## Button

Interactive button with four visual variants and a loading state.

**Props:**

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `variant` | `'primary' \| 'secondary' \| 'ghost' \| 'danger'` | `'primary'` | Visual style |
| `loading` | `boolean` | `false` | Shows a spinner and disables the button |
| `disabled` | `boolean` | `false` | Disables the button |
| `className` | `string` | — | Additional CSS classes |

**Variants:**

| Variant | Background | Text | Border | Hover |
|---------|-----------|------|--------|-------|
| `primary` | `--color-emerald` | `--color-zinc-950` | none | Lighter emerald |
| `secondary` | `--color-zinc-800` | `--color-zinc-100` | `--color-zinc-700` | `--color-zinc-700` bg |
| `ghost` | transparent | `--color-zinc-400` | none | `--color-zinc-800` bg, text brightens to `--color-zinc-100` |
| `danger` | red 10% opacity | red text | red 20% opacity | red 20% opacity bg |

**Behavior:**
- Font: mono, 14px (text-sm), bold, uppercase tracking inherited from font-mono context
- Padding: `px-4 py-2.5`
- Border radius: `--radius-md` (8px, via `rounded-lg`)
- Focus ring: emerald 50% opacity, 2px ring, offset by 2px on zinc-950
- Loading state: prepends a spinning border circle (4x4, border-2, `border-current border-t-transparent`), disables pointer events
- Disabled: 50% opacity, no pointer events

---

## Card

Container surface with optional emerald accent bar.

**Props:**

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `accent` | `boolean` | `false` | Adds a 4px emerald bar at the top |
| `className` | `string` | — | Additional CSS classes |
| `children` | `ReactNode` | — | Card content |

**Appearance:**
- Background: `--color-zinc-900`
- Border: 1px solid `--color-zinc-800`
- Border radius: `--radius-lg` (12px, via `rounded-xl`)
- When `accent` is true: `overflow-hidden` is added and a `h-1 bg-emerald-400` div is rendered before children

---

## Badge

Small inline label with five color variants.

**Props:**

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `variant` | `'default' \| 'emerald' \| 'warning' \| 'danger' \| 'info'` | `'default'` | Color scheme |
| `children` | `ReactNode` | — | Badge content |
| `className` | `string` | — | Additional CSS classes |

**Variants:**

| Variant | Background | Text | Border |
|---------|-----------|------|--------|
| `default` | `--color-zinc-800` | `--color-zinc-400` | `--color-zinc-700` |
| `emerald` | emerald 10% | `--color-emerald` | emerald 20% |
| `warning` | amber 10% | amber-400 | amber 20% |
| `danger` | red 10% | red-400 | red 20% |
| `info` | blue 10% | `--color-info` | blue 20% |

**Appearance:**
- Font: mono, 11px, bold, uppercase, wider tracking
- Padding: `px-2 py-0.5`
- Border radius: `--radius-sm` (6px, via `rounded-md`)
- Border: 1px solid

---

## Avatar

Circular initials avatar in three sizes.

**Props:**

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `name` | `string` | (required) | Full name, used to generate initials |
| `size` | `'sm' \| 'md' \| 'lg'` | `'md'` | Avatar dimensions |
| `className` | `string` | — | Additional CSS classes |

**Sizes:**

| Size | Dimensions | Font size |
|------|-----------|-----------|
| `sm` | 28x28 (h-7 w-7) | 10px |
| `md` | 36x36 (h-9 w-9) | 12px |
| `lg` | 48x48 (h-12 w-12) | 14px |

**Appearance:**
- Shape: circle (`rounded-full`)
- Background: emerald 10% opacity
- Text: `--color-emerald`, mono font, bold
- Initials: first letter of first two words, uppercased
- `title` attribute set to the full name

---

## Input

Text input with optional label, error message, and leading icon.

**Props:**

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `label` | `string` | — | Label text above the input |
| `error` | `string` | — | Error message below the input |
| `icon` | `ReactNode` | — | Icon rendered inside the input on the left |
| `className` | `string` | — | Additional CSS classes on the input element |

Extends all native `<input>` HTML attributes.

**Appearance:**
- Label: mono font, 11px, bold, uppercase, tracking 0.15em, `--color-zinc-500`
- Input: `--color-zinc-900` bg, 1px `--color-zinc-800` border, `--color-zinc-100` text, 14px (text-sm)
- Padding: `px-3 py-2.5` (left padding increases to `pl-10` when icon is present)
- Border radius: `--radius-md` (8px, via `rounded-lg`)
- Placeholder: `--color-zinc-600`
- Hover: border changes to `--color-zinc-700`
- Focus: border `emerald 50%`, ring 1px `emerald 25%`
- Error state: border `red 50%`, error text is 13px `red 80%` below the input
- Icon: positioned absolutely, left-3, vertically centered, `--color-zinc-600`, pointer-events-none

---

## Modal

Dialog overlay with title, optional description, optional body content, and confirm/cancel actions.

**Props:**

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `open` | `boolean` | (required) | Controls visibility |
| `onClose` | `() => void` | (required) | Called on Escape key or backdrop click |
| `title` | `string` | (required) | Modal heading |
| `description` | `string` | — | Secondary text below the title |
| `confirmLabel` | `string` | `'Confirm'` | Text for the confirm button |
| `confirmVariant` | `'primary' \| 'danger'` | `'primary'` | Confirm button variant |
| `onConfirm` | `() => void` | — | Confirm handler; if omitted, no confirm button is rendered |
| `loading` | `boolean` | `false` | Disables both buttons and shows spinner on confirm |
| `children` | `ReactNode` | — | Custom body content between description and actions |

**Appearance:**
- Overlay: fixed full-screen, `bg-black/60`, `backdrop-blur-sm`, fade-in animation
- Panel: max-width `md` (28rem), `--color-zinc-900` bg, 1px `--color-zinc-800` border, `rounded-xl`, `p-6`, `shadow-2xl`, fade-in-up animation
- Title: 18px (text-lg), bold, white
- Description: 14px (text-sm), `--color-zinc-400`, margin-top 8px
- Actions: flex row, right-aligned, gap 12px. Cancel is `ghost` variant, confirm uses `confirmVariant`.

**Behavior:**
- Closes on Escape keydown (document-level listener)
- Closes on backdrop click (overlay click, not panel click)
- Returns `null` when `open` is false (no DOM rendered)

---

## StatCard

Metric display card with label, value, optional change indicator, and loading skeleton.

**Props:**

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `label` | `string` | (required) | Metric label (rendered in mono uppercase) |
| `value` | `string \| number` | (required) | Metric value |
| `change` | `string` | — | Change text (e.g. "12%") |
| `positive` | `boolean` | — | If true, change is emerald with "+" prefix; if false, change is red |
| `loading` | `boolean` | `false` | Shows shimmer skeleton instead of content |
| `className` | `string` | — | Additional CSS classes |

**Appearance:**
- Built on top of Card component with `p-5` padding
- Label: mono, 10px, bold, uppercase, tracking 0.25em, `--color-zinc-500`
- Value: 24px (text-2xl), bold, tight tracking, white
- Change: mono, 12px, emerald when positive, red when negative
- Loading skeleton: two shimmer bars (gradient from `--color-zinc-800` through `--color-zinc-700` and back), replacing label (h-3 w-16) and value (h-7 w-24)

---

## ProgressBar

Horizontal bar with dynamic color thresholds based on percentage.

**Props:**

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `value` | `number` | (required) | Current value |
| `max` | `number` | (required) | Maximum value |
| `label` | `string` | — | Label text above the bar |
| `showValue` | `boolean` | `true` | Shows "value / max" text on the right |
| `className` | `string` | — | Additional CSS classes |

**Color thresholds:**

| Percentage | Bar color |
|-----------|-----------|
| < 70% | `--color-emerald` (emerald-400) |
| 70% -- 89% | `--color-warning` (amber-400) |
| >= 90% | `--color-error` (red-400) |

**Appearance:**
- Track: h-2, `rounded-full`, `--color-zinc-800` bg
- Fill: h-2, `rounded-full`, width set via inline style to computed percentage, `transition-all duration-500`
- Label: mono, 11px, bold, uppercase, tracking 0.15em, `--color-zinc-500`
- Value text: mono, 12px, `--color-zinc-400`, formatted with `toLocaleString()`
- Percentage is clamped to 0-100

---

## CodeInput

Multi-digit code entry with individual digit boxes, used for verification codes.

**Props:**

| Prop | Type | Default | Description |
|------|------|---------|-------------|
| `length` | `number` | `6` | Number of digit boxes |
| `name` | `string` | `'code'` | Hidden input name for form submission |
| `error` | `string` | — | Error message below the boxes |
| `onComplete` | `(code: string) => void` | — | Called when all digits are filled |

**Appearance:**
- Each box: 56x44 (h-14 w-11), `--color-zinc-900` bg, 1px `--color-zinc-800` border, `rounded-lg`
- Text: mono, 20px (text-xl), bold, white, centered
- Focus: border `emerald 50%`, ring 1px `emerald 25%`
- Error state: border `red 50%`, error text is 13px `red 80%` centered below
- Boxes are spaced with gap-2 (8px), centered with flexbox

**Behavior:**
- Accepts only numeric input (non-digits stripped)
- Auto-advances focus to next box on digit entry
- Backspace on empty box moves focus to previous box
- Arrow keys navigate between boxes
- Paste on first box distributes digits across all boxes
- Hidden input holds the concatenated code for form submission
- `onComplete` fires when all boxes are filled (on change or paste)
