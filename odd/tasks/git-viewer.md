# Git viewer — feature document

**Status: delivered** (2026-10-12) — see *Evidence* and *RDD status* below.

## Objective
Replace the broken **Diff** tab in the right-hand panel with a **Git** tab that
browses the repository like VS Code's Source Control view: the **commit history
of the target branch** and, for the selected commit, **the names of the files
that changed** (with their status letter). Read-only throughout.

## Problem
The current `Diff` tab (a `MonacoEditor` side-by-side diff fed by
`GET /session/:id/diff` → `FileDiff[]`) does not work for the user. The
per-message session diff is not a useful or reliable view; a repository-level git
history is what they actually want.

## Why now
The app now has a git module (`crates/orquestador-core/src/git.rs`) and a
per-session branch indicator. Extending that read-only git surface to history +
changed files is the natural next step and reuses the same "derive locally with
git, never mutate" approach.

## Verified facts (2026-10-11)
- The panel is `EditorPanel` (`frontend/src/components/EditorPanel.tsx`): tabs
  `Files` (`FileTree`) and `Diff` (`DiffViewer`). Single call site in
  `frontend/src/App.tsx:582`.
- `DiffViewer` is used only by `EditorPanel`; `getDiff` only by `commands.ts`;
  the frontend `FileDiff` type only by `commands.ts`. Nothing else consumes them.
- The Rust `diff()` (`AgentBackend`/`OpenCodeClient`, `FileDiff` in core) has its
  own tests and is part of the OpenCode API surface — **leave it untouched**;
  only the thin shell command `get_diff` is retired.
- The **local project directory** is available to the shell without touching the
  `AgentBackend` trait: `app.path().app_config_dir()` →
  `server_config::effective_file(&dir)` → `to_client_config(&file).directory`
  (exactly how `set_project_directory` persists it). `None` = no project picked.
- Command shapes verified live on this repo:
  - `git -C <dir> log --max-count=N --date=iso-strict --pretty=format:%H%x1f%h%x1f%an%x1f%ad%x1f%s%x1e`
    → one record per commit, fields split on `\x1f`, records on `\x1e`.
  - `git -c core.quotepath=false -C <dir> show --name-status --format= --find-renames --first-parent <hash>`
    → works for normal **and merge** commits (merge commits otherwise print no
    name-status); `-c core.quotepath=false` keeps non-ASCII paths unescaped.
  - A non-repo dir and an unborn HEAD both exit non-zero → surface a message.
- `tsconfig` sets `noUnusedLocals` + `noUnusedParameters`; `EditorPanel`'s
  `theme`/`rootSessionId` props become unused once `DiffViewer` goes.

## Design (settled)
1. **Core** — extend `crates/orquestador-core/src/git.rs` (read-only, TDD):
   - `#[derive(Serialize)] Commit { hash, short, subject, author, date }`
   - `#[derive(Serialize)] CommitFile { path, status }` (status = the single
     letter from `--name-status`: `A`/`M`/`D`/`R`/`C`/`T`…)
   - `#[derive(Serialize)] RepoLog { branch: Option<String>, commits: Vec<Commit> }`
   - `pub fn overview(dir: &Path, limit: usize) -> Result<RepoLog, String>`:
     branch via the existing `branch_for_dir` + the `log` command above.
   - `pub fn commit_files(dir: &Path, hash: &str) -> Result<Vec<CommitFile>, String>`.
   - Errors are friendly strings (no commits yet, not a repository, git missing).
2. **Shell** — `src-tauri/src/commands.rs`:
   - `fn active_directory(app: &AppHandle) -> Result<PathBuf, String>` (see
     verified facts; `None` → a clear "no project directory selected" error).
   - `#[tauri::command] pub async fn git_log(app, limit: Option<usize>) -> Result<RepoLog, String>`
     and `git_commit_files(app, hash: String) -> Result<Vec<CommitFile>, String>`
     — `spawn_blocking` around the core calls.
   - Register both in `lib.rs`; **remove** `get_diff` and its registration and the
     now-unused `FileDiff` import.
3. **Frontend**:
   - `types.ts`: remove `FileDiff`; add `GitCommit`, `GitCommitFile`, `GitLog`
     (`branch: string | null`).
   - `commands.ts`: remove `getDiff` (+ its `FileDiff` import); add
     `gitLog(limit?)` and `gitCommitFiles(hash)`.
   - Delete `frontend/src/components/DiffViewer.tsx`.
   - New `frontend/src/components/GitViewer.tsx` (no props): loads `gitLog()` on
     mount, auto-selects the first commit, renders the selected commit's files;
     a **Refresh** button re-fetches (agents commit while you watch). Header shows
     the branch (`HEAD` → `detached`). Renders explicit loading / error /
     "no commits" / "no directory" states. Rows reuse the existing `.tnode` look.
   - `EditorPanel.tsx`: tabs become `"files" | "git"` (`Files` / `Git`), render
     `<GitViewer />`, and **drop the now-unused `theme`/`rootSessionId` props**.
   - `App.tsx`: drop those two props from the `<EditorPanel>` call site.
   - `styles.css`: add the `.gitviewer*` classes; remove the dead `.diffviewer*`
     rules and rename `.editor-panel__diff` → `.editor-panel__git` (grep each
     class name first to prove nothing else uses it).
