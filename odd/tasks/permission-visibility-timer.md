# Permission visibility + execution timer — feature document

- **Feature**: `permission-visibility-timer`
- **Directory**: `/home/sanchiro/projects/orquestador` (git, `main`, no remote)
- **Created**: 2026-10-10
- **Delivery strategy**: `single-branch` (repo policy: work-unit commits land on `main`)

## Problem (live evidence, 2026-10-10)

The user saw the orchestrator and a subagent stuck on `running` for a very long time ("tuve que abortar una ejecución que seguramente ya habría terminado").

Evidence gathered from the live server (`http://127.0.0.1:4096`):

- `GET /session/status` → `{"ses_edb2968abffe...":{"type":"busy"},"ses_ed94fcd3effe...":{"type":"busy"}}` — the server really reports both as busy.
- `GET /permission` → **a pending `bash` permission on the orchestrator session** (`per_{id}`, sessionID `ses_edb2968abffe...`) whose `patterns` are the exact `git add … && git commit …` commands the agent tried to run. Nobody ever answered it.
- The child session's last assistant message has `time.created` but **no `time.completed`** → the turn never closed.

**Root cause**: pending permission requests are tracked **only** from the `permission.updated` SSE event (`state.rs:259-270`). There is **no bootstrap and no reconciliation** of pending permissions:

- `bootstrap()` fetches sessions, statuses and agents — never `GET /permission` (`events.rs:165-207`).
- `resync()` on reconnect refreshes statuses only (`events.rs:352-365`).
- `client.rs` has **no** `list_permissions`; only `respond_permission` (`client.rs:336`).

So any permission that arrives while the stream is down (the pre-`bb96ff0` 30 s total-timeout bug killed `/event` every 30 s) or before the app starts is **invisible forever**. The blocked session stays `busy` → the UI correctly shows `running`, but the user has no way to see or answer the approval. That is the "stuck running" and a large part of the perceived "slowness": the agent was not slow, it was **blocked waiting for an approval the UI never showed**.

## Objective

1. **Never lose a pending permission**: bootstrap and periodically reconcile pending permissions from `GET /permission`, so a blocked session is always visible and answerable.
2. **Show elapsed execution time** per row (orchestrator + subagents): live while `running`/`waiting_approval`, frozen duration when finished.
3. **Perceived latency / jank**: document the real UI overhead found (per-event full-tree re-render + full snapshot serialize) as a follow-up; the permission fix is the actual execution unblock.
4. **Picker styling** (carried over): compact, dark-theme-legible orchestrator select.

## Constraints

- No server-side deletion; no destructive changes.
- Core stays headless-testable (TDD, mock HTTP/SSE server as in existing tests).
- Frontend has no test runner (by design) → proof = `npx tsc --noEmit` + `npm run build`.

## Tasks

- [x] **P1 — Permissions never lost (core)**: `Client::pending_permissions()` (`GET /permission`) + `AppState::apply_pending_permissions_at()` (add-only dedup union, never removes on reconcile) applied in `bootstrap()`, `resync()`, and a new 5 s `RECONCILE_INTERVAL` tick inside `EventHub::connect_once` (`reconcile_once()`), plus `PermissionRequest`/`PermissionToolRef` types folding into `Permission`. TDD: 10 new tests (bootstrap-without-event, late arrival via tick, replied-not-resurrected). Route: delegated core writer (strict TDD).
- [x] **P2 — Execution timer data (core)**: `status_since` / `last_run_ms` per session tracked in the reducer (run window = `Running | WaitingApproval`), exposed on `SessionSnapshot`, cleared on `SessionDeleted`. TDD. Route: same core writer session as P1.
- [x] **P3 — Timer UI + picker CSS (frontend)**: new leaf `AgentTimer.tsx` (1 s interval isolated from `App`), `M:SS` / `H:MM:SS` with `tabular-nums`, warn-colored `waiting_approval`; `.agent-panel__select` replaces the `composer__input` reuse + inline style. Route: delegated frontend writer (ran after P1/P2).
- [x] **P4 — Verify + commit**: `cargo test -p orquestador-core` 54 passed; `npx tsc --noEmit` clean; `npm run build` OK; `scripts/verify-headless.sh` → "OK — headless verification passed". Route: parent.

## Progress log

- [x] Feature document created (parent, 2026-10-10) with live-server evidence.
- [x] P1+P2 implemented (delegated core writer, strict TDD) — 44 → 54 tests; clippy `-D warnings` + fmt clean.
- [x] P3 implemented (delegated frontend writer) — 5 files (1 new component); tsc + build green.
- [x] Parent spot checks: `cargo test -p orquestador-core` → 54 passed; `npx tsc --noEmit` → clean; `scripts/verify-headless.sh` → passed.
- Known approximation: `status_since` on first observation of a session is the observation time, not the true transition time (documented in the core writer's report).
- Follow-up still open: frontend event coalescing (per-token full-tree re-render + snapshot serialize) — separate feature.

## Documented follow-up (NOT in this feature)

- Frontend event coalescing: `store.pushMessage` invalidates caches and notifies on **every** raw event (`store.ts:239-255`), and `useAppState` sits at `App.tsx:86`, so the whole tree re-renders per token; `deriveStream` re-derives all messages per cache miss (`store.ts:194-216`). Real UI jank under long sessions — separate feature.
- Optimistic `abort_session` local update (currently HTTP only, no local state change) — partially covered by the P1 reconcile tick.

## Verification

- Core: `cargo test -p orquestador-core` (all green, new tests included) + `cargo clippy -p orquestador-core -- -D warnings` + `cargo fmt --check`.
- Frontend: `npx tsc --noEmit` + `npm run build`.
- Manual acceptance (user): with a permission pending, the app shows the approval dialog without needing a reconnect.

## Acceptance criteria

1. A pending permission that exists on the server is surfaced by the app even if its `permission.updated` event was missed (bootstrap and periodic reconcile).
2. Answering a permission removes it from the pending list.
3. Each row shows elapsed time: live while running/waiting, frozen when finished.
4. Picker renders compact and legible in dark theme.
5. No regression: `42+` core tests green, frontend typecheck/build green (actual: 54 core tests).
