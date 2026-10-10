# Orquestador — Implementation Plan (task list)

> Extracted from `docs/PLAN.md` §9. `PLAN.md` is the single source of truth; read
> it (and `SPEC.md`) before starting. This file is the ordered checklist for
> **AGENT 2 (IMPLEMENTER)**.

## Ground rules

- No `todo!()` / `unimplemented!()` may remain (`grep -rnE 'todo!\(|unimplemented!\(' crates src-tauri/src`).
- TypeScript strict stays clean (`npm run typecheck`).
- No new state framework; the store is `useSyncExternalStore`.
- Do not edit `SPEC.md`. Do not commit (the orchestrator commits).

## Steps (dependencies in the last column)

| # | Step | Files | Acceptance | Depends |
|---|---|---|---|---|
| S1 | Confirm scaffold builds | — | `bash scripts/verify-headless.sh` | — |
| S2 | Confirm manifest isolation | — | `(cd src-tauri && cargo metadata --format-version 1 >/dev/null)`; root metadata shows 1 member | — |
| S3 | Store + wire types hardening | `frontend/src/store.ts`, `types.ts` | `npm run typecheck` | S1 |
| S4 | Typed command bindings | `frontend/src/commands.ts` (new) | `npm run typecheck` | S3 |
| S5 | Agent panel + cards | `frontend/src/components/AgentPanel.tsx`, `AgentCard.tsx` | `npm run build` | S4 |
| S6 | Composer | `frontend/src/components/Composer.tsx` | `npm run build` | S4 |
| S7 | Agent pane + stream + split grid | `.../AgentPane.tsx`, `MessageStream.tsx`, `PaneGrid.tsx` | `npm run build` | S4,S5 |
| S8 | Approval dialog + notifications drawer | `.../ApprovalDialog.tsx`, `NotificationsDrawer.tsx` | `npm run build` | S4 |
| S9 | File tree + Monaco + workers | `.../FileTree.tsx`, `MonacoEditor.tsx`, `monaco/setup.ts` | `npm run build` | S4 |
| S10 | Diff viewer | `.../DiffViewer.tsx` | `npm run build` | S9 |
| S11 | App wiring + layout + styles | `App.tsx`, `styles.css` | `npm run typecheck && npm run build` | S5–S10 |
| S12 | Verify shell after deps install | `src-tauri/**` | `bash scripts/setup-system-deps.sh` then `cargo build` in `src-tauri` | S2, env |
| S13 | Implementation notes | `docs/IMPLEMENTATION_NOTES.md` | file exists with evidence + deviations | S11 |

## Verification (must all pass, report exact output)

```
bash scripts/verify-headless.sh          # metadata + fmt + check + test + clippy + frontend build + stub audit
```

`src-tauri` compilation and GUI smoke are **environment-blocked** until the user
runs `scripts/setup-system-deps.sh` with sudo (see PLAN §10).

## Deliverable for AGENT 3

`docs/IMPLEMENTATION_NOTES.md` describing: what was built, deviations from this
plan, how to run, and the exact verification output. AGENT 3 then audits, writes
`docs/AUDIT.md`, and finalizes `README.md` + the first-run RUNBOOK.
