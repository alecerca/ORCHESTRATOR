# Orquestador — Diff panel (working-tree changes, live)

## Objective

Ship the diff feature that is documented but missing from the UI: a third
`EditorPanel` rail tab next to **Files** and **Git** that shows working-tree
changes **live** and opens a side-by-side diff of any changed file.

## Problem (evidence)

- No `git status` / `git diff` anywhere: `crates/orquestador-core/src/git.rs`
  has only `branch_for_dir`, `overview`, `commit_files`, `branches`, `checkout`
  (verified: git.rs L19-201); `src-tauri/src/commands.rs` exposes `git_branch`,
  `git_log`, `git_commit_files`, `git_branches`, `git_checkout`
  (commands.rs:803-878), all routing through `active_directory(&app)`
  (commands.rs:818, reads persisted `x-opencode-directory` from `server.json`).
- `monaco` already supports `DiffEditor` (`@monaco-editor/react`); none of the
  components use it. `files`/`git` are the only rail tabs (`EditorPanel.tsx`).
- The `diff?` field (`frontend/src/types.ts:317`) is a message field, not git.
- **`file_status` exists but is irrelevant here**: `backend.rs:118` /
  `client.rs:538` / `commands.rs:451` proxy the **opencode server's**
  workspace file status (`FileStatus { path, added, removed, status }`,
  core `types.rs:773`). That is not the local git worktree — the diff panel
  uses **local git** (`status`/`show` against the active directory), consistent
  with `GitViewer`.
- The bug the user fears ("never worked") does not exist to fix: the feature
  was never implemented. We build it from scratch.

## Scope

- **Core** (`crates/orquestador-core/src/git.rs`): `status(dir)` → `Vec<Change>`
  (modified/added/deleted/untracked; rename → modified, new path) parsed from
  `git status --porcelain -z` with `-c core.quotepath=false`; `show(dir, path)`
  → `HeadFile { kind: text|binary|missing, content }` from `git show HEAD:<path>`
  (missing = left side empty). Unit tests in-repo (`TempDir` helper, git.rs:280).
- **Shell** (`src-tauri/src/commands.rs`): `git_status(app)` →
  `Vec<orquestador_core::git::Change>`, `git_show_file(app, path)` →
  `HeadFile`; both via `active_directory(&app)?.` + `spawn_blocking` (same
  pattern as `git_log` commands.rs:832), registered in `src-tauri/src/lib.rs`
  after `git_checkout` (lib.rs:129). Path guard: reject absolute paths and `..`.
- **Frontend**:
  - `types.ts`: `GitChangeKind = "modified"|"added"|"deleted"|"untracked"`,
    `GitChange { path, kind }`, `HeadFile { kind: "text"|"binary"|"missing",
    content: string | null }`; `commands.ts`: `gitStatus()`, `gitShowFile(path)`.
  - `tabs.ts`: new `DiffTab { id, kind: "diff", path }` in the `Tab` union;
    `diffTabKey(path)` (`diff:<path>`), `openDiffTab(layout, path)`; switch-by-
    kind points (`tabTitle`, `tabPath`, restore) updated. Diff tabs restore from
    persisted layout like file tabs (deterministic id, content re-read).
  - `EditorPanel`: third rail tab **Diff** (`files | git | diff`); new prop
    `onOpenDiffFile(path)`; renders `DiffPanel`.
  - `DiffPanel.tsx` (new): header = `.gitviewer-style` branch menu + poll every
    3 s of `gitStatus()` + `gitLog()` (branch auto-follows the orchestrator);
    list of changes with M/A/D/U badge + path, click → `onOpenDiffFile(path)`;
    states: clean / loading / no project.
  - `BranchSwitcher.tsx` (new): extracted from `GitViewer.tsx` (branch dropdown
    + `onSelect`), re-imported by GitViewer; DiffPanel reuses it with a
    `gitCheckout` + `invalidateGitBranches()` + immediate re-poll handler.
  - `DiffViewer.tsx` (new): Monaco `DiffEditor`, original = `gitShowFile`
    (text→content, binary→placeholder, missing→""), modified = existing
    `readFile`-based text loading; `languageForPath(path)` (utils.ts:60);
    `theme` prop; polluted modified side every 3 s while mounted.
  - `PaneGrid.tsx` GroupView: render branch for `kind === "diff"` →
    `<DiffViewer key path theme />` (slot: not session, not file).
  - `TabStrip.tsx`: `DiffIcon` for kind `diff` in the icon ternary (TabStrip:127).
  - `icons.tsx`: new `DiffIcon` (SkillIcon pattern, icons.tsx:233).
  - `App.tsx`: `onOpenDiffFile={(path) => setLayout((c) => openDiffTab(c, path))}`
    beside `onOpenFile` (App.tsx:680); import `openDiffTab`.
- **CSS** in `styles.css` following the existing `editor-panel`/`gitviewer`
  visual language (`.diff-panel`, `.diff-row`, `.diff-badge--<kind>`).

## Out of scope (this slice)

- Staging area controls (stage/unstage) — list only, VSCode-style, read-only.
- Per-commit diff browsing (GitViewer already covers commit files).
- Prompt on app quit / close-all with dirty file tabs (single-tab close only).
- Restoring an unsaved draft after the app restarts (drafts are in-memory;
  a reopened file tab re-reads from disk).

## Design decisions

