# Cowork — two people, one project (Option A)

## Objective

Give a small team the tools to work on one shared project **at the same time**
without stepping on each other, and let the server owner control who may join.
This is **read-shared, write-independent**: everyone sees everyone's sessions,
but each person owns and drives their own.

Scope of this document: the **design** requested by the user, plus the
implemented slices — Phases 1–3 (identity + attribution, hashed whitelist
store, whitelist UX) are built; Phase 4 (presence) is design.

## Problem

Today the app connects as a single anonymous client (`username: opencode`).
On a shared `opencode serve` everyone sees every session, but **nothing says who
owns what** — sessions are anonymous, and a running agent does not tell you whose
work it is. Two people cannot tell "this is mine" from "this is my teammate's".

## Decisions (from the user)

- **Option A.** Give people the tools; they organize themselves. Do **not** build
  turn-locking, shared-session prompting, or file locks.
- **File collisions are the users' responsibility**, exactly like git. We *show*
  awareness (who is touching what); we do not *block*.
- **Session title carries the creator's identity** (`[alice] fix login`), and it
  stays **renameable**.
- **Running agents show the username** — same pattern as "working in {branch}".
- **Whitelist**: the server owner creates users with a password; a joiner enters
  that username + password to connect.
- No cloning / no reconstruction. Careful not to break existing behavior.

## Layer model

| Layer | Adds | Status |
|---|---|---|
| **L0 · Transport** | HTTP + SSE to one `opencode serve` | ✅ exists |
| **L1 · Identity** | a member (display) name per app instance | **Phase 1** |
| **L2 · Attribution** | creator name in the session title | **Phase 1** |
| **L3 · Owner display** | "by <name>" chip on every agent/sub-agent | **Phase 1** |
| **L4 · Room UX** | all sessions visible (tabs already exist) | ✅ exists |
| **L5 · Presence** | who is online / typing | Phase 4 (design) |
| **L6 · Access (whitelist)** | owner-managed users + roles + join | **Phase 2** (store + CLI); **Phase 3** (UX) |

## Phase 1 — Identity, attribution, owner chip (IMPLEMENT)

### Identity
- New optional `member_name` in app + backend config. **Effective identity** =
  `memberName` → else `username` → else `"opencode"`.
- Editable in **Settings** as "Display name (shown on your sessions)".
- Persisted in `localStorage` (`serverConfig`) **and** mirrored to `server.json`
  so the Rust bootstrap can attribute the session it creates.

### Attribution
- Title format: `[<member>] <base>`, e.g. `[alice] New session`.
- Applied where **this app creates a session**:
  - explicit "New session" (`AgentPanel.handleCreate`);
  - the lazily-created root session (Rust `bootstrap`, `events.rs:149`).
- Parser is tolerant: `^\[([^\]]+)\]\s+` → owner, rest = title. A title without
  the prefix has **no owner** (e.g. sessions made by another tool).
- Renaming keeps working (rename is already supported); renaming may drop the
  prefix — that is the user's call.

### Owner display
- `OwnerChip`: resolves the **root ancestor** of a session by walking `parentId`
  through the snapshot, then reads the `[name]` prefix from the root title.
- Rendered next to `BranchChip` in `AgentCard` and `GroupInfoBar`, so **every
  running agent/sub-agent shows the owner** of its session tree.
- Renders nothing when no owner can be determined.

### Acceptance
- `cargo test --workspace` green (new `member_name` covered).
- `npm run typecheck` green.
- Manual: two sessions created by two different `Display name`s show their own
  `[name]` prefix; every sub-agent under a session shows the same owner chip;
  branch chip still works.

## Phase 2 — Whitelist user store (IMPLEMENTED)

The role-enforcing proxy (`crates/orquestador-proxy`, `odd/tasks/role-proxy.md`)
**already implements the whitelist**:

```json
{ "listen": "0.0.0.0:4097",
  "upstream": "http://127.0.0.1:4096",
  "upstream_username": "opencode",
  "users": [ { "name": "alice", "token": "…", "role": "editor" } ] }
```
It maps `username → {token, role}`, injects the real upstream credentials, and
enforces viewer (safe methods) vs editor (all). That **is** "owner creates a user
with a password; the joiner enters it".

