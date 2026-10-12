# Git panel controls — resizable split + branch switcher

## Objective

Make the `Git` tab of the right editor panel interactive:

1. **Resizable split** between the commits list and the files-of-commit list, by
   dragging a divider (same feel as the side panels already have).
2. **Branch switcher**: the branch name in the Git panel header becomes a button
   that opens a list of the project's local branches and switches to the selected
   one with a **real `git checkout`**, so the working tree changes and the running
   agents move to the target branch.

## Problem

The read-only Git panel (delivered in `git-viewer`) has a fixed layout: the
commits pane is capped at `max-height: 40%` and cannot be resized. There is also
no way to see or change the branch the project is standing on; the branch label is
a plain `<span>`.

## Why

- Users reading long commit messages or long file lists need to rebalance the two
  panes for the content at hand.
- Switching branch is a core workflow for the orchestrator: agents and their
  working directory should follow the branch the user picks, without leaving the
  app.

## Scope

In scope:

- Core read-only `branches(dir)` and the **only mutating** git op `checkout(dir, name)`.
- Two Tauri commands (`git_branches`, `git_checkout`) and their frontend bindings.
- `GitViewer`: vertical resizable divider between the two panes; branch button +
  dropdown listing local branches; checkout on select; refresh of the panel.
- Post-checkout refresh of the rest of the UI: file explorer remount, git-branch
  cache invalidation (session chips), and open file tabs re-read.
- CSS for the divider usage and the branch dropdown.

Out of scope (explicitly):

- Creating, deleting, renaming, or force-switching branches; pull/push/fetch.
- Remote branch tracking, upstream/behind/ahead display.
- Persisting the divider ratio across app restarts (session-only, like other
  ephemeral panel state).
- Stashing or discarding local changes; the checkout must fail cleanly when the
  worktree is dirty.

## Constraints

- **The checkout must never destroy work.** Run a plain `git checkout <name>`:
  no `--force`, no `-f`, no `-B`, no `--discard-changes`, no `stash`. A dirty tree
  that conflicts must fail and surface a friendly reason.
- The branch name is passed as a single argument (never a shell string). Reject
  empty names, names starting with `-`, and names containing ASCII whitespace or
  control characters.
- Keep the existing code style: `git.rs` error mapping via a `friendly_git_error`
  helper, blocking work on `spawn_blocking`, typed bindings in `commands.ts`,
  wire types in `types.ts`.
- No new dependencies (Rust or npm). No new npm scripts.
- UI copy in English. Conventional commits. No AI attribution.
- Frontend has no test runner → verification is `tsc --noEmit` + `vite build`
  (+ manual GUI). The Rust core has tests → **RED first**, then GREEN.

## Authorized scope (edit surfaces)

- `crates/orquestador-core/src/git.rs`
- `src-tauri/src/commands.rs`
- `src-tauri/src/lib.rs`
- `frontend/src/types.ts`
- `frontend/src/commands.ts`
- `frontend/src/components/GitViewer.tsx`
- `frontend/src/components/EditorPanel.tsx`
- `frontend/src/components/useGitBranch.ts`
- `frontend/src/components/PaneGrid.tsx`
- `frontend/src/components/FileViewer.tsx`
- `frontend/src/App.tsx`
- `frontend/src/styles.css`

## Tasks

- [x] **G1 — Core: list local branches (TDD).**
      In `crates/orquestador-core/src/git.rs` add a serializable `Branch { name, current }`
      and `pub fn branches(dir: &Path) -> Result<Vec<Branch>, String>` reading
      `git -C <dir> for-each-ref --format=%(refname:short) refs/heads` (read-only),
      marking the current branch from `branch_for_dir`. Current branch first, then
      the rest sorted by name. Errors map through `friendly_git_error`.
      Tests (write first, see RED): lists all local branches with exactly one
      `current`; errors outside a repo.

- [x] **G2 — Core: checkout a branch (TDD).**
      `pub fn checkout(dir: &Path, name: &str) -> Result<(), String>` running
      `git -C <dir> checkout <name>` (no force/stash flags). Reject an invalid name
      (empty, leading `-`, or containing whitespace/control chars) with
      `Err("invalid branch name")` **before** spawning git. Map stderr through
      `friendly_git_error`, adding a short case for "would be overwritten by
      checkout" → a message telling the user to commit or stash first.
      Tests (write first, see RED): switches to an existing branch (assert
      `branch_for_dir` follows); rejects `""`, `"-x"`, `"a b"`; fails on a dirty
      worktree conflict and leaves the branch unchanged.

- [x] **G3 — Shell commands.**
      In `src-tauri/src/commands.rs` add `#[tauri::command] git_branches(app)` (uses
      `active_directory`, `spawn_blocking`) and `git_checkout(app, name: String)`.
      Register both in `src-tauri/src/lib.rs` `invoke_handler`.

- [x] **G4 — Frontend bindings.**
      `frontend/src/types.ts`: `export interface GitBranch { name: string; current: boolean }`.
      `frontend/src/commands.ts`: `gitBranches()` and `gitCheckout(name)`.