1. **Time-realism via polling** (`git status --porcelain -z` every ~3 s +
   refresh on checkout). Simple, robust, no event plumbing; the opencode SSE
   hub does not carry file-change events.
2. **Untracked files**: shown as `untracked` (U badge, like VSCode SCM);
   left side = empty, right = content. Renames collapse to `modified` with the
   destination path.
3. **Deleted files**: right side = empty; left = HEAD content.
4. **Binary files**: `HeadFile.kind = "binary"` → placeholder, no DiffEditor.
5. **Diff tab is a document kind** in the existing layout tree (`Tab` union +
   `GroupView` render switch), so it integrates with groups/splits/persistence
   exactly like file tabs. `tabPath()` stays file-only → GroupView's file slot
   stays empty for diff kinds and a dedicated branch renders `DiffViewer`.
6. Branch auto-follow is just the poll: `gitLog().branch` re-read every tick,
   no `useGitBranch` cache (that hook caches once per directory and is for
   session chips — the panel needs freshness).
7. Shell commands follow the exact `git_log` pattern (commands.rs:832):
   `active_directory(&app)?` + `spawn_blocking` + `map_err(stringify)`; no
   proxy/bridge involvement.

## Tasks

- [ ] **D1 — Core git status/show.** `status()` + `show()` in `git.rs` with
  in-repo tests (`TempDir` helper, git.rs:280): modified, added, untracked,
  deleted, non-UTF8 → binary, rename → modified destination, show missing →
  `missing`, outside-repo errors.
- [ ] **D2 — Tauri commands.** `git_status`, `git_show_file` via
  `active_directory` + `spawn_blocking` (git_log pattern, commands.rs:832);
  path guard (no absolute, no `..`); register in `src-tauri/src/lib.rs`.
- [ ] **D3 — Diff rail tab + status list.** `EditorPanel` third tab +
  `onOpenDiffFile` prop; `DiffPanel` (branch menu + polled M/A/D/U list);
  `BranchSwitcher` extracted from GitViewer and reused; `types.ts`/`commands.ts`
  additions; CSS.
- [ ] **D4 — Diff document tab.** `DiffTab` kind; `tabs.ts` helpers; `App.tsx`
  wiring; `GroupView` render branch; `DiffViewer` (Monaco `DiffEditor`,
  HEAD vs working, live poll); `TabStrip` `DiffIcon`; layout persistence.
- [ ] **D5 — Verification.** Core tests green; `tsc --noEmit` + `npm run
  build`; `cd src-tauri && cargo check`; RDD review on the work-unit commits.
- [ ] **D6 — Save files (Ctrl+S) + close prompt for dirty tabs.** User request
  (2026-10-13, "una ultima cosa antes de comitear y pushear a main"):
  - Core `files.rs`: `write_text_file(dir, rel_path, content)` — same relative
    path guard as `git_show_file`, `fs::write`, in-repo tests.
  - Shell `write_file` command (active_directory + spawn_blocking), registered.
  - Frontend `fileDrafts.ts`: clone of `skillDrafts.ts` keyed by file path
    (baseline-based dirtiness, in-memory) — the skill close-prompt pattern
    applied to file tabs.
  - `MonacoEditor` `onSave` prop → `editor.addCommand(KeyMod.CtrlCmd |
    KeyCode.KeyS, onSave)`; `FileViewer` wires save → `writeFile` invoke,
    updates baseline, shows errors via `describeError`.
  - `App.tsx` `requestCloseTab`: file tab + dirty → modal (Save / Don't save /
    Cancel), mirroring `pendingSkillClose` (save → write → close; discard →
    drop; cancel → keep). Esc closes = cancel.
  - `TabStrip`: dirty dot on file tabs while unsaved (VSCode-style bullet).

## Checks (per task)

- `cargo test -p orquestador-core`
- `cd frontend && npx tsc --noEmit && npm run build`
- `cd src-tauri && cargo check` (shell is not a workspace member; check only)
- `bash scripts/verify-headless.sh`

## Delivery

- Branch `feat/diff-panel`. Forecast: 400–700 authored lines (frontend-heavy);
  if the cohesive slice exceeds the 400-line review budget, recommend
  `size:exception` rather than an artificial split (same rationale as the
  visual identity skin).
- User explicitly authorized: implement then audit then **commit, push and
  merge to main**.

## Progress log

- [x] Feature document created (2026-10-13) on `main`.
- [x] D1..D5 implemented and reviewed by the user in the running app (dummy
  M/D/U files verified working).
- [x] **User-reported fix (accepted)**: the webview's overlay horizontal
  scrollbar on `.tabstrip` expanded on hover and swallowed the strip (blocking
  the close buttons). Replaced with hidden native scrollbar + `‹ ›` arrow
  buttons shown only while the strip overflows (`TabStrip.tsx` + `styles.css`).
- [ ] D1..D5 formal verification, RDD assessment on the work-unit commits
  (assessment ran: medium/slice_budget_reached → candidate review declined by
  the user, `declined_this_candidate`), push + merge to `main` (authorized,
  pending final OK).
- [ ] `cargo build` note: the shell does not recompile when
  `frontend/dist` changes (no `rerun-if-changed`); after a frontend build,
  force with `cargo clean -p orquestador && cargo build`.
- [ ] D6 (save + close prompt) implemented, verified, binary rebuilt; then
  push + merge to `main` (authorized).