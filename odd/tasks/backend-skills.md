# Backend-aware skills (header panel) — feature document

- **Feature**: `backend-skills`
- **Directory**: `/home/sanchiro/projects/orquestador` (git)
- **Created**: 2026-10-10
- **Delivery strategy**: `ask-on-risk` (default). Work-unit commits on a feature branch.
- **Delivery forecast**: moderate (core type + shell IO + frontend render), under the ~400-line planning heuristic.
- **Depends on**: `skills-header` (shipped), `AgentBackend` trait (`feat/agent-backend`).

## Objective

Make the header Skills panel **backend-aware**: the app must know which agent backend it is
connected to (`AgentBackend::id()`) and list **that backend's native skills** plus the app's own
skills, so selecting a skill actually injects its content on every prompt.

## Problem

The panel lists **only** app-owned skills at `<app_config_dir>/skills/<name>.md`
(`~/.config/com.orquestador.app/skills/`), which contains a single file (`prueba.md`). Meanwhile the
persisted selection (`localStorage` key `orquestador.skills`) held 22 names that are the user's
**OpenCode** skills (`~/.config/opencode/skills` + `~/.agents/skills`). Result: the button showed
"Skills (22)" while the list showed one item, and `apply_skills` silently skipped all 22 (their files
are not in the app store), so **nothing was injected**. The mismatch is a real bug plus a missing
capability: the app never read the connected backend's skills.

## Verified facts (2026-10-10)

- `AgentBackend` (`crates/orquestador-core/src/backend.rs`) exposes `fn id() -> &'static str`;
  `BACKEND_OPENCODE = "opencode"`. `AppBridge::client() -> Arc<dyn AgentBackend>` is the active
  backend; only `OpenCodeClient` implements it today, so `id()` is always `"opencode"`.
- Native skills live at `~/.<backend>/skills/<name>/SKILL.md` (a **directory** per skill). Observed on
  this machine: `~/.config/opencode/skills` (18 dirs) + `~/.agents/skills` (12, shared convention);
  `~/.claude/skills` and `~/.codex/skills` exist but are empty.
- App skills are single files `<app_config_dir>/skills/<slug>.md` (source of truth for name = file stem).
- Shell IO today: `skills_dir`, `apply_skills`, `list_skills`, `read_skill`, `save_skill`,
  `delete_skill` in `src-tauri/src/commands.rs` (~line 395+). Commands are registered in
  `src-tauri/src/lib.rs:116-119`.
- Frontend: `SkillInfo { name, path }` (`frontend/src/types.ts:192`), selection store
  (`frontend/src/skills.ts`, names in `orquestador.skills`), `TopBar.tsx` badge uses
  `selected.length` (line 198) and lists `listSkills()`.

## Design (settled)

- **Backend id is the source of truth** for which native skills to show. Read it from
  `app.state::<AppBridge>().client().id()`; no new config field, no Settings selector. (Claude Code /
  Codex become reachable when their `AgentBackend` impls land — Track 3 — and the roots map below
  already covers them.)
- **Native skill roots by backend id** (home via Tauri `app.path().home_dir()`):
  - `opencode` → `[~/.config/opencode/skills, ~/.agents/skills]`
  - `claude` → `[~/.claude/skills]`
  - `codex` → `[~/.codex/skills]`
  - unknown → `[]`
