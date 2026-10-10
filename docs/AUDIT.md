# AUDIT — Orquestador

> Independent adversarial audit (AGENT 3) of the Orquestador desktop app against
> `SPEC.md` and `docs/PLAN.md`. Date: 2026-10-10.
> Method: full source read, re-run of every verification command (no reliance on
> the author's `docs/IMPLEMENTATION_NOTES.md`), finding-by-finding resolution.

## Verdict

**PASS.** The headless core, the frontend, and the documented contract are
consistent with `SPEC.md`/`PLAN.md`. One real defect was found and fixed
(F1). Remaining items are hardening/quality notes that are either
environment-blocked or explicitly non-defects. The Tauri shell was reviewed by
inspection only (its crate cannot compile here — see "Environment-blocked").

## Verification matrix (real captured output)

| Check | Command | Result |
|---|---|---|
| Rust format | `cargo fmt --all -- --check` | exit 0 |
| Rust tests | `cargo test --workspace` | `39 passed; 0 failed` |
| Rust lints | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 (clean) |
| TS typecheck | `cd frontend && npm run typecheck` | exit 0 |
| Frontend build | `cd frontend && npm run build` | `✓ built` (Monaco: 5 workers emitted) |
| Full headless chain | `bash scripts/verify-headless.sh` | `OK — headless verification passed.` |
| Debug-macro audit | grep `todo!`/`unimplemented!` in `crates`, `src-tauri/src` | none |
| `console.log` audit | grep in `frontend/src` | none |
| GUI compile/run | `tauri dev` / `tauri build` | **ENV-BLOCKED** (webkit2gtk missing) |

`npm run build` emits `editor/css/html/json/ts.worker-*.js` — confirms Monaco
(editor + diff + language workers) is bundled locally, no CDN.

## Findings

| ID | Severity | Status | Location | Description | Resolution |
|---|---|---|---|---|---|
| F1 | Medium | **Fixed** | `crates/orquestador-core/src/client.rs:87` | `from_env()` read `OPENCODE_SERVER_URL`, but `README.md`, `docs/PLAN.md` §6.2 and `src-tauri/src/bridge.rs:28` all document `OPENCODE_SERVER_BASE_URL`. Setting the documented variable silently fell back to the default `127.0.0.1:4096`. | Now reads `OPENCODE_SERVER_BASE_URL` first, with `OPENCODE_SERVER_URL` kept as a legacy alias. Re-ran fmt/clippy/tests: green. |
| F2 | Low | Documented | `src-tauri/tauri.conf.json:25` | `security.csp = null` disables CSP. No XSS vector exists today (all untrusted server/LLM text is rendered as escaped React text nodes — no `dangerouslySetInnerHTML`/`innerHTML`/`eval`), but there is no defense-in-depth layer. | Cannot be tightened safely without `tauri dev` to validate that Monaco workers still load (env-blocked). Recommend a CSP allowing `'self'` plus `worker-src 'self' blob:` and `wasm-unsafe-eval` at release. |
| F3 | Low | Documented | `frontend/src/store.ts:312` | `useRawEvents` builds a fresh array inside `getSnapshot`, violating the `useSyncExternalStore` caching contract (React would warn/loop if it were used). It is currently **unused** (dead, like `useLastMessage`). | Either cache the mapped array or delete both unused hooks. Not fixed — dead code, zero runtime impact. |
| F4 | Info | Documented | `src-tauri/src/lib.rs:49` | `eprintln!("[orquestador] connected …")` is a non-error diagnostic; PLAN §11 reserves `eprintln!` for genuine error paths. | Cosmetic; the connection state is already surfaced in the UI. Left as-is. |
| F5 | Info | Documented | `crates/orquestador-core/src/types.rs` (`decode_payload`) | Typed `Event`/`Session` deserialization requires the full documented field set; a future or omitted field makes that event `Unknown`. The **raw** value is still forwarded, so the message stream UI is unaffected — only the reducer could miss a state change. | Acceptable forward-compat trade-off; raw SSE is the source of truth for display. |
| F6 | Info | Documented | `frontend/src/components/DiffViewer.tsx` | The diff is fetched on mount / session change only; incoming `session.diff` events do not live-refresh the viewer. | UX gap, not a bug. |
| F7 | Info | Env-blocked | `src-tauri/src/lib.rs:47` | `AppBridge::bootstrap` runs via `block_on` inside Tauri `setup`, so window creation waits for the initial health/connect round-trip (bounded by the HTTP request timeout). | Acceptable for a control-tower app; revisit if startup latency is observed once the GUI can run. |

No High-severity findings. No secrets are logged or persisted: the basic-auth
password lives only in memory from the environment (`client.rs`), is never sent
to the webview, and no `std::process::Command`/shell execution exists anywhere.

## Area-by-area review

- **Core API client (`crates/orquestador-core/src/client.rs`)** — PASS.
  Every endpoint required by SPEC §4 is implemented with the correct method and
  path. Session ids are percent-encoded in path segments; query values use a
  query-safe encoder (no injection). `respond_permission` correctly omits
  `remember` when unset (covered by `respond_permission_omits_remember_when_none`
  / `_includes_remember_when_set`).
- **SSE hub & reconnect (`events.rs`)** — PASS. Exponential backoff with jitter,
  resync-on-reconnect, dirty-snapshot suppression; covered by
  `hub_survives_a_server_that_dies_mid_stream`,
  `bootstrap_connects_reuses_nothing_and_reports_disconnection`,
  `only_state_changing_events_mark_the_snapshot_dirty`.
- **State reducer (`state.rs`)** — PASS. Status derivation, sticky error until
  the next run, notify-once semantics, permission-wins precedence, tree listing
  newest-first. Well covered by the test suite.
- **Tauri shell (`src-tauri/src/*`)** — PASS (inspection; crate env-blocked).
  Thin glue: command surface forwards to the core, `core://message` events are
  emitted from a single forwarder task seeded by `getState()`, capabilities are
  limited to `core:default`, and no fs/shell/http plugins are granted.
- **Frontend (`frontend/src/*`)** — PASS. `tsc --noEmit` and `vite build` clean;
  the Tauri command argument keys (camelCase JS → snake_case Rust) are
  consistent at every call site; untrusted text is rendered escaped; error
  surfaces propagate to the UI.
- **Scope discipline** — PASS. No worktree/issue/PR machinery, no multi-project,
  no `xterm`, no zustand/redux (store uses `useSyncExternalStore`). The
  `xterm` stretch goal is correctly deferred.
- **Docs** — PASS after F1. `README.md`, `docs/PLAN.md`,
  `docs/ARCHITECTURE.md`, `docs/IMPLEMENTATION_PLAN.md`,
  `docs/IMPLEMENTATION_NOTES.md` are internally consistent with the code, and
  the runbook is in `docs/RUNBOOK.md`.

## Environment-blocked (not a code defect)

`webkit2gtk-4.1` and `libayatana-appindicator3` are absent and `sudo` requires a
password, so the `src-tauri` app crate cannot be compiled or launched here. This
was a known constraint from the feature doc and drove the workspace split:

- `src-tauri` is a **standalone empty workspace**, so the root `cargo` never
  tries to compile Tauri — this is what makes the headless chain possible.
- The core crate has **no `tauri` dependency** and is fully exercised headless.

Consequently, `tauri dev`/`tauri build`, GUI startup, and anything requiring the
real window runtime (e.g. validating a tightened CSP) are **unverified** and out
of reach of this audit. `scripts/setup-system-deps.sh` prints the exact
`apt-get` command for the user to run.