- [x] **G5 — GitViewer: resizable split.**
      Wrap the commits and files panes in a flex-column container
      `.gitviewer__panes` (the `containerRef`), render `SplitDivider` with
      `orientation="vertical"`, `index={0}`, `sizes` state (default `[0.4, 0.6]`),
      and apply the weights as inline `flex` on each pane. Remove the static
      `max-height: 40%` / `flex-shrink` / `border-bottom` from `.gitviewer__commits`
      and the static `flex: 1` from `.gitviewer__files` (keep `overflow:auto` /
      `min-height:0` / padding). Update `.gitviewer__panes` CSS.

- [x] **G6 — GitViewer: branch switcher.**
      Turn the branch label into a button that toggles a dropdown
      (`role="menu"`) listing `gitBranches()`, the current one marked. Selecting a
      branch calls `gitCheckout(name)`, then `loadLog()`, and invokes an
      `onBranchChanged` callback. Close on Escape / outside click. Show an inline
      error when the checkout fails (e.g. dirty worktree) and never leave the
      panel in a stale "switching" state.

- [x] **G7 — Post-checkout refresh.**
      `EditorPanel`: accept `filesVersion: number` and `onBranchChanged: () => void`,
      remount `FileTree` via `key={filesVersion}`. `App`: hold a `filesVersion`
      counter and a `handleBranchChanged` that bumps it and calls
      `invalidateGitBranches()`. `useGitBranch.ts`: export `invalidateGitBranches()`
      that clears the module cache and notifies mounted hooks (subscribe via a
      version counter, e.g. `useSyncExternalStore`) so session chips re-read.
      `PaneGrid` → `FileViewer`: thread `filesVersion` and include it in the file
      content effect deps so open file tabs re-read after a switch.

- [x] **G8 — Styles.**
      Add `.gitviewer__panes` and the branch-button/menu styles (current branch
      marked, hover/active states, keyboard focus). Reuse the existing
      `--border` / `--brand` / `--muted` tokens; no new colors.

## Acceptance criteria

- Dragging the divider between the commits and files panes resizes both; neither
  collapses below ~10%; keyboard arrows resize it; the layout still works with an
  empty repo and when a pane is empty.
- The header shows the branch as a button; opening it lists the local branches,
  marks the current one, and switching performs a real `git checkout` in the
  project directory.
- A dirty-worktree conflict fails with a readable message and the branch is
  unchanged.
- After a successful switch: the Git panel shows the new branch and its commits;
  the file explorer reflects the new tree; session branch chips re-read; open file
  tabs re-read their content.
- `cargo test -p orquestador-core --lib`, `cargo test --workspace`,
  `cargo check --manifest-path src-tauri/Cargo.toml --tests`, `npx tsc --noEmit`,
  `npx vite build` all pass.

## Verification

- Rust core: `cargo test -p orquestador-core --lib` (G1/G2 tests, RED then GREEN).
- Workspace: `cargo test --workspace`.
- Shell: `cargo check --manifest-path src-tauri/Cargo.toml --tests` and without `--tests`.
- Frontend: `cd frontend && npx tsc --noEmit && npx vite build`.
- Manual (user, after build): drag the divider; open the branch menu; switch to
  another branch and confirm the tree/panel/chips update; switch back.

## Progress

- 2026-10-12 — Feature created. Recon complete (resizer primitives, GitViewer
  structure, project-change flow, branch cache, refresh gaps). No source written
  yet. Branch `feat/git-panel-controls` created from `main` (`08d100d`).
- 2026-10-12 — G1–G8 implemented by one bounded writer on
  `feat/git-panel-controls` (12 files). Structural readback by the parent.
- 2026-10-12 — Work-unit commit `cc3ded0` created on the feature branch.
- 2026-10-12 — RDD assess on the work unit: `review_due=true`
  (`slice_budget_reached`), risk `medium` (13 files / 795 lines). Preflight
  returned a typed `gentle-ai.review-integration.consent/v3` envelope; this
  runtime has no native `question` tool, so the consent lifecycle STOPPED with
  nothing consumed (lineage `review-715321b26216dc6b` stays reviewable). The user
  chose to deliver without the receipt; delivery follows ordinary repository
  policy.

## Evidence

- `git diff --stat` at the work unit: 12 files changed, 577 insertions(+),
  36 deletions(-), plus this untracked doc.
- `cargo test -p orquestador-core --lib` → 100 passed, 0 failed (parent spot
  check; matches the writer's report).
- `cd frontend && npx tsc --noEmit` → exit 0.
- `cd frontend && npx vite build` → exit 0 (only the pre-existing >500 kB chunk
  warning).
- Work-unit commit: `cc3ded0` `feat(ui): add resizable git split and a branch
  switcher`.

## Next step

- Merge `feat/git-panel-controls` into `main` (`--no-ff`), record this doc, push,
  and delete the feature branch. Manual GUI verification is the user's (rebuild /
  restart the app to see the resizable divider and the branch switcher).
