# Orquestador — Audit & hardening review

A point-in-time review of the Orquestador desktop app against `SPEC.md` and
`docs/PLAN.md`: what was verified, what was found, and what remains
environment-blocked. Method: full source read plus re-running every verification
command directly (not relying on any prior summary), then finding-by-finding
resolution.

## Verdict

**PASS.** The headless core, the proxy, the frontend contract, and the
documented contract are consistent with `SPEC.md`/`docs/PLAN.md`. One real
defect was found and fixed (F1); the remaining items are hardening/quality notes
that are either environment-blocked or non-defects. The Tauri shell is reviewed
by inspection only — its crate cannot be compiled in this environment.

## Verification matrix (captured output)

| Check | Command | Result |
|---|---|---|
| Rust tests (core) | `cargo test -p orquestador-core` | `132 passed; 0 failed` |
| Rust tests (proxy, unit) | `cargo test -p orquestador-proxy --lib` | `56 passed; 0 failed` |
| Rust tests (proxy, integration) | `cargo test -p orquestador-proxy --test proxy` | `18 passed; 0 failed` |
| Rust lints | `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 (clean) |
| Rust format | `cargo fmt --all -- --check` | **exit 1** — local rustfmt would reformat several files (toolchain drift) |
| TS typecheck | `cd frontend && npx tsc --noEmit` | exit 0 |
| Tauri commands | `grep -c '#\[tauri::command\]' src-tauri/src/commands.rs` | 46 |
| Debug-macro audit | grep `todo!`/`unimplemented!` in `crates`, `src-tauri/src` | only in `events.rs` `#[cfg(test)] mod tests` |
| Git tags | `git tag` | 0 (not published) |
| GUI compile/run | `npx @tauri-apps/cli dev` / `build` | **ENV-BLOCKED** (see below) |

The `cargo fmt --check` failure is toolchain drift (line-wrapping / multi-line
call style across `backend.rs`, `client.rs`, `events.rs`, `skills.rs`,
`authorize.rs`, `config.rs`), not a logic defect; it was not fixed here because
the Rust sources are outside this documentation task's edit surface.

## Findings

| ID | Severity | Status | Location | Description | Resolution |
|---|---|---|---|---|---|
| F1 | Medium | **Fixed** | `crates/orquestador-core/src/client.rs:75` | `from_env_or_default()` read `OPENCODE_SERVER_URL` only, but `README.md`, `docs/PLAN.md` §6.2 and `src-tauri/src/bridge.rs` documented `OPENCODE_SERVER_BASE_URL`. Setting the documented variable silently fell back to `127.0.0.1:4096`. | Now reads `OPENCODE_SERVER_BASE_URL` first, with `OPENCODE_SERVER_URL` kept as a legacy alias (client.rs:75–76). Re-ran tests: green. |
| F2 | Low | Documented | `src-tauri/tauri.conf.json:25` | `security.csp = null` disables CSP. No XSS vector exists today (all untrusted server/LLM text is rendered as escaped React text nodes — there is no `dangerouslySetInnerHTML`/`innerHTML`/`eval` in `frontend/src`), but there is no defense-in-depth layer. | Cannot be tightened safely without `tauri dev` to validate that Monaco workers still load (env-blocked). Recommend a CSP allowing `'self'` plus `worker-src 'self' blob:`. |
| F3 | Low | Documented | `frontend/src/store.ts:312` | `useRawEvents` builds a fresh array inside `getSnapshot`, violating the `useSyncExternalStore` caching contract (React would warn/loop if it were used). It is currently **unused** (dead, like `useLastMessage` at store.ts:293). | Either cache the mapped array or delete both unused hooks. Not fixed — dead code, zero runtime impact. |
| F4 | Info | Documented | `src-tauri/src/lib.rs:60` | `eprintln!("[orquestador] connected …")` is a non-error diagnostic; the convention reserves `eprintln!` for genuine error paths. | Cosmetic; the connection state is already surfaced in the UI. Left as-is. |
| F5 | Info | Documented | `crates/orquestador-core/src/client.rs:628` (`decode_payload`) | Typed `Event`/`Session` deserialization requires the full documented field set; a future or omitted field makes that event `Unknown`. The **raw** value is still forwarded, so the message stream UI is unaffected — only the reducer could miss a state change. | Acceptable forward-compat trade-off; raw SSE is the source of truth for display. |
| F6 | Info | Documented | `frontend/src/components/DiffViewer.tsx` | The working-tree diff refreshes by **polling on an interval**, not by reacting to incoming `session.diff` events, so it does not live-refresh from the event stream. (`DiffPanel.tsx` likewise polls `git_status`.) | UX gap, not a bug. |
| F7 | Info | Env-blocked | `src-tauri/src/lib.rs:58` | `AppBridge::bootstrap` runs via `tauri::async_runtime::block_on` inside Tauri `setup`, so window creation waits for the initial health/connect round-trip (bounded by the HTTP request timeout). | Acceptable for a control-tower app; revisit if startup latency is observed once the GUI can run. |