4. **Branch scope (v0)**: the history shown is the **current branch (HEAD)** of
   the active project directory, displayed in the header. A branch/tag picker is
   an explicit follow-up, not part of this slice.

## Tasks
- [x] **T1 — Core `git.rs`**: `Commit`/`CommitFile`/`RepoLog`, `overview()`,
      `commit_files()` + 5 TDD tests (RED: 3 failing on stubs → GREEN: 94 passed;
      also `parse_log`/`friendly_git_error` extracted in a refactor pass).
      Runner: `cargo test -p orquestador-core --lib`.
- [x] **T2 — Shell**: `active_directory` + `git_log` + `git_commit_files`,
      both registered in `lib.rs`, `get_diff` command + its registration + its
      `FileDiff` import removed. `cargo check --manifest-path
      src-tauri/Cargo.toml --tests` clean.
- [x] **T3 — Frontend**: types/bindings, delete `DiffViewer`, add `GitViewer`,
      retab `EditorPanel`, drop `theme`/`rootSessionId` in `EditorPanel`+`App`,
      CSS renamed/added. `npx tsc --noEmit` clean, `npx vite build` OK.
- [x] **T4 — Verification + bookkeeping**: full suite green, spot check, evidence
      + commit ids recorded below, RDD assess run.

## Evidence (2026-10-12)
- Work-unit commit `382357e` `feat(ui): add a read-only git history panel`
  (10 files, +562/−142) on `feat/git-viewer`; merged with `--no-ff` as `396c014`.
- Verification (delegated writer, all green): `cargo test -p orquestador-core
  --lib` → 94 passed; `cargo test --workspace` → OK (core 94, proxy 30 + 5);
  `cargo check --manifest-path src-tauri/Cargo.toml --tests` and without
  `--tests` → clean; `npx tsc --noEmit` → clean; `npx vite build` → OK (only the
  known >500 kB chunk warning).
- Parent spot check: re-ran `cargo test -p orquestador-core --lib` → 94 passed,
  0 failed; `git diff --stat` matched the writer's report; grep for
  `DiffViewer|getDiff|FileDiff|diffviewer|editor-panel__diff` under `frontend/src`
  → no matches (nothing dead left).

## Known limits (v0)
- The serde `\x1e`/`\x1f` record parse would misparse a subject containing those
  control characters (practically impossible in a commit subject).
- A failed Refresh clears the previously shown log and surfaces the error
  (no "keep last good list").
- GUI behaviour (the Git tab listing this repo's commits + each commit's files)
  is verified only at the type/compile/test layer; the in-app manual check is
  still the user's step.

## Authorized scope
Exactly the files above (plus this document and its Engram mirror). No new
dependencies. Read-only git only (`log`, `show`, `rev-parse`). No mutating git
command, no commit/push from the UI. Rust core `diff()` stays untouched.

## Acceptance criteria
- The right panel has two tabs: **Files** and **Git** (no `Diff`).
- **Git** shows the current branch and its commits (newest first: short hash,
  subject, author, date); selecting a commit lists the changed file names with a
  status letter; merge commits list their files too.
- Clear states for: loading, no project directory selected, not a git repo / no
  commits, and a load error. A Refresh button re-reads the history.
- No UI action runs a mutating git command. Removing `Diff` leaves nothing unused
  (no dead `DiffViewer`/`getDiff`/`FileDiff`, no dead CSS referencing them).
- Full suite + `tsc` + `vite build` green.

## Verification commands
- `cargo test --workspace` (core lib incl. new git tests; proxy 30 + 5)
- `cargo check --manifest-path src-tauri/Cargo.toml --tests && cargo check --manifest-path src-tauri/Cargo.toml`
- `cd frontend && npx tsc --noEmit`
- `cd frontend && npx vite build` (only the known >500 kB warning)
- Manual (user): with a project directory picked, the **Git** tab lists this
  repo's commits and each commit's changed files.

## RDD status
- `gentle-ai review assess --base-ref ddd2e37 --committed-only` → **medium**
  (`executable_change`, `crates/orquestador-core/src/git.rs`),
  `review_due: true` (`slice_budget_reached`, 826 changed lines incl. this doc).
- Preflight (with `--intended-untracked=odd/tasks/git-viewer.md`) returned
  `review.start`; START returned the typed
  `gentle-ai.review-integration.consent/v3` envelope (medium). This runtime has
  **no native `question` tool**, so the consent prompt cannot be rendered and the
  contract forbids a chat-token fallback → the review lifecycle stopped there;
  **neither `granted` nor `declined` was consumed**. The code candidate stays
  reviewable (lineage `review-12b92672ab0e9b75`, target `sha256:300a197f…`).
- A separate passive candidate (this doc, current-changes projection)
  auto-approved as **low** / 0 lenses and was acknowledged
  (`authority: burned`, lineage `review-86671b829ec66831`).