- **Skill shape gains provenance**: `SkillInfo { name, path, source, editable }` where `source` is the
  backend id (`"app"` for app-owned) and `editable = (source == "app")`. Native skills are **read-only**
  (the app must never mutate another tool's config).
- **Union + dedup**: list = app skills **then** each native root in order; dedup **by name**, first
  occurrence wins (app shadows native; earlier root shadows later). Sorted by name.
- **Selection stays name-based** (`orquestador.skills` unchanged): a name resolves app-first, then each
  native root (`<root>/<name>/SKILL.md`, or `<root>/<name>.md` for file-shaped roots). This keeps the
  existing store format and immediately makes the 22 OpenCode names resolve and inject.
- **Read layout**: a native skill is `<root>/<name>/SKILL.md` (dir) or, defensively, `<root>/<name>.md`
  (file). Reading a missing skill skips it.
- **Badge fix**: the button count reflects only **available** selections
  (`selected ∩ list`), so it can never over-count again.
- `save_skill` / `delete_skill` keep writing/removing the app store only (unchanged behavior).

## Tasks

- [x] **T1 — Core: `SkillInfo` provenance + pure merge helper** (`crates/orquestador-core/src/skills.rs`):
  add `source: String` and `editable: bool` to `SkillInfo`; add a pure
  `merge_skills(app: Vec<SkillInfo>, native: Vec<SkillInfo>) -> Vec<SkillInfo>` that concatenates and
  dedups by `name` (first wins) then sorts by name. Unit tests: dedup keeps app over native, preserves
  order for distinct names, sorts output, and `editable` survives. Route: delegated writer. Verification:
  `cargo test -p orquestador-core skills`. **Evidence: commit `178ebba`; RED = 6 compile errors
  (E0560 missing `source`/`editable`, E0425 `merge_skills` not found) → GREEN = 8 passed, 0 failed
  (4 new merge tests + 4 existing).**
- [x] **T2 — Shell: native roots + backend-aware list/read/inject** (`src-tauri/src/commands.rs`):
  add `native_skill_roots(app, backend_id) -> Vec<PathBuf>`; extend `list_skills`, `read_skill`,
  `apply_skills` to merge app + native per the design; thread the backend id from
  `app.state::<AppBridge>().client().id()`. Keep `save_skill`/`delete_skill` app-only. Route: same
  delegated writer. Verification: `cargo check --manifest-path src-tauri/Cargo.toml`, `cargo test --workspace`.
  **Evidence: commit `178ebba`; `cargo check --manifest-path src-tauri/Cargo.toml` clean (no warnings);
  `cargo test --workspace` → 77 core passed, 0 failed.**
- [x] **T3 — Frontend: source-aware list + badge fix** (`frontend/src/types.ts`,
  `frontend/src/skills.ts`, `frontend/src/components/TopBar.tsx`): `SkillInfo` gains `source`/`editable`;
  the popover shows a small source tag per skill and marks native (non-editable) skills read-only; the
  badge counts only `selected ∩ available`. Route: same writer. Verification: `cd frontend && npx tsc --noEmit`.
  **Evidence: commit `178ebba`; `npx tsc --noEmit` exit 0 (no output). `skills.ts` unchanged (no helper
  needed). Source tag uses inline styles because `styles.css` was out of the worker's surfaces.**
- [x] **T4 — Docs + feature finalization**: update `docs/ARCHITECTURE.md` (skills store + backend-aware
  sources + injection) and check off this doc with evidence. Route: parent inline. **Evidence: this commit.**

## Authorized scope

Only the files in each task's route plus this document. No changes to `send_command`, no new Tauri
plugins, no writes outside `crates/`, `src-tauri/`, `frontend/`, `docs/`, `odd/`.

## Acceptance criteria

- With backend `opencode`, the Skills panel lists the union of `~/.config/opencode/skills`,
  `~/.agents/skills` and app skills, deduped by name, each tagged with its source.
- The 22 previously-orphaned names now appear (checked) and their content is injected into
  `PromptBody.system` on every send.
- The badge count equals the number of selected skills that actually exist.
- Native skills are read-only; app skills remain editable/importable.
- `cargo test --workspace`, `cargo check --manifest-path src-tauri/Cargo.toml`, and
  `cd frontend && npx tsc --noEmit` all pass.

## Verification commands

- `cargo test --workspace`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cd frontend && npx tsc --noEmit`
- Manual (user): restart the app, open Skills, confirm the OpenCode skills show and a message carries them.

## RDD status

- `gentle-ai review assess --base-ref 41841e5 --committed-only` → risk **medium** (`executable_change`),
  `review_due=true` (`slice_budget_reached`, 443 changed lines). Preflight STATUS returns a ready
  `review.start` (`--consent=relay`, lineage `review-f457c93b051a9671`).
- **Deferred to delivery by the user** — the review transaction was not started.