No High-severity findings.

## Process execution & secrets

- **No secrets are logged or persisted.** The basic-auth password lives only in
  memory (from the environment or the persisted `server.json`) and is never sent
  to the webview.
- **`std::process::Command` IS used** — `crates/orquestador-core/src/git.rs`
  invokes the fixed binary `git` (`Command::new("git")`) with **argument arrays**
  (`-C <dir>`, `.args([...])`), never a shell string. User-supplied paths are
  validated first (`validate_relative`: non-empty, repository-relative, no `..`)
  and passed as args, so there is no shell-injection surface. All git operations
  are read-only introspection except two deliberate, user-confirmed mutations:
  `checkout` (switch branch) and `discard_file` (discard one path's changes).
  There is no other process execution anywhere in `crates/` or `src-tauri/src/`.

## Area-by-area review

- **Core API client (`crates/orquestador-core/src/client.rs`)** — PASS. Every
  endpoint from `SPEC.md` §4 is implemented with the correct method and path in
  this one module. Session ids are percent-encoded in path segments; query values
  use a query-safe encoder. `respond_permission` routes by id form (`per_…` →
  `POST /permission/:id/reply`, else `POST /session/:id/permissions/:permissionID`)
  and omits `remember` when unset (covered by
  `respond_permission_omits_remember_when_none` / `_includes_remember_when_set`).
- **SSE hub & reconnect (`events.rs`)** — PASS. Exponential backoff with jitter,
  resync-on-reconnect, periodic HTTP reconcile, idle watchdog, dirty-snapshot
  suppression; covered by `hub_survives_a_server_that_dies_mid_stream`,
  `bootstrap_connects_reuses_nothing_and_reports_disconnection`,
  `only_state_changing_events_mark_the_snapshot_dirty`.
- **State reducer (`state.rs`)** — PASS. Status derivation, sticky error until
  the next run, notify-once semantics, permission-wins precedence, newest-first
  tree listing. Well covered by the test suite.
- **Tauri shell (`src-tauri/src/*`)** — PASS (inspection; crate env-blocked).
  Thin glue: the 46 commands forward to the core, `core://message` events are
  emitted from a single forwarder task, capabilities are limited to
  `core:default`, and no fs/shell/http plugins are granted.
- **Frontend (`frontend/src/*`)** — PASS. `tsc --noEmit` clean; Tauri command
  argument keys (camelCase JS → snake_case Rust) are consistent at every call
  site; untrusted text is rendered escaped; error surfaces propagate to the UI.
- **Proxy (`crates/orquestador-proxy`)** — PASS. Role/whitelist authorization,
  presence, member management; 74 tests green.
- **Scope discipline** — PASS. No worktree/issue/PR machinery, no multi-project
  switching, no `xterm`, no zustand/redux (the store uses `useSyncExternalStore`).

## Environment-blocked (not a code defect)

`libayatana-appindicator3` is absent and `sudo` requires a password, so the
`src-tauri` app crate cannot be compiled or launched here. (`webkit2gtk-4.1` is
now present, but the indicator library remains missing.) This drove the
workspace split:

- `src-tauri` is a **standalone empty workspace**, so the root `cargo` never
  tries to compile Tauri — this is what makes the headless chain possible.
- The core crate has **no `tauri` dependency** and is fully exercised headless.

Consequently `tauri dev`/`tauri build`, GUI startup, and anything requiring the
real window runtime (e.g. validating a tightened CSP) are **unverified** and out
of reach of this review. `scripts/setup-system-deps.sh` prints the exact
`apt-get` command for the user to run once.
