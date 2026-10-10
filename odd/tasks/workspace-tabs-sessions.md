# Workspace tabs + session CRUD — feature document

- **Feature**: `workspace-tabs-sessions`
- **Directory**: `/home/sanchiro/projects/orquestador` (git, `main`, no remote)
- **Created**: 2026-10-10
- **Delivery strategy**: `single-branch` (repo policy: work-unit commits land on `main`; no remote, so no PR chain is possible)
- **Delivery forecast**: authored changed lines (adds+deletes) well above the ~400 planning heuristic — this is one coherent feature delivered as sequential work-unit commits on `main`, not artificially split.

## Problem (verified, 2026-10-10)

The workspace is a fixed set of docked columns: `ActivityBar | AgentPanel | workspace(PaneGrid) | EditorPanel`, with `Composer` below. Two concrete pain points:

1. **Files open inline, not as documents.** `EditorPanel` (`EditorPanel.tsx:16`) keeps the `FileTree` and the viewer side by side in the same right column; opening a file replaces the viewer next to the tree. There is no tab strip.
2. **The workspace has no tabs and no splits.** `panes: string[]` (`App.tsx:88`) is a flat list of open session ids rendered as a plain grid of `AgentPane` (`PaneGrid.tsx:11`). Each `AgentPane` carries its own header (title + `StatusChip` + Abort + ✕). **Panes are not persisted** — everything closes on reload.
3. **Sessions cannot be created from the UI, so the root picker is useless when only one session exists.** The backend was already complete and simply never wired:
   - core: `create_session` (`client.rs:227`), `delete_session` (`client.rs:243`), `rename_session` (`client.rs:253`)
   - shell: commands registered (`commands.rs:56-100`)
   - frontend: `createSession` / `deleteSession` / `renameSession` already exported (`commands.ts:33-40`)
   - events: `session.created` / `session.updated` / `session.deleted` already reduce into state (`state.rs:372-377`), so a created session appears on its own.

## Objective

A VS Code–style workspace: **one tab = one document**, documents are *agent history* and *file*; drag/context-menu splits into halves; a thin info bar below the tab strip; plus real session create/rename/delete from the panel.

## Decisions (settled)

