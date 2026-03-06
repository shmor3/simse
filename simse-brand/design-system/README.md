# simse Design System

Design tokens and component specifications for the simse product family.

## Files

| File | Purpose |
|------|---------|
| `tokens.css` | CSS custom properties — the single source of truth for all design tokens |
| `tokens.ts` | TypeScript constants mirroring the CSS tokens for use in JS/TS code |
| `components.md` | Component specifications documenting variants, props, and behavior |

## How tokens.css works

`tokens.css` defines every color, font, spacing, layout, and animation value as CSS custom properties on `:root`. All other consumers derive their values from this file.

### Consumption in simse-app (Tailwind)

simse-app uses Tailwind CSS v4 with `@theme` to map these tokens into utility classes:

```css
@import '../../simse-brand/design-system/tokens.css';

@theme {
  --color-emerald: var(--color-emerald);
  --color-dark: var(--color-dark);
  --font-sans: var(--font-sans);
  --font-mono: var(--font-mono);
  /* ... */
}
```

This keeps Tailwind classes (e.g. `bg-emerald`, `font-mono`, `rounded-lg`) in sync with the brand tokens without duplicating values.

### Consumption in TypeScript

Import from `tokens.ts` when you need values in JavaScript — for example, in chart libraries, inline styles, or runtime logic:

```ts
import { colors, duration } from 'simse-brand/design-system/tokens';

element.style.backgroundColor = colors.emerald;
```

## Relationship between files

```
tokens.css  (source of truth)
   |
   +-- tokens.ts        (mirrors CSS values as typed TS constants)
   +-- simse-app/app.css (imports tokens.css, maps via @theme)
   +-- components.md     (references tokens by name)
```

Brand guidelines live in `simse-brand/guidelines/brand-guide.md`. The tokens formalize those guidelines into consumable values. If the brand guide and tokens ever conflict, update the tokens to match the brand guide.
