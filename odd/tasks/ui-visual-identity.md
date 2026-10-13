# Orquestador — Visual identity (palette, type scale, density)

## Objective

Give the shell a deliberate visual identity instead of the current generic
dark-purple dev-tool look, and fix the density problem that makes the app feel
small, compressed and text-heavy:

1. **Identity**: a real palette and elevation system with a point of view.
2. **Legibility**: bigger type and more whitespace.
3. **Hierarchy**: the eye should know where to land; less "wall of text".
4. **No content deleted** — every string stays; the fix is size, space,
   hierarchy and progressive disclosure, never removing information.

## Problem (evidence, measured on the branch)

| Signal | Measurement | Reading |
| --- | --- | --- |
| Type size | 39× `12px`, 31× `13px`, 16× `11px`, 5× `10px` | 70 rules at 12–13px; the UI is effectively a 12px UI |
| Body base | `body { font-size: 14px }`, but almost nothing uses it | the base is nominal, not real |
| Weights | only `600` (16×) and `500` (1×) | no hierarchy: everything is 400 or 600 |
| Scales | no `--fs-*`, `--sp-*`, `--radius-*`, no elevation tokens | every value is a hardcoded literal → no lever to change density |
| Palette | `#16151a` purple-black, stock green `#3ba55d`, blue `#4c9eeb` | generic; reads as "default template" |
| Sheets | `styles.css` = 2274 lines, 34 uses of `var(--brand)` | tokens are the right lever; palette swap is cheap |

**Why it feels soulless**: a purple-black base + a stock green with no accent
system, a single emphasis weight, and 12px everywhere. **Why it feels
compressed**: no spacing scale, so padding is minimal and consistent-but-tight,
and everything is the same size, so nothing leads.

## Scope

- `frontend/src/styles.css`: the token layer and the density/hierarchy sweep.
- A small number of components only where progressive disclosure or a label is
  needed (secondary text demoted, section headings, tooltips).

## Constraints

- **Preserve every token name already in use** (`--bg`, `--surface`,
  `--surface-alt`, `--border`, `--text`, `--muted`, `--brand`, `--brand-text`,
  `--secondary`, `--warning`, `--error`, `--mono`, `--sans`). Only add; never
  rename, or 2274 lines break.
- No behaviour change, no wire change, no JS logic change.
- No new dependency, no CSS framework.
- Keep the light theme working (`html[data-theme="light"]`).

## Out of scope

- Component-level UX redesign beyond density/hierarchy/disclosure, except for
  the explorer and the tab strip, which U6 covers.
- Motion/choreography beyond simple transitions.
- Layout restructuring and new features.

## Design direction — "Night Console"

A control tower read: cool near-black graphite (not purple), one confident
accent, real elevation, monospace reserved for data.

**Dark**

| Token | Value | Role |
| --- | --- | --- |
| `--bg` | `#0b0e13` | shell background, cool and deep |
| `--surface` | `#12161e` | panels |
| `--surface-alt` | `#191e28` | raised/secondary panels |
| `--surface-raised` | `#202633` | menus, popovers (new) |
| `--border` | `#242b38` | hairline |
| `--border-strong` | `#333c4d` | emphasised separators (new) |
| `--text` | `#e9edf5` | primary text |
| `--muted` | `#93a0b6` | secondary text |
| `--brand` | `#34d07f` | alive emerald — "go" |
| `--brand-text` | `#04150c` | dark ink on brand (brand is now bright) |
| `--secondary` | `#6aa6ff` | action blue |
| `--accent` | `#29d3c4` | identity teal: focus ring, live indicators (new) |
| `--warning` | `#ffb02e` | attention |
| `--error` | `#ff5a5f` | failure |

**Light**

| Token | Value |
| --- | --- |
| `--bg` | `#e9ebef` |
| `--surface` | `#f7f8fb` |
| `--surface-alt` | `#eef0f5` |
| `--surface-raised` | `#fcfdff` |
| `--border` | `#d7dbe2` |
| `--border-strong` | `#b9c0cb` |
| `--text` | `#1b2129` |
| `--muted` | `#5b6573` |
| `--brand` | `#0b7d47` |
| `--brand-text` | `#ffffff` |
| `--secondary` | `#1565c0` |
| `--accent` | `#0b6d86` |
| `--warning` | `#7a5200` |
| `--error` | `#c02b38` |

Light mode shipped softer than first drafted: the pure `#ffffff` shell and
panels glared once the denser type/spacing landed, so the shell is now a
low-glare grey and only the panels stay near-white.

**Scales (new tokens)**

- Type: `--fs-2xs: 11px`, `--fs-xs: 12px`, `--fs-sm: 13px`, `--fs-md: 14px`,
  `--fs-base: 15px`, `--fs-lg: 18px`, `--fs-xl: 22px`, `--fs-2xl: 28px`