**Gap to close:** passwords are plaintext in the file and there is no UI to
create users. Design:

- **Hashing**: store `password_hash` (Argon2id, salted per user). Keep accepting
  the legacy `token` field for back-compat; new users get hashes. Compare in
  constant time (already the pattern in `auth.rs`).
- **Owner tooling**: `orquestador-proxy adduser <name> --role editor` — prompts
  for the password, hashes, and appends to `proxy.json` (never prints the hash
  back). This is the "server owner creates a user with a password" path without
  hand-editing JSON.
- **Config validation**: reject duplicate names (already), empty password, and
  unknown roles (already).

**Delivered** (commit `dba46ff`): `User` gained `password_hash` and its `token`
became `Option<String>` (legacy configs still parse); `resolve_user` verifies the
Argon2id hash first (fail-closed on a malformed PHC string) and otherwise falls
back to the constant-time legacy token compare; new `adduser` module + the
`orquestador-proxy adduser <name> --role <editor|viewer> [--config <path>]`
subcommand reads the password from stdin and appends to the raw config JSON so
existing users and unknown fields survive. The `argon2` crate is added to the
proxy crate only.

## Phase 3 — Whitelist UX (IMPLEMENT)

**Host model (decided):** `odd/tasks/role-proxy.md` fixes the placement — *the
host runs `orquestador-proxy` in front of `opencode serve`*. So the app does
**not** run the proxy: it **edits a `proxy.json`** located at a path configured
in Settings, and derives the invite from it. Adding app-managed proxy startup is
a later, additive step.

- **Host side — "Members" dialog**: lists users from `proxy.json`, add/remove,
  pick role (editor/viewer), and shows an **invite** (proxy address + username +
  the password once).
- **Joiner side — "Join workspace"**: promote the Settings `username`/`password`
  fields into a small first-run screen ("server address, user, password"). The
  joined `username` becomes the member identity, so attribution works with zero
  extra setup.
- Enforcement stays **only** in the proxy (the trust boundary). The app never
  re-implements auth; it only reads/writes the owner's whitelist file.

### Tasks (Phase 3)

- **T5 [Rust]** `src-tauri` proxy-config module + commands: `list_members`,
  `add_member` (Argon2id, reject duplicate/empty), `remove_member`, reading and
  writing the `proxy.json` at a caller-supplied path; unit tests with a temp dir.
- **T6 [FE]** Settings "Proxy config path" field + the "Members" dialog
  (list/add/remove + invite), wired to the T5 commands.
- **T7 [FE]** "Join workspace" first-run screen writing the server config; the
  joined username drives member identity via the existing fallback.

### Verification (Phase 3)

- `cargo test --manifest-path src-tauri/Cargo.toml --lib`
- `cargo test --workspace` (proxy untouched; regression)
- `npm run typecheck`
- Manual: point at a `proxy.json`, add a user, confirm the file gains an
  `$argon2id$…` hash and that the proxy authenticates it; the Join screen
  persists the connection and the username shows up as the member identity.

## Phase 4 — Presence (IMPLEMENTED)

**Goal.** Show *who is currently connected* to the shared workspace. Presence
here means "online" (identity + last-seen); "typing" is a follow-up (see Open
questions).

### Why the proxy owns it

The proxy already sees *every* authenticated request per identity: `handle`
resolves `(name, role)` from the inbound Basic header before forwarding
(`crates/orquestador-proxy/src/proxy.rs`). It is the only component that knows
which whitelisted user is talking to the workspace. The app has no
"connected members" concept, and `opencode serve` has no member model at all.

### Recommended approach — proxy heartbeat (cheapest viable)

1. **Track last-seen in the proxy.** `AppState` gains
   `presence: Mutex<HashMap<String, Instant>>` (name → monotonic last request).
   `handle` records the authenticated user *after* auth and before the role
   gate, so a read-only viewer is still "online".
