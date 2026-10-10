# Panel dismiss + session picker — feature document

- **Feature**: `panel-dismiss-picker`
- **Directory**: `/home/sanchiro/projects/orquestador` (git, `main`, no remote)
- **Created**: 2026-10-10
- **Delivery strategy**: `single-branch` (repo policy from `orquestador-dashboard.md`: work-unit commits land on `main`; no remote → no PRs)
- **Forecast**: ~200 authored changed lines (frontend-only) → under the 400-line slice budget; no chain expected.

## Objective

1. Let the user **dismiss (hide) finished sub-agent rows** from the side panel to keep it clean. The **orchestrator (root) row can never be dismissed** — structural guarantee, not a disabled button.
2. Add a **session picker** so the user can choose which session acts as the orchestrator root, fixing the documented wrong-session bug (`pick_root` auto-selects the newest parentless session; `odd/tasks/orquestador-dashboard.md:80`).
3. Housekeeping: mark stale T1–T3 checkboxes done in `odd/tasks/orquestador-dashboard.md`; initialize the CodeGraph index (user-authorized).

## Constraints & design decisions

- **Hide only, never delete**: `delete_session` / `DELETE /session/:id` stays unused. Dismiss is a UI-local hidden set; the session remains alive on the server (user decision 2026-10-10).
- **Root protection**: `AgentPanel` renders the root row from a branch that receives no dismiss prop → the orchestrator cannot be hidden even by state accident.
- **Dismiss side effects**: dismissing a row also closes its pane (`closePane`) and drops its stream (`dropStream`) — no orphan panes.
- **Picker = frontend-local effective root override**: `src-tauri` cannot compile on this machine (missing `webkit2gtk-4.1`), so a new Rust `set_root_session` command would be unverifiable. Design: picker selects among parentless sessions; the choice persists in `localStorage` (existing pattern, `App.tsx:18,46-53`); on snapshot, if the stored id still exists as a parentless session it becomes the effective root, otherwise fall back to `snapshot.root_session_id`. All root-dependent UI (row filter, composer target, pane bootstrap) uses the effective root. Documented limitation: core `AppState.root_session_id` itself is unchanged.
- **Finished** = `AgentUiStatus` ∈ {`done`, `idle`, `error`}; rows with `running`/`waiting_approval` keep only their existing abort action (dismiss offered after finish).

## Tasks

- [x] **D1 — Dismiss sub-agent rows**: hidden set in `App.tsx` (localStorage `orquestador.dismissed`), `dismiss(id)` no-ops on the effective root, `closePane` + `store.dropStream`, auto-undismiss on `running`/`waiting_approval`; filter in `AgentPanel.tsx`; `onDismiss` only on the sub-agent row branch. Route: delegated writer (writer rule: 3 non-trivial files). Trigger evidence: 2+ non-trivial files.
- [x] **D2 — Session picker**: `resolveEffectiveRoot` + persisted pick (`orquestador.rootPick`) + `<select>` in the panel header; all root call sites routed (`AgentPanel`, `App` bootstrap/composer, `TopBar`, `EditorPanel`); `dashboard.md:80` reconciled. Route: delegated writer — same writer/session as D1 (no parallel writers).
- [x] **D3 — Stale checkboxes**: T1–T3 marked `[x]` in `odd/tasks/orquestador-dashboard.md:42-44` (evidence: commits `046b0c0`, `b662e94`, `2b412ff`, progress log `:70-72`). Route: inline (mechanical, 1 file).
- [x] **D4 — CodeGraph index**: `gentle-ai codegraph init --cwd <repo>` → `CodeGraph initialized`. `.codegraph/` added to `.gitignore` (machine-local derived index must not be committed). Route: inline bash.

## Writer derivation (recorded, accepted by parent)

The writer reported edits to `TopBar.tsx` and `EditorPanel.tsx`, **outside** the authorized surfaces. Both are 17 lines total and necessary for the effective-root to be coherent (top-bar root label; `DiffViewer` diff target). Parent reviewed both diffs and accepted them as in-scope derivations; the authorization miss is noted here for observability. Not repeated.

## Verification

- No frontend test runner exists (by design, `SPEC.md:85`) → **test-first exception**: deterministic RED not runnable; proportional proof = `npm run typecheck` + `npm run build` (`tsc --noEmit && vite build`) + parent spot-check of one reported command.
- Parent spot check: re-run `npx tsc --noEmit` before delivery.
- RDD gate: read `gentle-ai review mode status`; after the work-unit commit run `gentle-ai review assess --cwd <repo> --committed-only` and follow the tier.

## Acceptance criteria

1. Dismissing a finished sub-agent row removes it from the panel, closes its pane, drops its stream; server session untouched.
2. The root/orchestrator row has no dismiss affordance under any state.
3. Picker lists parentless sessions; chosen root persists across reload; falls back to snapshot root if that session disappears.
4. `npm run typecheck` and `npm run build` green; `cargo test -p orquestador-core` unaffected (no core changes) but re-run as regression.
5. `dashboard.md` T1–T3 checked; picker gap line reconciled.
6. Conventional work-unit commit(s) on `main`, no AI attribution.

## Progress log

- [x] Feature document created (parent, 2026-10-10) — before first source write.
- [x] D1+D2 implemented (delegated writer, 2026-10-10) — 7 frontend files; `npx tsc --noEmit` clean; `npm run build` OK (only pre-existing >500 kB chunk warning).
- [x] Parent spot check: `cd frontend && npx tsc --noEmit` → exit 0, clean.
- [x] D3 + D4 (parent, inline): checkboxes reconciled; CodeGraph initialized; `.codegraph/` gitignored.
- Note: no frontend test runner (by design) → test-first exception documented above; proof is typecheck + build.
