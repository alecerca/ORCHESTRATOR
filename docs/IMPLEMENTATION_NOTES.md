# Implementation Notes

Implementation of the Orquestador desktop control tower (PLAN.md steps S1–S13),
written by AGENT 2 (IMPLEMENTER). This document records what was built, the
decisions behind it, the verification actually run, and the known limits.

## Scope delivered

- **Core (`crates/orquestador-core`)** — one targeted fix (see below); the rest of
  the client / SSE hub / state model was already complete and green.
- **Frontend (`frontend/`)** — full TypeScript/React UI: store, typed command
  bindings, all components, local Monaco bundling and styling.
- **Tauri shell (`src-tauri/`)** — already scaffolded by AGENT 1; **not modified**
  and **not compiled** here (see *Environment*).

## Core change

`respond_permission` was sending `"remember": null` in the request body when the
caller passed `None`. PLAN §6.3 and the OpenAPI document specify that `remember`
is **omitted** when absent. Fixed so the JSON body is `{ "response": ... }` and
only includes `remember` when it is `Some(...)`.

- File: `crates/orquestador-core/src/client.rs`
- Tests added (RED before the fix, GREEN after):
  - `respond_permission_omits_remember_when_none`
  - `respond_permission_includes_remember_when_set`
- To observe request bodies in the mock server, its read loop now reads until a
  short idle timeout, so it captures `Content-Length` bodies.

PLAN §6.3 and the OpenAPI document specify that `remember` is **omitted** when
absent, not sent as `null`. The wire contract is strict about field presence, so
the fix keeps the request minimal.

## Frontend architecture

- **Store (`src/store.ts`)** — a hand-rolled store on `useSyncExternalStore`
  (SPEC §5 mandates no zustand/redux). It caches:
  - the latest `AppStateSnapshot` (state-changing messages),
  - a bounded ring buffer of raw core messages (cap 500) for the activity view,
  - a per-session reducer folding `message.*` / `message.part.*` events so a pane
    renders incrementally without re-scanning the buffer.
  Derived arrays are cached and only invalidated on mutation, so `getSnapshot`
  returns stable references between notifications (required by
  `useSyncExternalStore`). Hooks: `useAppState`, `useSessions`,
  `usePendingPermissions`, `useSessionStream`, `useRawEvents`, `useLastMessage`.
- **Commands (`src/commands.ts`)** — typed 1:1 bindings for all 23 Tauri
  commands. Tauri 2 maps camelCase JS keys to snake_case Rust params
  (`sessionId` → `session_id`, `permissionId` → `permission_id`, `messageId` →
  `message_id`).
- **Wire naming** — `types.ts` uses the capitalised abbreviations the core emits
  (`sessionID`, `parentID`, `messageID`, `permissionID`, `projectID`,
  `providerID`), verified field-by-field against `crates/orquestador-core/src/
  types.rs` and `state.rs`. `#[serde(rename_all = "camelCase")]` would be wrong.
- **Monaco (`src/monaco/setup.ts`)** — bundled locally (no CDN): imports
  `monaco-editor` and five `?worker` entrypoints (editor, json, css, html, ts),
  then `loader.config({ monaco })` so `@monaco-editor/react` uses the local
  instance. `main.tsx` imports this module before `App`.

## Design decisions & deviations

- **Approvals from the snapshot registry.** `ApprovalDialog` renders from
  `AppStateSnapshot.permissions` (the Rust core's pending registry) rather than
  only from transient `effect` messages — robust to approvals that arrive before
  the webview is listening. Responses are `once` / `always` / `reject`.
- **Notifications (PLAN over SPEC §5).** Desktop notifications are produced in
  Rust (`src-tauri/src/notify.rs`, `notify-rust`), driven by the same `Effect`
  values; the frontend additionally renders an in-app `NotificationsDrawer` fed
  by `core://message` effects. No notification plugin dependency.
- **Sub-agent list from the snapshot.** `AgentPanel` derives the orchestrator and
  its sub-agents from the cached snapshot (bootstrap lists every session), which
  is inherently live; `list_children` remains bound and available. This avoids a
  redundant fetch loop for data already in the event stream.
- **Editor is in-memory.** v1 exposes no file-write command, so Monaco edits are
  local to the running window and are not persisted. Documented in the component.
- **Pane layout** wraps responsively (`flex: 1 1 340px`) instead of a fixed
  column count. `ActivityBar` toggles the agent panel, editor, composer and theme,
  and can close all panes.
- **Theme** toggles via `data-theme` on `<html>` with matching CSS variables;
  persisted in `localStorage` and seeded from `prefers-color-scheme`.

## Files

```
crates/orquestador-core/src/client.rs     # remember-omission fix + 2 tests
frontend/src/types.ts                     # wire + stream + file/diff types, CORE_EVENT
frontend/src/commands.ts                  # 23 typed invoke wrappers
frontend/src/utils.ts                     # describeError, languageForPath, safeJson, ...
frontend/src/store.ts                     # snapshot + per-session reducer + raw ring buffer
frontend/src/App.tsx                      # layout, bootstrap, event listener, actions
frontend/src/main.tsx                     # imports ./monaco/setup
frontend/src/styles.css                   # two-theme palette + full layout
frontend/src/monaco/setup.ts              # local Monaco + 5 workers
frontend/src/components/*.tsx             # TopBar, ActivityBar, AgentPanel, AgentCard,
                                          #   StatusChip, PaneGrid, AgentPane, MessageStream,
                                          #   FollowUpInput, Composer, ApprovalDialog,
                                          #   NotificationsDrawer, EditorPanel, FileTree,
                                          #   MonacoEditor, DiffViewer
```

## Verification (actual results)

```
cargo fmt --check                                   -> clean
cargo test --workspace                              -> 39 passed; 0 failed
cargo clippy --workspace --all-targets -- -D warnings -> clean
stub audit (todo!/unimplemented!)                   -> none found
(frontend) npm run typecheck  (tsc --noEmit)        -> clean
(frontend) npm run build                            -> success; 5 monaco workers emitted locally
bash scripts/verify-headless.sh                     -> OK — headless verification passed.
```

Monaco workers emitted into `frontend/dist/assets/`:
`editor.worker`, `json.worker`, `css.worker`, `html.worker`, `ts.worker`.

The main JS bundle is ≈3.4 MB (Monaco dominates); acceptable for a local desktop
app. A future improvement is code-splitting the editor panel.

## Environment / not verified here

- **`src-tauri` was not compiled.** This environment lacks `webkit2gtk-4.1` and
  has no passwordless `sudo`, so the Tauri shell cannot build (see PLAN). The
  frontend↔shell contract was instead validated by cross-reading
  `src-tauri/src/commands.rs` (all 23 command names and argument names) and
  `src-tauri/src/lib.rs` (the `core://message` event name). **These must still be
  compiled and run in an environment with the Tauri system dependencies.**
- No git operations were performed (the orchestrator owns commits).