2. **Reserve two proxy-native routes** in `router`, ahead of the fallback:
   - `GET /orquestador/whoami` → `{ name, role }` for the caller (also a clean
     credential check).
   - `GET /orquestador/present` → `[{ name, role, idleMs }]` for users seen
     within the window.
   Both require valid Basic auth (reuse `authenticate`) and are read-only. The
   `/orquestador/` prefix is reserved and never forwarded upstream.
3. **Heartbeat = the app's own poll.** Window `W` ≈ 3× poll interval `P`
   (`W` ≈ 45s, `P` ≈ 15s): each poll refreshes the caller's last-seen, so no
   separate heartbeat write is needed.

### Wiring

- **core** (`AgentBackend`): add an optional `presence()`; the HTTP backend
  GETs `/orquestador/present`, and a 404 / transport failure (talking to
  `opencode` directly, no proxy) returns an **empty list, not an error** —
  presence is feature-detected, never assumed.
- **src-tauri**: expose the list through the existing bridge/command pattern.
- **frontend**: a compact presence strip (green dot + name + role, `(you)` on
  the caller), reusing the `OwnerChip` / `BranchChip` / `StatusChip` look; hide
  it entirely when the list is empty (feature off / no proxy).

### Limits (documented, accepted)

- Only clients **through the proxy** are visible; a direct `opencode` client is
  invisible (and was never access-controlled).
- Presence is **global to the instance**, not per project — consistent with
  Option A ("everyone sees everything").
- A user whose poll loop is alive is online even while idle.
- In-memory: a proxy restart clears presence.

### Alternatives considered

- **Client-side convention on the SSE stream** (a heartbeat session each client
  writes): rejected — needs write access (`viewer` cannot write) and pollutes
  the shared session state.
- **`opencode serve` awareness**: rejected — OpenCode has no member model, and
  this would change how it authenticates.

### Open questions

- Typing indicator: deferred; needs a shared, low-latency channel.
- Granularity: instance-wide vs per project (this design: instance-wide).
- Showing known-but-offline members vs only online (this design: only online).

### Implementation tasks

- **T8 [Rust/proxy] DONE** `ecdde5d` — `presence: Mutex<HashMap<String, Instant>>`
  on `AppState`; recorded in `handle` after auth and before the role gate (a
  read-only viewer counts as online); routes `/orquestador/whoami` and
  `/orquestador/present` registered ahead of the fallback; the `present` poll
  refreshes the caller (heartbeat); `PRESENCE_WINDOW` = 45 s; the
  `/orquestador/` prefix is reserved and 404s instead of being forwarded
  upstream. 6 new integration tests (identity, 401, viewer counted, heartbeat
  refresh, window expiry, reserved-prefix never forwarded).
- **T9 [Rust/core + tauri] DONE** `eb58b43` — `PresenceEntry`;
  `AgentBackend::presence()` with a default `Ok(Vec::new())` (feature-detected,
  never assumed); `OpenCodeClient::presence()` GETs the route and degrades to an
  empty list on 404 / transport / decode failure; `get_presence` command wired
  in `lib.rs`. 3 new core tests.
- **T10 [FE] DONE** `fc6a7b3` — `PresenceStrip` polls `get_presence` every 15 s
  (the poll is the heartbeat), renders dot + name + role badge with `(you)` on
  the caller, returns `null` while the list is empty, keeps the last good list
  on a failed poll, and guards against overlapping polls.

### Design verification

- **Automated (below the UI):** `cargo test --workspace` green (core 106, proxy
  48 lib + 12 integration); `cargo test --manifest-path src-tauri/Cargo.toml
  --lib` 18 passed; `npm run typecheck` (from `frontend/`) exit 0.
- **Still manual:** the two-client smoke pass below — the strip's polling,
  `(you)` marker and hidden-when-empty behaviour are typecheck-only verified.
- Keep the response shape additive so the FE can ship before the proxy route.

## Phase 5 — Project scope containment (design)

### The problem, as it stands today (verified in code)

A whitelisted client can point the shared server at **any project directory on
the host machine**:

1. `ClientConfig.directory` is sent on every scoped request as
   `X-Opencode-Directory` (`crates/orquestador-core/src/client.rs:35`, `:143`,
   including the SSE event stream).
2. `setProjectDirectory` only writes the caller's **own** `server.json` and
   reconnects that client (`src-tauri/src/commands.rs:183-197`) — so it does not
   mutate a shared server state. The problem is not contamination between
   clients; it is that the header is the client's own choice.
3. `ProjectPicker` offers every project from `GET /project` **and a free-text
   absolute path** (`frontend/src/components/ProjectPicker.tsx:66-88`).
4. The proxy forwards `X-Opencode-Directory` **verbatim**: `strip_request_header`
   removes only `authorization`, `accept-encoding`, `content-length`, `host` and
   hop-by-hop headers (`crates/orquestador-proxy/src/proxy.rs:157-167`).
5. `GET /project` and `GET /project/current` are proxied too, so a whitelisted
   client can **enumerate** every project the server knows
   (`crates/orquestador-core/src/client.rs:244-251`).

Net effect: the whitelist gates **who** may talk and the role gates **which HTTP
methods** they may use, but nothing gates **which project directory** they may
scope to. A whitelisted `editor` can select another project and read/modify its
files through the agent.

**Open question to verify before implementing:** whether `opencode serve` clamps
`X-Opencode-Directory` to projects it already knows, or accepts an arbitrary
path. Either way the leak is real — every *known* project stays reachable — so
the fix does not depend on the answer.

### Threat model — read this before trusting the fix

Project scoping narrows **which directory is the working project**. It is not a
sandbox. The agent runs with the host user's privileges and can execute shell
commands, so a whitelisted `editor` can, in principle, ask it to touch anything
that user can. If the machine holds projects that a coworker must not touch at
all, the correct answer is **not a proxy rule**: use a separate OS user, a
container or a VM per trust boundary, or grant `viewer` only. The proxy can make
the *project switch* safe; it cannot make the *agent* harmless.

### Where enforcement must live

The proxy. It is the only component that (a) resolves the authenticated user per
request and (b) can rewrite the header before it reaches upstream. A UI-only
disable is not security: the header is one `curl` away.

### Option A — per-user project scope (recommended)

Extend `proxy.json` additively:

```json
{
  "defaultProject": "/home/owner/projects/team-app",
  "users": [
    { "name": "ana", "role": "editor",
      "projects": ["/home/owner/projects/team-app"] },
    { "name": "bob", "role": "viewer",
      "projects": ["/home/owner/projects/team-app", "/home/owner/projects/docs"] }
  ]
}
```

Enforcement in `handle`, **before** forwarding:

- `projects` **absent** → today's behaviour (unrestricted). Keeps existing
  deployments working; opt in per user.
- `projects` **present** → the requested directory must match one entry exactly,
  compared after `normalize_directory` + `std::fs::canonicalize` on both sides
  (deny when a path cannot be canonicalized — fail closed; comparing raw strings
  would let `../` and symlinks escape the allowlist).
- Header **absent** for a scoped user → the proxy **injects** that user's first
  allowed project, so the client never falls through to the server's own cwd
  project by accident.
- Not allowed → `403 project_not_allowed`, audited, never forwarded upstream.
- `GET /project` for a scoped user → the proxy filters the response to that
  user's allowlist, so the picker cannot enumerate the host's other projects.
  (This is the one place the proxy must buffer a response body; it is a small
  JSON document and the `/event` stream is untouched.)

CLI: `orquestador-proxy adduser <name> --role <editor|viewer> --project <path>`
(repeatable).

### Option B — one pinned project for the whole shared instance (simplest)

A top-level `projectDirectory` (or a `--directory` flag): the proxy **strips any
client** `X-Opencode-Directory` and injects the pinned path. Every client, no
matter what it sends, works on the same project. No per-user bookkeeping, no
path comparison, no response filtering — safety by construction, and it matches
the ordinary "cowork on one project" case.

### Option C — disable the Project button for remote clients

