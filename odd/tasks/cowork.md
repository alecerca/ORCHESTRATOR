# Cowork — two people, one project (Option A)

## Objective

Give a small team the tools to work on one shared project **at the same time**
without stepping on each other, and let the server owner control who may join.
This is **read-shared, write-independent**: everyone sees everyone's sessions,
but each person owns and drives their own.

Scope of this document: the **design** requested by the user, plus the
implemented **identity + attribution** slice (Phase 1).

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
| **L5 · Presence** | who is online / typing | Phase 4 (design only) |
| **L6 · Access (whitelist)** | owner-managed users + roles + join | **Phase 2** (store + CLI); Phase 3 UX (design) |

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

## Phase 2/3 — Whitelist UX (design only; depends on Track 4 host mode)

- **Host side — "Members" dialog**: lists users from `proxy.json`, add/remove,
  pick role (editor/viewer), and shows an **invite** (proxy URL + username +
  the password once). Writing requires the app to know the proxy's config path /
  run the proxy — that is the **host mode** decision in Track 4, so it is not
  built yet.
- **Joiner side — "Join workspace"**: promote the Settings `username`/`password`
  fields into a small first-run screen ("server address, user, password"). The
  joined `username` becomes the member identity, so attribution works with zero
  extra setup.
- Enforcement stays **only** in the proxy (the trust boundary). The app never
  re-implements auth.

## Phase 4 — Presence (design only)

Who is online. Cheapest viable: an authenticated heartbeat endpoint on the proxy
(`GET /whoami` + in-memory last-seen), or a client-side convention on top of the
SSE stream. Explicitly **out of scope** for now.

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

## Delivery / RDD

- Delivery strategy: `ask-on-risk` (default). Work-unit commits on `feat/cowork`.
- Reviewed boundary: branch point `6b005bb` on `main`.
- Pending candidate `e241075` (medium, 16 files / 433 lines): the RDD review
  cannot advance — the START returns a `gentle-ai.review-integration.consent/v3`
  envelope and this runtime has no native `question` tool to relay it. Per
  contract: report the limitation and stop; resolve by running the review's
  `granted` invocation out-of-band or by `gentle-ai review mode disable`.
- Candidate `dba46ff` (Phase 2) committed after the blocked candidate; not yet
  assessed.