- Space: `--sp-1: 4px` … `--sp-6: 32px`
- Radius: `--radius-sm: 6px`, `--radius-md: 9px`, `--radius-lg: 14px`
- Elevation: `--shadow-1`, `--shadow-2`

## Tasks

- [x] **U1 — Token layer.** Replace the `:root` and light-theme blocks with the
  "Night Console" palette; add the type, space, radius and elevation scales.
  Keep every existing token name. *Exit: build green; palette visibly changes.*
- [x] **U2 — Density sweep.** Raise the type floor: `10→11, 11→12, 12→13,
  13→14, 14→15, 15→16, 16→18, 18→20, 20→22, 22→24` on `font-size`
  declarations; raise line-height where missing; add breathing room to the main
  containers (shell, panels, toolbars, lists) via the space scale.
  *Exit: build green; nothing clipped or overflowing.*
- [x] **U3 — Hierarchy pass.** Titles (`welcome__title`, `empty-state__title`,
  `approval__title`, `group-infobar__title`, `agent-card__title`) lifted to 700;
  the transcript role label became a bold micro-label carrying the accent tint on
  the agent side. *Exit: build green.*
- [x] **U4 — Progressive disclosure.** Tool output past 700 characters folds
  behind a `<details>` whose summary carries the line count, so the transcript no
  longer pays a screenful per call. Short output still renders inline. No
  information removed.
- [x] **U5 — Light palette softening.** Light-theme surfaces dropped pure
  `#ffffff` for a low-glare grey shell with near-white panels (see the table).
- [x] **U6 — Kind icons + tab strip.** New `frontend/src/components/icons.tsx`
  (hand-written SVG, no dependency): a page tinted by the file's extension, an
  opening folder, a terminal for session tabs, a spark for skill tabs. Wired into
  `FileTree` and `TabStrip`; the tab close affordance now waits for
  hover/active/focus instead of adding noise to every tab.
- [ ] **U7 — Verification.** `npx tsc --noEmit` + `npm run build`; a human pass
  over the running app (dark + light) for contrast, clipping and focus. Needs a
  second pair of eyes: the agent cannot see the rendered result.

> U5/U6 are new (they came out of the direction review); the verification task
> that used to be U5 is now U7.

## Acceptance criteria

1. The app has a recognisable identity: cool graphite base, one accent, real
   elevation — not the stock purple-gray + green template.
2. Body/content text is meaningfully larger and better spaced; the UI no longer
   reads as a 12px UI.
3. Clear hierarchy: a first-time look has an obvious entry point per screen.
4. Less text competing at once, achieved by size/space/hierarchy/disclosure —
   **no information deleted**.
5. Dark and light themes both legible; no clipping, no overflow, no contrast
   regressions on buttons (brand is bright → ink text, not white).
6. Build stays green: `npx tsc --noEmit` + `npm run build`.

## Checks (per task)

- `cd frontend && npx tsc --noEmit && npm run build`
- Human pass in the running app for dark + light (U5).

## Delivery

- Chain: `feat/ui-ux-onboarding` (PR 1) → **`feat/ui-visual-identity`** (PR 2,
  this slice), child targeting the parent branch.
- Forecast: ~400–700 authored changed lines (CSS-dominant). A re-skin does not
  split cohesively across files; if the total exceeds the 400-line review
  budget, recommend `size:exception` rather than an artificial split.

## Progress log

- [x] Feature document created (2026-10-12) on branch `feat/ui-visual-identity`
  (branched from `feat/ui-ux-onboarding`).
- [x] U1 — token layer + ad-hoc shadows rewired onto `--shadow-1/2`.
  `ec5f3df` feat(ui): introduce a "Night Console" identity (palette, scales,
  elevation). Build green.
- [x] U2 — type floor raised one step on every `font-size`, `body` line-height
  1.5, paddings/gaps widened one step, fixed chrome grown (topbar 48px, its
  buttons 30px, rail buttons 36px). `3942c7e`. Build green.
- [x] U3 + U5 — hierarchy pass and the light palette softening.
  `5ee5f91` feat(ui): soften the light theme and lift heading hierarchy.
- [x] U4 — progressive disclosure (tool output folds past 700 characters).
  `d1de3d6` feat(ui): fold long tool output behind a disclosure.
- [x] U6 — kind icons + tab strip polish (4 files, new `icons.tsx`).
  `730680c` feat(ui): give the explorer and the tab strip kind icons.
- [x] Direction approved on review ("vale, si vamos por ahi con este diseno"),
  with three follow-ups folded in as U5/U6: soften light mode, touch the agent
  console tabs, add file/folder icons by extension.
- [ ] U7 — verification + human pass (dark + light). Not yet run.
- [ ] Branch `feat/ui-visual-identity` is committed but **not pushed**; no PR
  opened. Waiting on the user's OK.