Rejected as a **security** control (the header is still forwarded; the UI is
decoration). Worth doing as **UX** on top of A or B: `GET /orquestador/whoami`
(already implemented in T8) can grow a `projects` field so the app knows its own
scope, hides the free-text input, lists only allowed projects, and labels the
picker as locked. The app must never be the enforcement point.

### Recommendation

Ship **B** first — it is a few lines in the proxy and closes the hole
immediately — then **A** when someone genuinely needs more than one project.
Both are additive config, so neither breaks a current deployment.

### Acceptance criteria (T11+)

- A scoped user requesting a non-allowlisted directory gets `403` and the stub
  upstream sees **nothing**.
- A scoped user sending no directory header lands on its allowed project, not on
  the server's cwd.
- `GET /project` returns only the caller's allowed projects.
- An unrestricted user (no `projects` key) behaves exactly as today.
- Path comparison fails closed on `..`, trailing slashes and symlinks.

## Out of scope

- Shared-session prompting (two people in one conversation), turn locking,
  permission races.
- File locking / merge resolution (user responsibility, like git).
- TLS termination and deployment hardening (Track 2 follow-up).
- Any change to how OpenCode itself authenticates.

## Constraints

- Reuse existing surfaces: `ServerConfigFile`, `ClientConfig`, `AgentCard`,
  `BranchChip`, `GroupInfoBar`, `proxy.json`.
- No new dependencies in `crates/orquestador-core` or `src-tauri` for Phase 1.
- Artifacts (code, IDs, strings) in English; comments neutral.
- Keep the change additive; do not alter existing session/rename/abort behavior.

## Tasks (Phase 1)

- **T1 [Rust]** `member_name` on `ClientConfig` + `ServerConfigFile` +
  `to_client_config` + the `ServerConfigDto` / `set_server_config` path, with
  unit tests (round-trip, fallback chain).
- **T2 [Rust]** Attribute the bootstrap root title: compose
  `[<member>] Orchestrator` (fallback `Orchestrator`); test via the FakeBackend
  bootstrap test.
- **T3 [FE]** `memberName` in `ServerConfig` type + `serverConfig.ts` (load/save)
  + `commands.setServerConfig`/`getServerConfig`; a "Display name" field in
  `SettingsDialog`.
- **T4 [FE]** `attributedTitle(member, base)` + `parseOwner(title)` helpers; use
  in `AgentPanel.handleCreate`; new `OwnerChip` rendered in `AgentCard` +
  `GroupInfoBar` (resolve owner via root-ancestor walk).

## Verification (Phase 1)

- `cargo test --workspace`
- `cargo build -p orquestador-proxy` (unchanged; sanity)
- `npm run typecheck` (frontend)
- Manual: open two sessions with different display names; confirm `[name]`
  prefixes, owner chips on agents and sub-agents, and that rename/abort still
  work.

## Progress

- 2026-10-12: feature doc created; Option A confirmed by the user; whitelist
  found already implemented at the proxy layer (design recorded above).
  Phase 1 (identity/attribution/owner chip) routed to a bounded writer.
- 2026-10-12: **Phase 1** implemented and committed `e241075` (branch
  `feat/cowork`, base `6b005bb`). Verified: core 101 / proxy 30 / proxy
  integration 5 / src-tauri lib 6, `npm run typecheck` exit 0.
- 2026-10-12: **Phase 2** implemented by a bounded writer and committed
  `dba46ff`. Verification: `cargo test -p orquestador-proxy` → 48 lib + 6
  integration, 0 failed; `cargo build -p orquestador-proxy` clean;
  `cargo test --workspace` green. Parent spot check reproduced 48 + 6.
