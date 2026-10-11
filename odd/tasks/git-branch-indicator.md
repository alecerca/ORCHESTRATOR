# Git branch indicator — feature document

## Objective
Show, for every agent and sub-agent card (and the workspace tab bar), **which git
branch it is working in**, e.g. `working in main`. Read-only information only —
no git interaction from the UI, by explicit user decision ("SOLO POR AHORA
brindar info").

## Problem
The user cannot tell which branch an agent/sub-agent is working on without
leaving the app. Sessions may live in different directories/worktrees, so the
branch can differ per agent.

## Why now
The project-switching slice made `session.directory` meaningful (a session can
be scoped to any project directory), so per-session branch info is now
actionable.

## Verified facts (2026-10-11)
- Every `Session` carries `directory: String` (core `types.rs:78`, frontend
  `types.ts:25`). Sub-agents can be in different worktrees → per-session
  directory is the right granularity.
- **opencode does NOT expose the branch**: probed `GET /project/current`
  (returns only `id/worktree/time`) and `GET /config` — no vcs/branch field. So
  the branch must be derived locally with `git`.
- The running-time + Abort UI lives in two places: `AgentCard` (left panel) and
  `GroupInfoBar` (bar above the tab strip of an `AgentPane`). Put the branch in
  BOTH — one shared `BranchChip` component.
- `orquestador-core` currently has no `std::process`/`std::fs` usage; running
  git is pure std, so it belongs in core as a testable unit. No new
  dependencies allowed (standing rule).

## Design (settled)
1. **Core** — `crates/orquestador-core/src/git.rs`:
   - `pub fn branch_for_dir(dir: &Path) -> Option<String>`: runs
     `git -C <dir> rev-parse --abbrev-ref HEAD` via `std::process::Command`,
     returns `None` when git fails / the dir is not inside a repo, else the
     trimmed branch name. `HEAD` (detached) is returned as-is; presentation
     maps it.
   - Unit tests: named branch (`git init -b main` in a `t.TempDir()`-style
     temp dir), non-repo dir → `None`, detached HEAD → `"HEAD"`.
     TDD: stub with `unimplemented!()` → RED → implement → GREEN.
2. **Shell** — `src-tauri/src/commands.rs`:
   - `#[tauri::command] pub async fn git_branch(directory: String)
     -> Option<String>`: empty dir → `None`; otherwise
     `tauri::async_runtime::spawn_blocking` over `branch_for_dir`. Plain
     return, no `Result` (nothing the UI must surface as an error).
   - Register in `src-tauri/src/lib.rs` (invoke_handler).
3. **Frontend**:
   - `frontend/src/commands.ts`: `gitBranch(directory): Promise<string | null>`
     → `invoke("git_branch", { directory })`.
   - `frontend/src/components/useGitBranch.ts` (new): module-level cache keyed
     by directory + fetch on mount/`directory` change. Known v0 limitation:
     branch changes outside the app are not re-polled (info-only).
   - `frontend/src/components/BranchChip.tsx` (new): renders
     `working in <strong>{branch}</strong>` when a branch exists, nothing when
     `null`/empty, and maps `"HEAD"` → `detached`.
   - `AgentCard.tsx`: add `<BranchChip directory={session.directory} />` in the
     meta row (next to the session id).
   - `GroupInfoBar.tsx`: add the same chip when a snapshot is present (active
     tab).
   - `frontend/src/styles.css`: one small class (`.agent-card__branch` shared by
     both call sites; keep the existing visual language).
4. **Verify** — full suite: `cargo test --workspace`,
   `cargo check --manifest-path src-tauri/Cargo.toml` (+ `--tests`),
   `npx tsc --noEmit`, `npx vite build` (pre-existing >500 kB warning only).
   UI has **no test runner** → manual check is the documented exception.

## Tasks
- [x] **T1 — Core `git.rs`**: `branch_for_dir` + tests (named/detached/non-repo).
      RED → GREEN. Runner: `cargo test -p orquestador-core --lib`.
      Evidence: RED = 3 git tests failed with `not implemented` at `git.rs:17`
      (86 passed); GREEN = `89 passed; 0 failed`.
- [x] **T2 — Shell command**: `git_branch` command + registration.
      Evidence: `#[tauri::command] git_branch` (spawn_blocking) registered in
      `lib.rs`; `cargo check --manifest-path src-tauri/Cargo.toml --tests` clean.
- [x] **T3 — Frontend**: `gitBranch` + `useGitBranch` + `BranchChip`, wired into
      `AgentCard` and `GroupInfoBar`, plus styles. Checks: `npx tsc --noEmit`,
      `npx vite build`.
      Evidence: `tsc --noEmit` exit 0; `vite build` `✓ built` exit 0 (only the
      pre-existing >500 kB chunk warning).
- [x] **T4 — Verification + bookkeeping**: full suite green, feature doc entries
      with evidence + commit id, RDD assess.
      Evidence: `cargo test --workspace` = core lib 89 + proxy 30 + 5, 0 failed.

## Progress / evidence
- Feature commit `a5c4e0d` (`feat(ui): show the git branch each agent is working
  in`), merged to `main` as `64d79ea`. 10 files, +251/−0. Spot check by parent:
  `cargo test -p orquestador-core --lib` re-run → 89 passed.
- **Deviation (T1 fixture)**: the named-branch test adds one `git commit
  --allow-empty` before asserting `Some("main")`, because git 2.56.0 exits 128 on
  an *unborn* HEAD. Assertion and implementation semantics are unchanged — only
  the fixture gained a birth commit.
- **Known limitation (v0)**: a repository with no commits renders no chip
  (`branch_for_dir` → `None`, per the "command fails → None" rule). Branch
  changes made outside the app are not re-polled (module cache, info-only).
- **Not yet done**: the manual GUI check (open the app against a running server;
  repo is on `main`, expect `working in main` on the orchestrator card and the
  tab info bar) — pending user, no runner in the worker session.

## Authorized scope
Exactly the files above plus this document and the Engram mirror. No git
interaction commands in the UI (read-only). No new dependencies. Working tree
and remote branches stay untouched by this feature.

## Acceptance criteria
- Each agent card and each open tab's info bar shows `working in <branch>` for
  git repos, `working in detached` on a detached HEAD, and nothing when the
  directory is not inside a git repo.
- Branch comes from that session's own `directory` (sub-agents in different
  worktrees show different branches).
- Read-only: no UI action runs a mutating git command.
- Full suite green; `tsc` + `vite build` green.

## Verification commands
- `cargo test --workspace` (expect core lib 86 + new git tests, proxy 30 + 5)
- `cargo check --manifest-path src-tauri/Cargo.toml` and `--tests`
- `cd frontend && npx tsc --noEmit`
- `cd frontend && npx vite build` (warning-only chunk size)
- Manual: open the app against the running server (repo is on `main`); expect
  `working in main` on the orchestrator card and the tab bar of an open pane.

## RDD status
- `gentle-ai review assess --base-ref ed33374 --committed-only` → risk
  **medium** (`reason: executable_change`, `git.rs`), **`review_due: false`**
  (`review_due_reason: under_budget`, 355 changed lines). No review gate
  pending. (A first run reported `high`/`unassessable` only because this
  untracked doc was undeclared; re-run with `--untracked-scope=select
  --intended-untracked=odd/tasks/git-branch-indicator.md` gave the real tier.)