- **Model: VS Code pure** (user's choice). Tabs live in split groups; "moving" a tab means dragging it between groups. **No floating overlay windows** (explicitly deferred as a possible phase 2; the tab model must not preclude it).
- **One tab = one document.** Two kinds: `session` (agent history, root or sub-agent) and `file`.
- **Each group** has its own tab strip and its own thin info bar.
- **Tab strip (VS Code style)**: the active tab is illuminated ("focused") and shows its ✕; inactive tabs are dimmed.
- **Info bar**: a thin, full-width strip directly below the tab strip, carrying the active tab's info — status (`running`/`waiting_approval`/`idle`/`done`/`error`), elapsed timer, and the `Abort` action. This is the old `AgentPane` header relocated below the tabs.
- **Splits**: drag a tab onto a half → a new **empty** group opens on that side to drop into; right-click a tab → "Split right" / "Split down". Dragging the divider resizes; closing a group's last tab closes the group.
- **File tree** becomes a togglable **side panel** (ActivityBar); clicking a file **opens a tab**. The right column as an inline viewer is removed.
- **Persistence**: the group/tab layout is persisted in `localStorage` under a versioned key (panes are lost on reload today — a UX bug fixed here).
- **Session CRUD is additive**: the existing **Hide** (UI-local dismiss, non-destructive) stays; **Delete** is new, server-side, and requires explicit confirmation.
- **Accessibility**: tabs are a real `role="tablist"`/`role="tab"` with keyboard navigation (arrow keys), the divider is keyboard-resizable or has an accessible alternative, and every icon button has a `title`/`aria-label`.

## Scope

**In scope**: tab strip + info bar + tab activation/close, split groups (drag, context menu, resize, empty-group drop), file-as-tab, agent-history-as-tab, layout persistence, session create/rename/delete UI.

**Out of scope**: floating/undockable windows; tab reordering animation polish; multi-window (OS) support; collaborative/session sharing.

## Edge cases to cover

- Deleting the session that is the effective root or the persisted pick → clear the pick (fallback to `root_session_id`).
- A tab whose session disappears (deleted server-side) → close the tab and drop its stream.
- Closing the last tab of a group → the group collapses; closing the last group → the empty-workspace state.
- Drag over the content area vs. the tab strip (drop into an existing group moves the tab; drop on a half creates a group).
- Empty halves must not persist as ghosts after a reload.
- A file tab for a missing/binary file → the existing error/`binary` messaging, not a crash.
- Duplicate open: clicking an already-open file/session focuses its existing tab instead of opening a second one.

## Tasks

- [x] **T1 — Session CRUD UI (frontend)**: `AgentPanel` header gets **New session** (`createSession(undefined, "New session")` then `onPickRoot(id)`); `AgentCard` gets double-click **rename** (input; Enter commits, Esc cancels, blur commits-if-changed, empty/unchanged cancels) and a two-step inline **delete** confirm; `App.deleteSession` awaits the server call, then closes the pane, drops the stream, clears a stale pick and un-dismisses. Hide untouched; the root card still never receives `onDismiss`. Route: delegated frontend writer.
- Known limitation (T2): parentless sessions other than the effective root have no row in the panel (they only appear in the `<select>`), so they cannot be renamed/deleted from the UI yet.
- [x] **T2 — Tab model + strip + info bar (frontend)**: `panes: string[]` replaced by a pure `Layout`/`TabGroup`/discriminated-`Tab` model in `frontend/src/tabs.ts` (helpers: open/focus, close, close-session, close-all, activate tab/group, prune); per-group `TabStrip` (`role="tablist"`/`role="tab"`, illuminated active, ✕, arrow/Home/End roving focus) and thin `GroupInfoBar` (title, `AgentTimer`, `StatusChip`, Abort only while running/waiting_approval) under it; the per-`AgentPane` header was removed. Layout persists to `localStorage` (`orquestador.layout`), gated on the first snapshot (`storeReady`) so a restored layout is never wiped or pruned against the empty pre-bootstrap state. Streams drop only when no tab anywhere references the session. Multiple groups render side by side with a divider — the split affordance is T3. Route: delegated frontend writer.
- T2 decisions: an empty-but-present persisted layout counts as "persisted" (close-all + reload stays empty instead of re-opening the root, VS Code semantics); clicking anywhere in a group activates it, so newly opened tabs land there.
- [x] **T3 — Splits (frontend)**: the flat `groups[]` row was promoted to a nested split tree (`SplitNode { orientation: horizontal|vertical, children, sizes }` | `GroupNode`); every T2 helper is tree-aware and `parseLayout` migrates the flat persisted layout into one horizontal split. Affordances: tabs are draggable with a four-zone drop indicator, and dropping **moves** the tab into a new group created on that side of the *hovered* group; right-click a tab → **Split right** / **Split down** / **Close tab** (acts on the right-clicked tab); a draggable `SplitDivider` between siblings resizes them with pointer capture or the arrow keys (`sizes`, persisted); a group that loses its last tab collapses and a split with fewer than two children collapses into its parent. Route: delegated frontend writer (a first pass landed the tree model and stopped mid-way; the parent re-delegated a completion pass and fixed two remaining defects itself).
- [ ] **T4 — File as tab (frontend)**: the right column becomes **only the file explorer** (`FileTree`); the inline file viewer is removed. Opening a file from the tree creates or focuses a `file` tab in the workspace grid rendering `MonacoEditor`; the `Tab` union already has a `kind` field for it. Route: delegated frontend writer.
- [ ] **T5 — Verify + commits**: `npx tsc --noEmit` + `npm run build` + `scripts/verify-headless.sh`; work-unit commits on `main`. Route: parent.

## Acceptance criteria

1. From the panel I can create a session, see it appear without a manual reload, switch the root to it, rename it, and delete it (with confirmation).
2. Creating a session makes it usable immediately (it becomes the effective root).
3. Opening a file opens a tab, not an inline viewer; clicking it again focuses the existing tab.
4. Agent histories and files are both tabs; the active tab is highlighted and closes with its ✕; a thin info bar below the strip shows the active tab's status, timer, and Abort.
5. I can split by dragging a tab to a half or via right-click; halves are resizable; an empty half can receive a dragged tab.
6. The layout survives a reload; deleting a session closes its tabs and clears a stale root pick.
7. No regression: `tsc` clean, `npm run build` green, headless verification green.

## Verification

- Frontend (no test runner by design, `SPEC.md:85`): `npx tsc --noEmit` + `npm run build`.
- Core unchanged: `cargo test -p orquestador-core` stays green; `scripts/verify-headless.sh` green.
- Manual acceptance by the user in the running app.

## Risks

- Drag-and-drop without a dependency is fiddly (pointer events vs HTML5 DnD). Prefer pointer events on the tab for a consistent feel across the webview, and keep the right-click path as the always-available fallback.
- The tab model rewrite touches `App.tsx`, `PaneGrid`, `AgentPane`, `EditorPanel`; T2 must land before T3/T4 to avoid overlapping rewrites.

## Progress log

- [x] Feature document created (parent, 2026-10-10) after verifying the backend CRUD surface already exists.
- [x] T1 implemented (delegated frontend writer, 4 files, +234/−20): no core/shell change was needed. Verification: `npx tsc --noEmit` clean, `npm run build` green, `npm run typecheck` clean, `scripts/verify-headless.sh` → "OK — headless verification passed" (66 core tests). Parent spot check re-ran `tsc` (green).
- [x] T2 implemented (delegated frontend writer, 3 new files + 5 changed): `tabs.ts`, `TabStrip.tsx`, `GroupInfoBar.tsx` new; `App.tsx`, `PaneGrid.tsx`, `AgentPane.tsx`, `AgentPanel.tsx`, `styles.css` changed. Verification: `tsc --noEmit` clean, `npm run build` green (only the pre-existing Monaco chunk warning), `verify-headless.sh` OK (66 tests). Parent spot check re-ran `tsc` (green).
- T2 follow-up: the split affordance (drag a tab to a half / right-click "Split right"/"Split down") is T3 and was deliberately not built; file tabs are T4.
- [x] T3 implemented: `tabs.ts` tree model + `splitGroup`/`moveTab`/`updateSplitSizes`/`parseLayout` migration; new `SplitDivider.tsx`; `PaneGrid.tsx` recursive render + drop zones; `TabStrip.tsx` draggable tabs + context menu (+ Escape / outside-pointerdown dismiss); `App.tsx` wiring; `styles.css`. The first writer pass landed the tree and stopped mid-way (no DnD, no divider, no menu dismissal, and it reported two files it never created); a second bounded pass completed those, and the parent fixed two defects found in review: (a) `moveTab` anchored the new group to the *source* group while the drop side was measured on the *hovered* group — the signature now carries `targetGroupId`; (b) `SplitDivider`'s keyboard resize divided an already-fractional step by the pixel span, making arrow resizing a no-op. Verification: `tsc --noEmit` clean, `npm run build` green, `verify-headless.sh` OK (66 tests); parent spot checks re-run. Route: 2 delegated writer passes + parent fixes.
- T3 known gaps (accepted for now): dragging a tab onto its *own* group is a no-op, so same-group splitting is only reachable through the context menu; `dataTransfer` values are unreadable during `dragover`, so the indicator also shows over the source group; divider weights ignore the divider's own few pixels (~1–2% mapping error).
- User reminder (2026-10-10): opening a file must open a **tab** like any other document, and the right column keeps **only** the file explorer — no inline viewer.
- [x] T1 follow-up polish committed `353ab04` (user request): the Delete trigger is now a quiet ghost button like Open/Hide, and the inline confirmation got a surface/border frame with danger red reserved for Confirm.
