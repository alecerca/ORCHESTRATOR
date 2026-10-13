# orquestador-proxy

A small, standalone **role-enforcing reverse proxy** that sits in front of a
shared `opencode serve` instance. It exists because OpenCode authenticates with a
**single shared password** and has **no RBAC**: anyone who can reach the server
can also execute (send prompts, answer permissions, mutate sessions).

The proxy lets remote clients join a shared project as either:

- **`viewer`** — read-only. Safe methods only (`GET` / `HEAD` / `OPTIONS`),
  which includes the `/event` SSE stream. Every write method is rejected with
  `403`.
- **`editor`** — full control (all methods).

It has **no dependency** on `orquestador-core` or `src-tauri` and compiles
headless (no Tauri, no webview).

## ⚠️ Security model — read this first

> [!WARNING]
> **This proxy does NOT terminate TLS.** v0 is designed to run on a **private
> network only** — a Tailscale tailnet, a WireGuard mesh, or an SSH tunnel
> (`ssh -L 4097:127.0.0.1:4097 host`).
>
> **Never expose the proxy — or `opencode serve` — plainly on `0.0.0.0` over an
> untrusted network.** A remote agent runs commands by design; a plaintext,
> unauthenticated path to it is a remote-code-execution surface. Use a tunnel or
> terminate TLS in front of the proxy (e.g. Caddy/nginx), and bind the proxy to
> `127.0.0.1` or the tunnel interface.

The proxy is the **trust boundary**:

- It never forwards a client's credentials upstream; it injects the real
  upstream Basic auth itself.
- It never lets an unauthenticated request reach OpenCode.
- Token comparison is **constant-time** (no early-exit `==` on secrets).

## Build

```bash
cargo build -p orquestador-proxy --release
# binary at target/release/orquestador-proxy
```

## Run

The proxy is run by the **host**, in front of `opencode serve`. Clients point
their Settings `base_url` at the proxy.

```bash
# 1. Run the real server with its own password.
OPENCODE_SERVER_PASSWORD=<upstream-password> \
  opencode serve --hostname 127.0.0.1 --port 4096

# 2. Run the proxy in front of it.
ORQUESTADOR_UPSTREAM_PASSWORD=<upstream-password> \
  ./target/release/orquestador-proxy /path/to/proxy.json
```

Config path resolution (first match wins):

1. CLI argument: `orquestador-proxy /path/to/proxy.json`
2. Environment: `ORQUESTADOR_PROXY_CONFIG=/path/to/proxy.json`
3. Default: `./proxy.json`

## Config reference (`proxy.json`)

```json
{
  "listen": "127.0.0.1:4097",
  "upstream": "http://127.0.0.1:4096",
  "upstream_username": "opencode",
  "upstream_password": "optional-fallback",
  "project_directory": "/home/owner/projects/team-app",
  "users": [
    { "name": "alice", "token": "change-me-1", "role": "editor" },
    { "name": "bob",   "token": "change-me-2", "role": "viewer" }
  ]
}
```

| Field | Required | Meaning |
|---|---|---|
| `listen` | yes | Address the proxy binds, e.g. `127.0.0.1:4097`. |
| `upstream` | yes | Base URL of the real `opencode serve`, e.g. `http://127.0.0.1:4096`. |
| `upstream_username` | yes | Username for the injected upstream Basic auth. |
| `upstream_password` | no | Fallback upstream password. `ORQUESTADOR_UPSTREAM_PASSWORD` **wins** over this field. |
| `project_directory` | no | Optional absolute path that pins the whole shared instance to one project directory (see below). Absent = unrestricted. |
| `users[].name` | yes | Client username; must be unique. |
| `users[].token` | yes | Client token (the password half of Basic auth). |
| `users[].role` | yes | `viewer` or `editor`. |

Config is validated on load: non-empty `listen`/`upstream`/`upstream_username`,
at least one user, unique names, non-empty tokens, and a non-empty **absolute**
`project_directory` when it is set. Invalid configs exit non-zero with a message
on stderr.

### Project containment (`project_directory`)

By default the proxy is **unrestricted**: it forwards the client's
`x-opencode-directory` header verbatim, so a whitelisted client can point the
shared server at any project on the host.

Set the optional `project_directory` to pin the **whole shared instance** to a
single absolute project directory. When it is set, the proxy strips whatever
directory the client sends and injects the pinned one, and `GET /project` is
filtered to that project alone — so no client can leave the pinned project or
even see the host's others. This is the containment boundary for a shared
instance; omit it (the default) to keep today's unrestricted behaviour.

> [!NOTE]
> It narrows **which directory is the working project**; it is not a sandbox.
> The agent still runs with the host user's privileges, so split genuinely
> untrusted work across OS users, containers, or VMs, or grant `viewer` only.

### Client setup

A client reuses its existing `username` + `password` fields — **no frontend
change required**. Set:

- `base_url` → the proxy's address (e.g. `http://host:4097`)
- `username` → the user's `name` from `proxy.json`
- `password` → the user's `token` from `proxy.json`

The client sends `Authorization: Basic base64(name:token)`; the proxy resolves
`name -> {token, role}` and injects the real upstream credentials.

## Audit log

The proxy writes **one line per request to stderr**:

```
user=<name|-> role=<role|-> method=<M> path=<p> status=<code> <allowed|denied|unauthenticated>
```

Examples:

```
user=bob role=viewer method=GET path=/event status=200 allowed
user=bob role=viewer method=POST path=/session status=403 denied
user=- role=- method=GET path=/session status=401 unauthenticated
```

Redirect stderr to a file or your log collector to keep a trail.

## Behaviour notes

- **Method-based classification.** OpenCode's read endpoints are all `GET`
  (including `/event`); writes are `POST`/`PATCH`/`PUT`/`DELETE`. This is the v0
  assumption — revisit if an upstream `GET` ever gains side effects.
- **Streaming.** Responses are streamed back as they arrive; the `/event` SSE
  stream is not buffered.
- **Project pin.** With `project_directory` set, the client's
  `x-opencode-directory` is replaced by the pinned value and `GET /project` is
  filtered to it; without it, the header passes through verbatim.
- **Hop-by-hop stripping.** `Authorization` (inbound), `host`,
  `content-length`, `transfer-encoding`, `connection`, `keep-alive`, `te`,
  `upgrade`, and `proxy-*` are stripped; the proxy adds upstream Basic auth.

## Out of scope (v0)

TLS termination in the proxy; per-user model / API-key routing; session
ownership; message persistence; dynamic token provisioning; rate limiting.

## Test

```bash
cargo test -p orquestador-proxy
```
