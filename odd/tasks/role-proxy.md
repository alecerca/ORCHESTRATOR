# Role-enforcing proxy (cowork — Track 2, hard half)

## Objective

Build a standalone `orquestador-proxy` binary that sits in front of an
`opencode serve` instance and enforces **per-user roles**, so remote clients can
join a shared project as **viewers** (read-only) or **editors** (full control).

This is the "owned role-enforcing proxy" called for by `docs/ROADMAP.md`
(Track 2, "The real gap — read-only / roles") and delivery-order step 3.

## Problem

OpenCode authenticates with a **single shared password** and has **no RBAC**:
anyone who can reach the server can also execute — send prompts, answer
permissions, mutate sessions. Cowork needs "connect but cannot execute".

## Why now

Settings (`base_url` + `username` + `password`) already landed, so a client can
point at any address. The missing half is the role gate in front of OpenCode.

## Design (v0)

- **New workspace crate** `crates/orquestador-proxy` (bin + lib). It is
  standalone and has **no dependency on `orquestador-core` or `src-tauri`**.
- **Placement**: the host runs `orquestador-proxy` in front of `opencode serve`.
  Clients point their Settings `base_url` at the proxy; the proxy forwards to
  the real `opencode serve`.
- **Auth**: **HTTP Basic**, reusing the client's existing `username` + `password`
  fields → **zero frontend change**. The client sends
  `Authorization: Basic base64(name:token)`; the proxy maps `name → {token, role}`
  and injects the real upstream credentials. Clients never see the upstream
  password.
- **Roles**:
  - `viewer` — safe methods only (`GET`/`HEAD`/`OPTIONS`), which includes the
    `/event` SSE stream. Any write method is rejected with `403`.
  - `editor` — all methods.
  - Classification is **method-based**: OpenCode's read endpoints are all `GET`;
    writes are `POST`/`PATCH`/`DELETE`. (Revisit if an upstream `GET` ever gains
    side effects.)
- **Config** (`proxy.json`, parsed with the already-present `serde_json`):
  ```json
  {
    "listen": "0.0.0.0:4097",
    "upstream": "http://127.0.0.1:4096",
    "upstream_username": "opencode",
    "users": [
      { "name": "alice", "token": "change-me-1", "role": "editor" },
      { "name": "bob",   "token": "change-me-2", "role": "viewer" }
    ]
  }
  ```
  Upstream password comes from the `ORQUESTADOR_UPSTREAM_PASSWORD` env var
  (preferred) or an optional `upstream_password` config field.
- **Forwarding**: strip the inbound `Authorization` and all hop-by-hop headers
  (`host`, `connection`, `content-length`, `transfer-encoding`, `keep-alive`,
  `te`, `upgrade`, `proxy-*`), add upstream Basic auth, forward method / path /
  query / body, and **stream** the response back (SSE `/event` must pass through
  without buffering).
- **Audit**: one line per request to stderr:
  `user=<name|-> role=<role|-> method=<M> path=<p> status=<code> <allowed|denied|unauthenticated>`.
- **TLS**: v0 assumes a private network (Tailscale / SSH tunnel). TLS termination
  is a follow-up; the README must say so loudly. **Never** suggest exposing the
  proxy or upstream plainly on `0.0.0.0`.

## Out of scope (v0)

- TLS termination in the proxy; per-user model / API-key routing; session
  ownership; message persistence; dynamic token provisioning; rate limiting.
- Any change to `frontend/`, `src-tauri/`, or `orquestador-core`.

## Constraints

- Keep new dependencies inside the proxy crate only (`axum` for the server,
  `base64` for Basic decoding, plus the workspace's `tokio` / `reqwest` /
  `serde` / `serde_json`). Do **not** add deps to the root workspace table or to
  core/`src-tauri`.
- Constant-time token comparison (avoid a byte-by-byte early-exit `==` on
  secrets).
- The crate must not depend on Tauri and must compile headless.
- No secrets committed; example config uses obvious placeholders.

## Tasks

- **T1 — core (pure, unit-tested)**: crate skeleton; `config.rs` (`Config`,
  `User`, `Role`, parse + validate, `id()`/lookup); `auth.rs` (`parse_basic` →
  `(user, token)`, constant-time compare, credential resolution); `authorize(method,
  role) -> bool`. No network.
- **T2 — server**: `axum` server; Basic-auth extraction → 401 on missing/invalid;
  role gate → 403 for viewer writes; upstream forwarding incl. SSE passthrough;
  audit logging; CLI (config path from argv or env, default `./proxy.json`).
  Integration test against a stub upstream.
- **T3 — docs**: `crates/orquestador-proxy/README.md` (run instructions, config,
  security note) and a short pointer in `docs/ROADMAP.md` (mark step 3 as
  "v0 implemented").

## Acceptance criteria

- `cargo test -p orquestador-proxy` — unit + integration green.
- `cargo test --workspace` — still green (new crate included).
- `cargo build -p orquestador-proxy` — produces the binary.
- Integration test proves: missing/invalid creds → `401`; `viewer` `GET` → `200`
  and `viewer` `POST` → `403`; `editor` `POST` → `200`; the upstream received the
  injected Basic auth, not the client's.

## Verification

- `cargo test -p orquestador-proxy`
- `cargo test --workspace`
- `cargo build -p orquestador-proxy`

## Notes / risks

- Method-based classification is sound for OpenCode's current contract; document
  the assumption.
- Streaming passthrough of `/event` is the fiddly part; test it.
- The proxy is the **trust boundary**: it must never forward client credentials
  upstream, and must never let an unauthenticated request reach OpenCode.

## Progress

- 2026-10-10: feature doc created; v0 implementation delegated.
- 2026-10-10: **T1 / T2 / T3 done**. `crates/orquestador-proxy` implements the
  role gate, Basic-auth resolution (constant-time compare), upstream credential
  injection, SSE passthrough, audit log, CLI, README, and the ROADMAP pointer.
  Verification: `cargo test -p orquestador-proxy` → 30 unit + 5 integration
  green; `cargo test --workspace` → core 73 + proxy, all green;
  `cargo build -p orquestador-proxy` → binary produced.
- Integration test proves: missing / invalid / unknown credentials → `401` and
  never reach upstream; `viewer` GET → `200` and `viewer` POST → `403` (the
  rejected write never reaches upstream); `editor` POST → `200`; the upstream
  received the injected Basic auth and never the client's; `/event` returns
  `text/event-stream` with the body passed through.
- Follow-up (not v0): TLS termination / deployment UX; stronger timing proof for
  SSE (currently asserted by content-type + body relay, not timing).
- RDD status (2026-10-10): `gentle-ai review assess --cwd <repo> --agent opencode
  --base-ref 41841e5 --committed-only` → risk **high** (hot path `auth.rs`),
  `review_due=true` (`high_risk`), 12 paths / 1420 lines. Review **deferred to
  delivery** per the user's instruction; no review was started for this slice.

## Commit

- `9fea838` `feat(proxy): add role-enforcing reverse proxy with viewer/editor RBAC`
  (branch `feat/role-proxy`, stacked on `feat/agent-backend`).
