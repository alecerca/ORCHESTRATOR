# Orquestador Dashboard — feature document

- **Feature**: `orquestador-dashboard`
- **Directory**: `/home/sanchiro/projects/orquestador` (greenfield; `git init` on `main`, no remote)
- **Created**: 2026-10-10
- **Delivery strategy**: `single-branch` (no PRs — repo has no remote; work-unit commits land on `main`)

## Objective

A desktop app (Tauri) to **see and manage OpenCode subagents from one interface**: a composer to send instructions to the orchestrator session, a subagent panel (task + state), per-subagent panes with live message streams, interrupt/re-instruct actions, permission (approval) responses, a code editor with diff viewing, and desktop notifications when subagents finish, fail, or need approval. Explicitly **no worktree/issue machinery** — Orca-inspired but simpler.

## Fixed stack decisions (user-approved)

- Tauri 2 (Rust backend) + TypeScript frontend + Monaco (editor + built-in diff) + `@xterm/xterm` (terminal panes, stretch goal).
- Transport: OpenCode server HTTP API + SSE event stream (`opencode serve`).

## Hard environment constraint (drives architecture)

`webkit2gtk-4.1` and `libayatana-appindicator` are NOT installed and `sudo` requires a password → the Tauri app crate **cannot compile on this machine yet**.

- **Mandatory structure**: Cargo workspace with the bulk of the logic in `crates/orquestador-core` — pure Rust (typed HTTP client, SSE parsing, session/subagent models, state machine), **zero `tauri` dependency** — fully verifiable headless via `cargo check` / `cargo test`.
- `src-tauri` is thin glue only; it compiles once system deps are installed via `scripts/install-deps.sh` (user runs it once with sudo).

## OpenCode server API (evidence: official docs, opencode.ai/docs/server)

| Need | Endpoint |
|---|---|
| Send instruction to orchestrator | `POST /session/:id/message` · `POST /session/:id/prompt_async` |
| List subagents (child sessions) | `GET /session/:id/children` |
| Status of every session | `GET /session/status` |
| Live updates (finish/error/approval) | `GET /event` (SSE) |
| Respond to approval request | `POST /session/:id/permissions/:permissionID` body `{ response, remember? }` |
| Interrupt subagent | `POST /session/:id/abort` |
| Session diffs (for the editor) | `GET /session/:id/diff` → `FileDiff[]` |
| Read project files / tree / search | `GET /file/content?path=` · `GET /file` · `GET /find` |
| OpenAPI spec | `GET /doc` |

Auth: optional HTTP basic via `OPENCODE_SERVER_PASSWORD`.

## Tasks

- [ ] **T1 — Plan + scaffold**: worker A creates repo scaffold, `docs/ARCHITECTURE.md`, and `docs/IMPLEMENTATION_PLAN.md` (self-contained contract for agent 2). Route: delegated (`gentle-ai-worker`). Trigger: writer rule (2+ non-trivial files) + preparation rule.
- [ ] **T2 — Implementation**: worker B implements per the plan (test-first on `orquestador-core`), writes `docs/IMPLEMENTATION_NOTES.md` for agent 3. Route: delegated (`gentle-ai-worker`). Trigger: writer rule.
- [ ] **T3 — Audit + tests + docs**: worker C audits against plan, runs all checks, fixes bounded defects, finalizes README/docs, writes `docs/AUDIT.md`. Route: delegated (`gentle-ai-worker`). Trigger: writer rule + output budget.

Inter-agent channel: handoff docs in `docs/` (each agent reads the previous agent's artifacts; each returns a structured handoff). Parent commits after each task (workers never commit).

## Verification (per task)

- T1: scaffold exists · `cargo metadata --no-deps` succeeds (validates manifests **without compiling**) · `npm install` succeeds.
- T2: `cargo check -p orquestador-core && cargo test -p orquestador-core` · `npx tsc --noEmit` · `npm run build` (vite).
- T3: re-run T2 checks + `cargo clippy -p orquestador-core -- -D warnings` (or documented allowlist) · docs reviewed for completeness.

## Known environmental failures

- Any `cargo check`/`build` of the `src-tauri` app crate FAILS: `webkit2gtk-4.1` + `libayatana-appindicator3-0.1` missing, `sudo` needs a password. Expected, documented — not a defect of the code. Headless verification targets `orquestador-core` only.
- Full `tauri dev`/`tauri build` (running the GUI) is blocked until the user runs `sudo scripts/install-deps.sh`.

## Acceptance criteria

1. `orquestador-core`: unit tests green (API client, SSE event parsing, subagent state derivation).
2. Frontend: `tsc --noEmit` + `vite build` green.
3. Docs complete: `README.md` (setup/run), `ARCHITECTURE.md`, `IMPLEMENTATION_PLAN.md`, `IMPLEMENTATION_NOTES.md`, `AUDIT.md`.
4. `scripts/install-deps.sh` with the exact system packages for Tauri 2.
5. Conventional work-unit commits on `main` (parent-authored, no AI attribution).

## Progress log

- [x] Feature document created (parent, 2026-10-10) — env probed: Rust 1.99, Node 24, npm 11; webkit2gtk missing; RDD on.
- [x] T2 implemented (worker B) — core fix (`respond_permission` omits `remember: null`) + full frontend (store, 23 invoke commands, 16 components, local Monaco with 5 workers); 39 tests GREEN, clippy `--workspace --all-targets -D warnings` clean, `tsc --noEmit` clean, `vite build` OK, `scripts/verify-headless.sh` OK. src-tauri not compiled (env-blocked, expected).
- [ ] T3 audited/documented — pending
- Commits: `046b0c0` chore: scaffold orquestador workspace and implementation plan (T1) · `b662e94` feat: implement orquestador dashboard (core, UI, tauri shell) (T2)
- Next step: launch worker C (auditor: adversarial review + full headless verification + AUDIT.md/RUNBOOK + fix bounded defects; doubles as independent verifier — RDD assess returned high/unassessable, writer self-verification done).