- 2026-10-12: **Phase 3** (§ "Whitelist UX") implemented by bounded writers in
  three work units on `feat/cowork`:
  - **T5** `22c6076` — `src-tauri/src/proxy_config.rs` (list/add/remove, Argon2id,
    unknown-field preserving; `argon2 0.6`, no axum) + three Tauri commands.
  - **T6** `f1fb0a3` — persisted `proxyConfigPath` end to end (Rust config + DTO +
    command, FE store/commands/Settings) + the Members dialog (+ invite).
  - **T7** `bae0b84` — dismissible Join-workspace first-run screen (reopens from
    Settings; persists via the same `setServerConfig` path).
  - Verified per unit: `cargo test --manifest-path src-tauri/Cargo.toml --lib`
    → 18 passed; `cargo test --workspace` green; `npm run typecheck` (from
    `frontend/`) exit 0. Parent spot checks reproduced each.
  - **Not automated:** the Members/Join dialogs are typecheck-only verified —
    they need a manual smoke pass (see Verification (Phase 3)).
- 2026-10-12: **Phase 4 (presence) design** written, grounded in the proxy's
  `handle`/`router` (it already resolves `(name, role)` per request). Decision:
  proxy-owned in-memory last-seen + two reserved proxy-native routes
  (`/orquestador/whoami`, `/orquestador/present`); the app polls and degrades to
  "no presence" on a proxy-less setup. No code — design only (T8–T10 pending).
- 2026-10-12: **Phase 4 (presence) implemented** by bounded writers in three
  work units on `feat/cowork`:
  - **T8** `ecdde5d` — proxy presence: last-seen on `AppState`,
    `/orquestador/whoami` + `/orquestador/present` ahead of the fallback,
    reserved-prefix guard, 45 s window, poll refreshes the caller. 6 new
    integration tests → `cargo test -p orquestador-proxy` 48 lib + 12
    integration. Parent spot check reproduced.
  - **T9** `eb58b43` — `PresenceEntry`; `AgentBackend::presence()` defaulting to
    `Ok(vec![])`; `OpenCodeClient::presence()` degrading to empty on 404 /
    transport / decode failure; `get_presence` command registered. Verified:
    `cargo test --workspace` → core 106, proxy 48 + 12; `cargo test
    --manifest-path src-tauri/Cargo.toml --lib` 18.
  - **T10** `fc6a7b3` — `PresenceStrip` (15 s poll = the heartbeat, `(you)`
    marker, `null` while empty, silent on failure, no overlapping polls) mounted
    in `TopBar`. `npm run typecheck` (from `frontend/`) exit 0. **Not
    automated:** the frontend has no test runner — the strip needs the
    two-client manual smoke pass.
- 2026-10-12: **Phase 5 (project scope containment) designed** (see above) after
  verifying in code that `X-Opencode-Directory` is forwarded verbatim by the
  proxy and that the picker accepts arbitrary absolute paths. Not implemented —
  it waits for a decision between Option B (pin one project) and Option A
  (per-user allowlist).

## Delivery / RDD

- Delivery strategy: `ask-on-risk` (default). Work-unit commits on `feat/cowork`.
- Reviewed boundary: branch point `6b005bb` on `main` (nothing reviewed yet).
- `gentle-ai review assess --base-ref 6b005bb --committed-only` over
  `6b005bb..HEAD` → **high** (`hot_path`: `crates/orquestador-proxy/src/auth.rs`),
  35 paths / 3282 changed lines, `review_due: true` (`high_risk`).
- Per-task RDD record (Phase 4): T8/T9/T10 each assessed on the branch range →
  `high` / `review_due: true`; the review stays blocked by the consent envelope
  below, so all three are recorded as **due, not reviewed**.
- The preflight returned a `review.start` whose relay yields a
  `gentle-ai.review-integration.consent/v3` envelope. **Not started:** this
  runtime has no native `question` tool to present the prompt and the contract
  forbids a chat-token fallback, so the review is reported-and-stopped. Resolve
  by running the review's `granted`/`declined` invocation in a runtime that can
  present the prompt, or `gentle-ai review mode disable`.
- Pending candidate `e241075` (medium): same blocker, reported earlier.
- Delivery size: Phase 3 alone is ~1300 authored lines across 3 work-unit
  commits, past the ~400-line heuristic → apply the delivery strategy (single
  merge to `main` vs chained PRs) before any PR.
- Candidate `dba46ff` (Phase 2) committed after the blocked candidate; not yet
  assessed.
