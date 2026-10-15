# Cowork end-to-end setup — from project to connected teammate

This is the **walkthrough**: one ordered path from an empty machine to a second
person working in the same OpenCode project from their own app. Cowork lets a
small group share a single `opencode serve` instance — everyone sees the same
sessions, each person is identified and attributed, and each person is limited by
a role. The trust boundary is a small reverse proxy (`orquestador-proxy`) the
host runs in front of `opencode serve`: the app is always a client, the proxy
authenticates and enforces roles, and `opencode serve` is the thing being shared.

> This doc is the **step-by-step guide**. For the reference material — the full
> config table, presence internals, the project pin, the safety checklist and the
> audit format — see [`COWORK.md`](COWORK.md). To install the app and the CLI,
> see [`INSTALL.md`](INSTALL.md).

The steps are ordered; each assumes the previous one. Holes in the flow are
called out where they occur and collected at the end under
[Not built (gaps)](#not-built-gaps) — Cowork does not do everything.

## Step 0 — The project

A **project is a directory**: an absolute path on the host that `opencode serve`
knows about and serves. There is no separate project record in Orquestador —
identity is the directory's `worktree` path.

Be clear about what the app can and cannot do here:

- **The app cannot create a project.** There is no `create_project` command; the
  available commands are `list_projects`, `current_project` and
  `set_project_directory`.
- **The OpenCode server exposes no `POST /project`.** The client only has
  `GET /project` and `GET /project/current`. Registering a project belongs to
  `opencode serve`, not the app.

What the app *does* is **select** which project it is scoped to. Choosing one
calls `set_project_directory(directory)`, which persists the absolute path into
`server.json` (`directory`) and sends it on every request as the
`x-opencode-directory` header. Scoping is per client: two people can point at
different directories of the same server, unless the host pins one (see the pin
in [`COWORK.md`](COWORK.md#project-containment-the-pin)).

**What to actually do:**

1. Have the directory on the host (create it normally, e.g. `git clone`).
2. Run the server against it (Step 1).
3. Pick it in the app — the project picker lists what `GET /project` returns.

## Step 1 — The server (local or remote)

`opencode serve` is the server everyone shares, and the app **never starts it** —
you run it yourself. Start it with its own password:

```bash
OPENCODE_SERVER_PASSWORD=<upstream-password> \
  opencode serve --hostname 127.0.0.1 --port 4096
```

The address the app expects by default is `http://127.0.0.1:4096`.

**Local vs remote.** There is no cloud provider and no built-in tunnel. "Remote"
just means the server is reachable at some address — same binary, different
`base_url`:

- **Local:** the server and the app are on one machine, `127.0.0.1:4096`.
- **Remote:** run `opencode serve` on another host (or point the app's `base_url`
  at a reachable address) and carry the traffic over **Tailscale / WireGuard / an
  SSH tunnel**.

The proxy does **not** terminate TLS, so never expose the proxy or the upstream
directly on an untrusted network. The app can also connect to a remote server
with no proxy at all ("bring your own server"), but roles, presence and the pin
require the proxy (Step 3).

## Step 2 — Users and passwords

Add each collaborator to the whitelist with the `adduser` subcommand. The
password is read from **stdin** (never an argument), so it does not land in your
shell history:

```bash
./orquestador-proxy adduser <name> --role <editor|viewer> [--config <path>]

# example
printf '%s\n' 'their-password' | \
  ./orquestador-proxy adduser alice --role editor --config ./proxy.json
```

- `--role` is required and must be exactly `editor` or `viewer`.
- `--config` chooses the file to edit; without it, resolution is
  `ORQUESTADOR_PROXY_CONFIG`, then `./proxy.json`.

The password is hashed with **Argon2id** (default parameters; a random salt is
embedded in the resulting PHC string). On disk a user record looks like:

```json
{ "name": "alice", "password_hash": "$argon2id$...", "role": "editor" }
```

An older record may carry a plaintext `token` instead of `password_hash`.
**Precedence matters:** if a record has **both**, the hash wins and the plaintext
`token` **stops authenticating**. A record with both keys is a half-finished
migration, not two ways in. Prefer `password_hash` — the file holds a secret and
is the host's to protect.

## Step 3 — The proxy

`opencode serve` authenticates with **one shared password** and has **no RBAC**:
anyone who can reach it can also execute. The proxy adds the missing identity and
roles without forking OpenCode. It is the trust boundary, and every client in a
Cowork session connects to **the proxy**, never straight to the server.

Run it (serve is the default mode):

```bash
ORQUESTADOR_UPSTREAM_PASSWORD=<upstream-password> \
  ./orquestador-proxy ./proxy.json
```

Config resolution for serve: **first positional argument → `ORQUESTADOR_PROXY_CONFIG`
→ `./proxy.json`**. (For `adduser`: `--config` → `ORQUESTADOR_PROXY_CONFIG` →
`./proxy.json`.) The proxy has **no help flag** — any first argument that is not
`adduser` is treated as a config path.

A minimal, real `proxy.json`:

```json
{
  "listen": "127.0.0.1:4097",
  "upstream": "http://127.0.0.1:4096",
  "upstream_username": "opencode",
  "project_directory": "/home/host/team-project",
  "users": [
    { "name": "alice", "password_hash": "$argon2id$...", "role": "editor" },
    { "name": "bob",   "password_hash": "$argon2id$...", "role": "viewer" }
  ]
}
```

`listen`, `upstream`, `upstream_username` and `users[]` are required;
`upstream_password` and `project_directory` are optional. The proxy **validates
on load** and exits non-zero with a stderr message on an empty field, a relative
pin, a duplicate name, or a user with no credential. For the full field-by-field
table and the optional pin, see
[`COWORK.md`](COWORK.md#configuration-reference) — this doc shows only the fields
a basic setup needs.

**The two passwords do not have the same name.** This is the most common setup
mistake — setting one and expecting the other to apply:

| Variable | Direction | Meaning |
|---|---|---|
| `OPENCODE_SERVER_PASSWORD` | app → `opencode serve` | The server's own password; also the value the host starts `opencode serve` with. |
| `ORQUESTADOR_UPSTREAM_PASSWORD` | proxy → `opencode serve` | The password the proxy injects upstream. Overrides `proxy.json`'s `upstream_password`; env wins over the config field. |

If neither is set, the proxy prints `warning: no upstream password set` and
injects an empty upstream password.

## Step 4 — The whitelist (correct the mental model)

**The "whitelist" is the `users` array in `proxy.json`.** It is not a path or
endpoint allowlist — there is no such thing here. Authorization is two rules:

1. **Deny by default.** A request without known credentials gets `401` plus a
   `WWW-Authenticate` challenge. Being on the network is not being allowed.
2. **Method-based by role.** Once authenticated, the role decides by HTTP method.
   There is **no endpoint allowlist** and no per-route permission table.

| Role | May issue | In practice |
|---|---|---|
| `viewer` | `GET` / `HEAD` / `OPTIONS` | Reads everything, writes nothing (still watches the live `/event` stream) |
| `editor` | every method | Anything the upstream allows — including executing the agent |

So a `viewer` can read every session and file the project exposes but cannot
create, send, rename or delete anything; an `editor` can do anything the server
itself can. The one reserved namespace is **`/orquestador/*`**, used by the
proxy's own routes (`GET /orquestador/whoami`, `GET /orquestador/present`):
anything under that prefix is never forwarded upstream and answers `404`.

## Step 5 — Teammates

"Teammate" is **not** a code concept — it is a whitelisted user, i.e. a row in
`proxy.json`'s `users` array. Adding or removing one means editing that array:

- **CLI:** `adduser` adds a user (Step 2). To remove one, delete their entry from
  `proxy.json` — the file is the source of truth.
- **Host app — Members dialog:** the host's app can list, add and remove members
  in `proxy.json` directly (it preserves unknown fields and never sees
  credentials). Point it at the file once via Settings.

**Choosing the role:** grant `viewer` to anyone you would not give a shell.
`editor` is a full collaborator who can run the agent on the host — treat it as
shell access, because that is effectively what it is.

**Presence:** connected members appear in a presence strip in the top bar. A
member counts as present for **45 s** after their last request, and the app polls
every **15 s** (the poll is the heartbeat). Against a plain `opencode serve`
without the proxy, the presence routes `404` and the app shows no strip instead
of an error.

## Step 6 — Sessions

Sessions are conversations with the agent, and they live on the **OpenCode
server**, not in the app — so everyone scoped to the same project sees the same
sessions. The app creates one with its new-session action, which calls
`POST /session` with an optional `{ parentID?, title? }` body. The rest of the
lifecycle runs through the same server API:

- **List** — `GET /session`
- **Status map** — `GET /session/status`
- **Children (sub-agents)** — `GET /session/:id/children`
- **Rename** — `PATCH /session/:id`
- **Delete** — `DELETE /session/:id`

When a member creates a session, its title is tagged `[<member>] <title>` so
attribution survives a flat shared list. On bootstrap the app ensures a **root
session** exists; if none does, it creates one with the default title
`Orchestrator`.

## Step 7 — What a teammate does to connect

Everything above is host work. This is the handoff: what the other person does
on their own machine.

1. **Open the join form.** On first run the Welcome screen offers it; it is also
   reachable any time from **Settings → Join a workspace…**.

2. **Fill in the fields:**

   - **Server address** — the **proxy's** address, e.g. `http://127.0.0.1:4097`,
     or the tunnel address if the proxy is remote.
   - **Username** — their `name` from `proxy.json`.
   - **Password** — their own password (the one the host hashed with `adduser`).
   - **Display name** (optional) — shown on the sessions they create; defaults to
     their username.

   The address is the **proxy's** `listen` address, **not** the upstream
   `opencode serve` port. Pointing a teammate at the upstream skips the proxy and
   loses roles, presence and the pin.

3. **Join.** The app persists the settings in `server.json` (`base_url`,
   `username`, `password`, `directory`, `member_name`, plus the host-only
   `proxy_config_path`). On later starts, resolution is **`server.json` → env →
   defaults**.

4. **Verify.** The presence strip shows everyone connected. A teammate with
   `viewer` can read but gets `403` on any write — that is the role working, not
   a bug.

**When it fails:**

| Symptom | Cause |
|---|---|
| `401` | Wrong username/password, or the user is not in `proxy.json` |
| `403` on a write | The user's role is `viewer` |
| "Can't reach `opencode serve`" banner | The server (or the proxy in front of it) is unreachable at the configured address |

**What a `viewer` can and cannot do:** they can browse projects, list and read
sessions, watch the live event stream and read files — everything read-only. They
cannot create or send messages, create/rename/delete sessions, edit files or run
commands; each of those is a non-`GET` method and is denied at the proxy before
it reaches the server.

## Not built (gaps)

These are real holes in the v0 flow, not configuration you are missing:

- **No project creation in the app.** Projects come from `opencode serve`; the
  app only lists and selects them (Step 0).
- **No TLS termination in the proxy.** Terminate TLS in front of it (Caddy,
  nginx) or carry traffic over a tunnel/VPN (Step 1).
- **No cloud provider and no built-in tunnel.** "Remote" is your own reachable
  address — Tailscale, WireGuard or an SSH tunnel.
- **No session ownership per user.** Attribution is the `[member]` title tag and
  is informational; it is not access control. Anyone scoped to the project sees
  every session.
- **No rate limiting.**
- **No host UI to set `project_directory`.** Edit `proxy.json` directly; the
  Members dialog preserves the pin because it edits the raw document.

See [`COWORK.md`](COWORK.md#not-built-v0-limits) for the reference copy of this
list, and the
[`orquestador-proxy` README](../crates/orquestador-proxy/README.md) for the
proxy's own reference.
