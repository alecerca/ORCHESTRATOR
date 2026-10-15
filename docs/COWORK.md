# Cowork — several people on one shared project

Cowork lets a group work in the **same** OpenCode project from their own apps:
everyone sees the same sessions, each person is attributed, and each person is
limited by a **role**. The app stays a client; the enforcement lives in a small
reverse proxy the host runs in front of `opencode serve`.

The reason it exists: `opencode serve` authenticates with **one shared password**
and has **no RBAC**. Anyone who can reach it can also execute. Cowork adds the
missing identity, roles and project scoping **without forking OpenCode**.

> **One sentence:** the proxy is the trust boundary — the app is the client, and
> `opencode serve` is the thing being shared.

## Quick path

**Host** (the machine that runs the project):

```bash
# 1. The real server, with its own password.
OPENCODE_SERVER_PASSWORD=<upstream-password> \
  opencode serve --hostname 127.0.0.1 --port 4096

# 2. A config (start from the example below).
#    The file does not exist yet: create it next to where you will run the
#    proxy, and paste the example from "Configuration reference" below.

# 3. Add each collaborator; the password is read from stdin and hashed.
cargo build -p orquestador-proxy --release
printf '%s\n' '<their-password>' | \
  ./target/release/orquestador-proxy adduser alice --role editor --config ./proxy.json

# 4. Run the proxy in front of the server.
ORQUESTADOR_UPSTREAM_PASSWORD=<upstream-password> \
  ./target/release/orquestador-proxy ./proxy.json
```

**Client** (each collaborator, in the app): open **Join a workspace** (or
Settings) and set `base_url` to the proxy, `username` to their `name` in
`proxy.json`, and `password` to their own password. Nothing else changes — the
proxy resolves them and injects the real upstream credentials.

**Verify:** the top bar shows a presence strip with everyone connected, and a
`viewer` gets `403` on any write.

## Who does what

| Piece | Role |
|---|---|
| `opencode serve` | Hosts the project and the agent. Not safe to expose directly. |
| `orquestador-proxy` | **Trust boundary**: authenticates clients, enforces roles, pins the project, audits every request. |
| The app | Client for everyone; host tooling (the whitelist editor) for whoever has the config. |

## Details

### Credentials

| Field | Meaning |
|---|---|
| `password_hash` | Argon2id (PHC string). Written by `adduser` or the app's **Members** dialog. |
| `token` | Legacy plaintext password. A record needs one of the two: when `token` is used **alone** it still authenticates. If a record has **both** `password_hash` and `token`, the **hash wins** and the plaintext token stops authenticating (`crates/orquestador-proxy/src/auth.rs`). |

Prefer `password_hash`. A user with `token` keeps a **plaintext secret in the
file** — that file is the host's to protect. Hashes are never logged, and
password comparison is constant-time.

### Roles

| Role | Allowed | Typical use |
|---|---|---|
| `viewer` | `GET` / `HEAD` / `OPTIONS` — including the `/event` SSE stream, so they still watch live | Someone who should read but not act |
| `editor` | Every method | A full collaborator |

The check is **method-based** and runs **only in the proxy**. Hiding a button is
never the control: a `viewer` who forges a request still gets `403`.

### Presence

The proxy tracks who is connected and exposes it on two routes:

| Route | Returns |
|---|---|
| `GET /orquestador/whoami` | The caller's own `name`, `role`, and the pinned `project` (or `null`) |
| `GET /orquestador/present` | Everyone currently present |

A member counts as present for **45 s** after their last request, and the app
polls every **15 s** — the poll itself is the heartbeat. Both routes are
feature-detected: against a plain `opencode serve` (no proxy) they `404` and the
app shows no strip instead of an error.

### Project containment (the pin)

By default the proxy is **unrestricted**: it forwards the client's
`x-opencode-directory` header verbatim, so a whitelisted `editor` can point the
shared server at **any** project on the host.

Set the optional top-level `project_directory` to pin the whole shared instance
to one absolute path. With it set, the proxy drops whatever directory a client
sends, injects the pinned one, filters `GET /project` to that project alone, and
the app's project picker shows that project **locked**. Without it, the request
path is unchanged from before.

> [!NOTE]
> The pin decides **which project is the working project**; it is not a sandbox.
> The agent still runs with the host user's privileges. For genuinely untrusted
> work, use a separate OS user, container or VM — or grant `viewer` only.

### Working together

Cowork is **read-shared, write-independent**: everyone sees every session, and
each person owns the sessions they create (titles are tagged `[member] …` so
attribution survives a flat list). There are **no file locks** — if two people
edit the same file, the last write wins, exactly like a shared working copy in
git. Agree on who owns what.

## Configuration reference

```json
{
  "listen": "127.0.0.1:4097",
  "upstream": "http://127.0.0.1:4096",
  "upstream_username": "opencode",
  "upstream_password": "<upstream-password>",
  "project_directory": "/home/host/team-project",
  "users": [
    { "name": "alice", "password_hash": "$argon2id$...", "role": "editor" },
    { "name": "bob",   "token": "legacy-plaintext",    "role": "viewer" }
  ]
}
```

