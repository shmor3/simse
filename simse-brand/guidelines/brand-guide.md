# simse Brand Guidelines

## Brand Name
- **Product name**: simse (lowercase, always)
- **Never**: Simse, SIMSE, SimSe, simse.dev (when referring to the product, not the URL)
- **URL**: simse.dev
- **Internal only**: simse-code, simse-core, simse-vector (never customer-facing)

---

## Logo

### Logomark
The simse logomark is a hexagonal shape with a flowing wave curve inside, representing continuous evolution and adaptability.

### Variants
| Variant | File | Use case |
|---------|------|----------|
| White | `logos/logomark-white.svg` | Dark backgrounds (primary) |
| Emerald | `logos/logomark-emerald.svg` | Brand emphasis, feature highlights |
| Dark | `logos/logomark-dark.svg` | Light backgrounds |
| Favicon | `logos/favicon.svg` | Browser tabs, small sizes |

### Logo Usage Rules
- Minimum size: 16x16px
- Always maintain equal padding around the logo (minimum: 25% of logo width)
- Do not stretch, rotate, or distort the logomark
- Do not place the logo on busy or low-contrast backgrounds
- Do not add effects (shadows, gradients, outlines) to the logo
- When pairing with the wordmark "SIMSE", use Space Mono Bold, letter-spacing 0.35em

### PNG Exports
Available in `logos/png/` at: 16, 32, 64, 128, 256, 512, and 1024px

---

## Color Palette

### Primary
| Name | Hex | RGB | Usage |
|------|-----|-----|-------|
| Emerald | `#34d399` | 52, 211, 153 | Primary accent, CTAs, highlights, logo |
| Dark | `#0a0a0b` | 10, 10, 11 | Primary background |
| White | `#ffffff` | 255, 255, 255 | Headings, primary text on dark |

### Neutrals (Zinc scale)
| Name | Hex (approx) | Usage |
|------|-------------|-------|
| zinc-200 | `#e4e4e7` | Body text on dark backgrounds |
| zinc-400 | `#a1a1aa` | Secondary text, muted labels |
| zinc-500 | `#71717a` | Placeholder text |
| zinc-600 | `#52525b` | Disabled states |
| zinc-700 | `#3f3f46` | Borders, dividers |
| zinc-800 | `#27272a` | Card backgrounds, input fields |
| zinc-900 | `#18181b` | Elevated surfaces |
| zinc-950 | `#09090b` | Deepest background |

### Semantic
| Name | Hex | Usage |
|------|-----|-------|
| Success | `#34d399` | Confirmation, success states (same as emerald) |
| Error | `#ff6568` | Error messages, destructive actions |

### Color Rules
- Always dark-mode first — the brand is built around a dark aesthetic
- Emerald is the only accent color. Do not introduce additional brand colors.
- Use zinc scale for all neutral/gray needs. Do not use generic gray.
- Minimum contrast ratio: 4.5:1 for body text, 3:1 for large text

---

## Typography

### Font Families
| Role | Font | Weight | Package |
|------|------|--------|---------|
| Body / UI | DM Sans Variable | 100–1000 | `@fontsource-variable/dm-sans` |
| Code / Labels | Space Mono | 400 | `@fontsource/space-mono` |

### Type Scale
| Element | Size | Weight | Tracking | Line Height |
|---------|------|--------|----------|-------------|
| H1 (hero) | 64px / 4rem | 700 | -0.02em | 1.1 |
| H2 | 36px / 2.25rem | 700 | -0.02em | 1.2 |
| H3 | 24px / 1.5rem | 600 | -0.01em | 1.3 |
| Body | 16px / 1rem | 400 | normal | 1.5 |
| Small | 14px / 0.875rem | 400 | normal | 1.5 |
| Label (mono) | 10–11px | 400 | 0.1em uppercase | 1 |
| Logo wordmark | 14px (mono) | 700 | 0.35em | 1 |

### Typography Rules
- Use DM Sans for all body text, headings, and UI elements
- Use Space Mono only for: labels, tags, code snippets, the logo wordmark, and the footer
- Enable antialiasing: `-webkit-font-smoothing: antialiased`
- Headings use tight letter-spacing (-0.02em), body uses default

---

## Imagery & Visual Language

### Aesthetic
- Minimalist, dark-mode first
- Subtle micro-interactions (dot grids, hover effects)
- Clean whitespace, centered layouts
- Tech-forward but not intimidating

### Interactive Elements
- **Dot grid**: 24px spacing, 1px radius dots, white-to-emerald on hover
- **Typewriter**: Cycles through use cases at 100ms/char type speed
- **Cursor blink**: 800ms step-end animation

### Photography & Illustration
- No photography in current brand — pure UI and typography
- If illustrations are added, use line art in emerald/white on dark backgrounds
- Avoid stock photos, gradients, or skeuomorphic elements

---

## Social Media Sizes

| Platform | Asset | Dimensions |
|----------|-------|-----------|
| Twitter/X | Header | 1500 x 500 |
| LinkedIn | Banner | 1584 x 396 |
| GitHub | Social preview | 1280 x 640 |
| Discord | Banner | 960 x 540 |
| Avatar (all) | Profile picture | 400 x 400 |
| OG Image | Link preview | 1200 x 630 |

---

## Voice & Tone

### Brand Voice
- **Clear**: No jargon for jargon's sake. Explain things simply.
- **Confident**: State what simse does, don't hedge.
- **Concise**: Short sentences. No filler words.
- **Technical when needed**: The audience is tech-savvy. Use precise terms (ACP, MCP) but explain them.

### Tone Examples
- Good: "Context carries over. Preferences stick."
- Good: "An assistant that gets better the more you use it."
- Avoid: "Leveraging cutting-edge AI to revolutionize your workflow experience."
- Avoid: "simse is a game-changing paradigm shift in AI assistance."

### Writing Rules
- Product name is always lowercase: "simse"
- Don't use exclamation marks in product copy
- Lead with what it does, not what it is
- Use active voice
