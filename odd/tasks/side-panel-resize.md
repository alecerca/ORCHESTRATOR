# Resizable side panels — feature document

- **Feature**: `side-panel-resize`
- **Directory**: `/home/sanchiro/projects/orquestador` (git, branch `feat/side-panel-resize`)
- **Created**: 2026-10-11
- **Delivery strategy**: `ask-on-risk` (default). Work-unit commit on the feature branch.
- **Delivery forecast**: small-to-moderate (one new component + wiring + CSS), well under the ~400-line planning heuristic.
- **Depends on**: none (uses the existing `SplitDivider` pattern as precedent).

## Objective

Let the user **drag to resize** the two fixed-width side panels — the left
**agents** panel (`AgentPanel`) and the right **editor** panel (`EditorPanel`) —
exactly like the split panes between tab groups already resize. Persist each width
across launches.

## Problem

`App.tsx` lays out `.app__body` as a flex row: `ActivityBar | AgentPanel |
workspace | EditorPanel`. Only the panes **inside** the workspace (`PaneGrid`) can
be resized, because `SplitDivider` renders between split-tree children. The two
outer panels are pinned:

- `.agent-panel { width: 270px; flex-shrink: 0; }` (`styles.css:354`)
- `.editor-panel { width: 420px; max-width: 45vw; flex-shrink: 0; }` (`styles.css:1442`)

So the user can rebalance tab groups but cannot widen the agent list or the file
explorer/diff. That is the gap.

## Verified facts (2026-10-11)

- Left-to-right flex order in `App.tsx:406-477`: `ActivityBar`, then `agentPanelOpen
  && <AgentPanel/>`, then `<main className="workspace">` (contains `PaneGrid`),
  then `editorOpen && <EditorPanel/>`.
- `SplitDivider` (`frontend/src/components/SplitDivider.tsx`) is the resizer
  precedent: pointer capture + `pointermove`, arrow keys, `role="separator"`,
  `aria-orientation`, `tabIndex=0`. It is **weight-based** and tied to the
  split-tree (`onResize(sizes[])`), so it cannot be reused directly for a
  free-standing pixel-width panel.
- `.split-divider` styling (`styles.css:708-730`): `flex: 0 0 auto`, 5px track,
  `cursor: col-resize`, `--brand` on hover/focus.
- Panels are `flex-shrink: 0`; `.workspace` is `flex: 1; min-width: 0` — so panel
  widths are the independent variables and the workspace absorbs the rest.
- No frontend test runner (`frontend/package.json`: only `dev`/`build`/`preview`/
  `typecheck`; `build` = `tsc --noEmit && vite build`). No `*.test.*` files exist.

## Scope

In scope:

- A new reusable `PanelResizer` component: pointer-drag + arrow-key resize of a
  single panel, clamped between `min`/`max`, reporting the new width in pixels.
- App-level width state for both side panels, restored from and persisted to
  `localStorage`, clamped on load.
- Render a resizer between the agents panel and the workspace, and between the
  workspace and the editor panel (only when the corresponding panel is open).
- `AgentPanel` and `EditorPanel` accept a `width` prop and apply it.
- `.panel-resizer` styles consistent with `.split-divider`.

Out of scope:

- Resizing `ActivityBar` or the `Composer` (bottom) bar.
- Collapsing panels (already handled by `ActivityBar` toggles).
- Any backend/Rust change.

## Design decisions

- **Pixel widths, not weights.** A `PanelResizer` takes `width`, `min`, `max`,
  `grow` (`+1` when dragging right widens the panel — left panel; `-1` for a
  right-hand panel) and `onWidth(newWidth)`. Drag captures the start width and
  applies `clamp(startWidth + grow * dx)`; arrow keys step by a fixed px.
- **Separate from `SplitDivider`.** Reusing the weight vector would couple the
  side panels to the split tree for no benefit; a small dedicated component keeps
  each concern honest. (Follows the existing `SplitDivider` interaction contract.)
- **Bounds**: agents `[180, 560]` default `270`; editor `[280, 720]` default
  `420`. CSS `min-width` guards are set at or below the JS minima so the two never
  disagree; the editor's `max-width: 45vw` is replaced by a width that the JS
  clamp owns (no silent visual clamp that would detach the handle).
- **Persistence keys**: `orquestador.agentPanelWidth`, `orquestador.editorPanelWidth`.

## Tasks

- [x] **T1 — `PanelResizer` component.** New
  `frontend/src/components/PanelResizer.tsx`: pointer capture + `pointermove`
  resize, arrow keys (±16px), `role="separator"`, `aria-orientation="vertical"`,
  `aria-label`, `aria-valuenow/min/max`, `tabIndex=0`. Clamps every emitted width.
- [x] **T2 — App wiring.** `frontend/src/App.tsx`: width state seeded via a
  `loadWidth(key, fallback, min, max)` helper; persist both in effects (matching
  the existing try/catch pattern); render a `PanelResizer` after `AgentPanel` and
  before `EditorPanel`, each gated on its panel being open.
- [x] **T3 — Panels + CSS.** `AgentPanel.tsx` / `EditorPanel.tsx` take a `width`
  prop and set it inline; `styles.css` gets `.panel-resizer` (mirroring
  `.split-divider`) plus `min-width` guards and the editor `max-width` adjustment.
- [x] **T4 — Verify.** `npx tsc --noEmit` and `npx vite build` (from `frontend/`).

## Acceptance criteria

- Dragging the handle between the agents panel and the workspace resizes the
  agents panel; dragging the handle between the workspace and the editor resizes
  the editor panel.
- Widths are clamped to their bounds; the workspace never collapses below its
  content and neither panel can be dragged past its min/max.
- Arrow keys on a focused handle resize the adjacent panel.
- Widths survive a reload (persisted) and are re-clamped on load.
- Handles are hidden when their panel is closed.
- `tsc --noEmit` and `vite build` pass.

## Verification

- `cd frontend && npx tsc --noEmit`
- `cd frontend && npx vite build`
- Manual: drag + keyboard resize both panels; reload and confirm persistence.
  (No frontend test runner exists — TDD exception documented; verification is
  typecheck + build + manual.)

## Progress

- 2026-10-11: feature doc created; docs commit (`docs: lead README with the pain
  and add the launch kit`) landed on `main` before branching. Implementation
  delegated to a bounded writer.
- 2026-10-11: T1–T4 complete. Verification evidence: `npx tsc --noEmit` → exit 0
  (worker run, re-confirmed by the orchestrator as a spot check); `npx vite build`
  → `✓ 1133 modules transformed`, `✓ built in 21.00s` (worker run; pre-existing
  >500 kB chunk advisory only). Manual drag/keyboard/persistence check still owed
  (no GUI in this environment). Implementation commit: _recorded on the branch._