| Key | Required | Notes |
|---|---|---|
| `listen` | yes | Address the proxy binds. Keep it on `127.0.0.1` or a tunnel interface. |
| `upstream` | yes | The real server, e.g. `http://127.0.0.1:4096`. |
| `upstream_username` | yes | Username for the injected upstream Basic auth. |
| `upstream_password` | no | Fallback only — `ORQUESTADOR_UPSTREAM_PASSWORD` wins. |
| `project_directory` | no | Absolute path; pins the instance. Absent = unrestricted. |
| `users[]` | yes | At least one; unique names; each needs `password_hash` or `token` plus a `role`. |

The proxy **validates on load** and exits non-zero with a message on stderr if
something is wrong (empty fields, a relative pin, a duplicate name, a user with
neither credential). Config path: CLI argument → `ORQUESTADOR_PROXY_CONFIG` →
`./proxy.json`.

The full audit trail is **one line per request on stderr**:

```
user=bob role=viewer method=GET path=/event status=200 allowed
user=bob role=viewer method=POST path=/session status=403 denied
user=- role=- method=GET path=/session status=401 unauthenticated
```

Redirect stderr to a file or your log collector.

## Operating it safely — checklist

- [ ] The proxy **does not terminate TLS**. Use a Tailscale tailnet, WireGuard,
      or an SSH tunnel (`ssh -L 4097:127.0.0.1:4097 host`).
- [ ] **Never expose the proxy or `opencode serve` on `0.0.0.0` over an
      untrusted network.** A remote agent runs commands by design.
- [ ] Bind the proxy to `127.0.0.1` or the tunnel interface; terminate TLS in
      front of it (Caddy/nginx) if clients must reach it over a network.
- [ ] Set `project_directory` for any shared instance.
- [ ] Prefer `password_hash` over `token`; delete unused users.
- [ ] Grant `viewer` to anyone you would not give a shell.
- [ ] Keep the audit log.

## How this was built

Five phases on the `feat/cowork` branch, merged to `main` in one go. Every phase
was implemented by a delegated worker under a design doc
([`../odd/tasks/cowork.md`](../odd/tasks/cowork.md)), and the parent reviewed the
diffs before committing.

| Phase | What it added | Commits |
|---|---|---|
| 1 — Identity | `member_name` through core → Tauri → UI, `identity.ts`, `OwnerChip`, `[member] title` attribution | `e241075` |
| 2 — Credentials | Optional `password_hash` / `token`, fail-closed resolver, `adduser` CLI with Argon2id | `dba46ff` |
| 3 — Whitelist UX | Host-side `proxy.json` editor (`list`/`add`/`remove`, preserves unknown fields), Members dialog, join flow | `22c6076` `f1fb0a3` `bae0b84`, docs `3aee063` |
| 4 — Presence | `/orquestador/whoami` + `/orquestador/present`, 45 s window, `PresenceStrip` polling 15 s | design `0e31875`; `ecdde5d` `eb58b43` `fc6a7b3`, docs `cc0ce71` |
| 5 — Containment | `project_directory` pin, `GET /project` filtered, `whoami.project`, locked project picker | `72e803d` `0302753`, docs `bf0fc9a` `4923738` `97bba06` |

Two design choices worth knowing:

- **The proxy owns the security model, the app owns the ergonomics.** Whitelist
  parsing, roles and the pin live in a standalone crate with no Tauri or core
  dependency, so the boundary can be tested and reasoned about on its own.
- **The pin (Option B) was built before per-user project scoping (Option A).**
  Pinning one project per shared instance is safety by construction — no
  per-user bookkeeping, no path comparison — and it matches the ordinary
  "cowork on one project" case. Option A stays the design for when different
  people need different projects.

Verification actually run at merge time:

```bash
cargo test --workspace                              # core 111, proxy 56 + 18
cargo test --manifest-path src-tauri/Cargo.toml --lib   # 18
npm run typecheck                                   # from frontend/ — exit 0
```

Not verified automatically: the presence strip and the locked project picker are
**typecheck-only** — the frontend has no test runner, so both need a manual smoke
with two clients.

## Troubleshooting

| Symptom | Cause |
|---|---|
| `401` from the proxy | Wrong `username`/`password`, or the user is not in `proxy.json`. |
| `403` on a write | The user's role is `viewer`. |
| Presence strip missing | The app talks straight to `opencode serve`: the `/orquestador/*` routes `404`. That is a normal feature-off state. |
| The project picker is locked | The host set `project_directory`. It is not a bug — the workspace is pinned. |
| A collaborator sees a project they did not choose | Same pin. |
| Proxy exits at startup | Invalid config: the message on stderr names the field. |
| `warning: no upstream password set` | Neither `ORQUESTADOR_UPSTREAM_PASSWORD` nor `upstream_password` is set; upstream auth will use an empty password. |

## Not built (v0 limits)

No per-user model or API-key routing, no session ownership, no message
persistence, no dynamic token provisioning, no rate limiting, and no TLS
termination in the proxy. There is also no host UI to set `project_directory` —
edit `proxy.json` (the Members dialog preserves it because it edits the raw
document).

## Next step

Read [`../odd/tasks/cowork.md`](../odd/tasks/cowork.md) for the full design,
acceptance criteria and open decisions, and
[`../crates/orquestador-proxy/README.md`](../crates/orquestador-proxy/README.md)
for the proxy's own reference.
