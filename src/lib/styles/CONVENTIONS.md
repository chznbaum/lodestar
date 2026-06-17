# Styles — Conventions

All CSS lives in `src/lib/styles/`. No `<style>` block in any `.svelte` file
(end-state). Svelte's auto-scoping is gone, so naming has to be collision-safe.
These are the rules; follow them for all new/renamed CSS.

## Naming & selectors

We use **two** selector styles, chosen by what is being styled:

1. **BEM classes** for non-semantic things — reusable atoms, block containers,
   and the structural parts of a block.
   - **Reusable atoms** = single block class: `.btn`, `.chip`, `.tab`, `.panel`,
     `.monogram`, `.cbx`, `.scrim`, `.modal`, `.hint`, `.error`.
   - **Parts of a block** = BEM element: `.panel__head`, `.panel__body`,
     `.companies__filters`, `.workspace__header`.
   - **Modifiers / states** = BEM modifier: `.btn--primary`, `.chip--danger`,
     `.tab--active`.

2. **Element selectors qualified by a block ancestor** for *genuine semantic
   elements* (`h1`, `h2`, `input`, `textarea`, `a`, `dt`, `dd`, `ul`, `li`,
   `button`, `p`): `.companies h1`, `.modal h2`, `.create-form input[type="text"]`,
   `.meta-grid dt`, `.notes p`.
   - **Never** slap a class on the element to style it — no `.input`, no `.h1`.
   - **Never** write a bare element rule (`p { … }`, `ul { … }`); it leaks to
     `@html` notes and everything else. The only bare element rules allowed are
     the deliberate global defaults in `reset.css` / `elements.css`.

**Block names match their component**, so "where is this styled?" is obvious:
`.companies` ↔ list page, `.workspace` ↔ detail page, `.create-form` ↔ create
modal, `.domain-picker` ↔ DomainPicker, `.rail` / `.app` ↔ layout.

## `.on`/`.good` boolean toggle vs `--modifier`

- **Prefer `--modifier`** (`.tab--active`, `.chip--danger`) for new and renamed
  classes. Pick one convention per block and keep it consistent within that block.
- **A boolean `.on` / `.good` class may stay** when the markup already toggles it
  via a `class:` directive (`class:on`, `class:open`, `class:selected`, …) and
  rewriting to a `--modifier` would be churn/risk for no benefit. Don't rename the
  base class out from under a `class:` directive without updating the directive too.

## Minimal utilities

- **No filler-span utilities.** Eliminate `.grow` / `.spacer` by giving the real
  adjacent element `margin-left: auto` in its block file (block-context layout).
- Only introduce a utility if a pattern recurs **≥3×** and block-context layout
  genuinely can't express it cleanly.

## Global defaults (the deliberate exceptions)

These are the only places bare element selectors are allowed:

- `reset.css` — true resets: `box-sizing`, `body` baseline, base `a` color, and
  `input/button/textarea/select { font: inherit }` (font only, no layout).
- `elements.css` — global element DEFAULTS: `h1`, `h2`, and the canonical text
  input/textarea (border, padding, font, focus ring). Block contexts add only
  layout on top (margin, line-height, flex) — they do not restyle these elements.
  New inputs need explicit `type="text"` / `type="url"` so the selectors match.

The combobox in-popover search input is the one deliberate input variant
(borderless/transparent) and overrides the global default via `.cbx-pop`.

## File layout & cascade order

`index.css` is the single entry point; `+layout.svelte` imports only it (plus the
`@fontsource` packages). Vite inlines the `@import`s. Cascade order is:

```
tokens.css      /* variables only — no output */
reset.css       /* resets */
elements.css    /* global element defaults (h1 / h2 / inputs) */
components/*     /* shared design-system pieces (button, chip, tabs, panel, …) */
layout/*         /* per-surface blocks (app, companies, workspace, …) */
```

Order = **tokens → reset → elements → components → layout**. Layout may override
components by source order at equal specificity; rely on that only for intentional
cases, not as a crutch for specificity battles.

Files are small and single-purpose (one component/surface each). All atoms now
live in `components/*` — the former `base.css` holding file has been removed.

## Tokens

Change the look in `tokens.css`, not in components. No hardcoded colors, shadows,
scrims, z-indexes, or magic font sizes in component CSS — reference a token. Use
the positional type scale (`--fs-2xs … --fs-2xl`). The old role-coupled size
aliases have been removed; use the positional names directly.
