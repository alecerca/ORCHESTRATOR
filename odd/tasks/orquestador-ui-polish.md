# Orquestador — UI polish (v1 feedback)

## Objective

Fix the first round of user-reported layout defects in the v1 shell so the app is
actually usable as a control tower: read long transcripts and open more than two
agent panes at once.

## Problem

1. **No scroll inside a pane.** Long transcripts cannot be read downward.
2. **Third pane is invisible.** With two panes open, opening a third splits the
   area again but the new pane never shows — it is pushed into a wrapped row that
   is clipped, and it reads as "hidden behind the Files panel".

## Root cause (evidence)

- `frontend/src/styles.css` `.pane-grid`: `display:flex; flex-wrap:wrap;
  overflow:hidden;` with `.pane { flex: 1 1 340px; min-width: 300px }`.
  When the panes no longer fit, flex moves the extra pane to a **second row**,
  and `overflow:hidden` **clips that row** → the pane is unreachable and no
  scrollbar is offered. There is no limit in JS (`App.tsx::togglePane` just
  appends to the array; `PaneGrid` maps every id), so this is a CSS-only defect.
- The pane scroll chain is not guaranteed: `.workspace` lacks `min-height: 0`,
  and `.pane` lacks an explicit `overflow`. `.stream` declares the right
  `overflow-y:auto`, but without a bounded pane it cannot take effect.

## Scope

- `frontend/src/styles.css` only (layout/overflow). No JS/TS behaviour change.
- Not in scope: resizable panes, drag-to-arrange, tabbed panes, persistence of
  the open-pane set (candidate follow-ups, waiting on user priorities).

## Tasks

- [x] **U1** — Tile N panes deterministically: replaced the wrapping flex with a
  CSS grid (`repeat(auto-fit, minmax(min(320px,100%), 1fr))` columns,
  `minmax(240px, 1fr)` rows); the grid scrolls only when panes cannot fit, so
  every open pane is visible and reachable.
- [x] **U2** — Guaranteed per-pane scrolling: `min-height: 0` + `overflow: hidden`
  on `.workspace`, `overflow: hidden` on `.pane`, hairline separation via `gap` +
  grid background instead of per-pane borders.
- [x] **U3** — Verified: `npx tsc --noEmit`, `npm run build` (vite),
  `scripts/verify-headless.sh` green; layout measured in headless Firefox (see
  Evidence).

## Evidence (measured, not assumed)

A static harness reproducing the shell structure (topbar + agent panel + 3 panes
+ Files panel) was rendered in headless Firefox at 1400x800, with the old CSS
(from `git show HEAD:...`) and the new one. Measurements were beaconed to a local
sink (`scripts`-less, throwaway in `/tmp`):

| Measurement | Before (old CSS) | After (fix) |
| --- | --- | --- |
| Pane A/B rect | `355x11164` (grew to content height) | `355x380` |
| Pane C rect | `y=11205` → `paneCVisible: false` | `y=421` → `paneCVisible: true` |
| `.stream` | `scrollH 11080 / clientH 11080` → **no scroll** | `scrollH 11080 / clientH 296` → **scrolls** |
| `.pane-grid` | `scrollH 11322 / clientH 760`, `overflow:hidden` → clipped | `scrollH 760 / clientH 760` |

Root cause confirmed empirically: the pane had no bounded height (it grew to
11164px), so `.stream` had nothing to scroll against, and the third pane landed
at `y=11205` — far below the 800px viewport.

## Acceptance criteria

1. Opening 3+ panes shows every pane (tiles into rows), with all of them
   reachable; each pane can be closed individually. — measured: yes (2x2 tiles)
2. A long transcript scrolls inside its own pane and the rest of the shell
   (top bar, side panels) stays fixed. — measured: yes
3. The workspace never bleeds under/over the Files (editor) panel. — structural
   (`overflow:hidden` on `.workspace`); to confirm on screen.
4. No regression: empty state, diff tab, follow-up input and Monaco keep working.
   — empty state kept on flex (grid would have stretched it); build green.

## Checks (per task)

- `cd frontend && npx tsc --noEmit && npm run build`
- `bash scripts/verify-headless.sh` (core tests + clippy + fmt + tsc + vite build)

## Progress log

- [x] Feature document created (2026-10-10) — root cause pinned to `.pane-grid`
  (`flex-wrap` + `overflow:hidden`) and the unbounded pane height chain.
- [x] U1+U2 implemented in `frontend/src/styles.css`; `verify-headless.sh` green
  (`OK — headless verification passed.`); headless-Firefox measurements above.
- [ ] Follow-up candidates (pending user priorities): resizable panes, tabbed
  panes, persisting the open-pane set; optional repo-committed layout harness
  (the throwaway one lives only in `/tmp`).
