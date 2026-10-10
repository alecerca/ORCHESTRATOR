# Orquestador — Roadmap & Future Architecture

> Forward-looking companion to `docs/ARCHITECTURE.md` (current design) and
> `docs/PLAN.md` (v1 contract). `PLAN.md` remains the single source of truth for
> v1; this document is **non-authoritative post-v1 planning**. It maps three
> product directions that are feasible but **not part of v1**: multi-platform
> distribution, collaborative/remote ("cowork") use with roles, and pluggable AI
> backends beyond OpenCode. Nothing here is implemented yet.

## TL;DR

| Track | Verdict | Biggest gap |
|---|---|---|
| 1. Multi-platform desktop (Windows / macOS / Linux) | Feasible now (Tauri 2) | Per-OS build matrix + code signing; WebKitGTK / notification variance |
| 2. Cowork — connect to a remote address; share a project; read-only role | Feasible, mostly wiring | OpenCode has no roles → need an owned role-enforcing proxy |
| 3. Multi-backend (OpenCode / Claude Code / Codex) | Feasible | Needs an `AgentBackend` trait + per-backend event adapters |

## Baseline (what exists today)

- **Tauri 2 desktop app** (native webview per OS), Rust shell + React frontend.
  Bundling is declared in `src-tauri/tauri.conf.json` → `bundle.targets: "all"`.
- **Headless core** `crates/orquestador-core` (no Tauri dependency, unit-tested).
- **Already a network client**: it drives `opencode serve` over HTTP + SSE.
  - `crates/orquestador-core/src/client.rs` — typed `reqwest`/`rustls` client;
    `DEFAULT_BASE_URL = http://127.0.0.1:4096` (`client.rs:20`); HTTP basic auth.
  - Config from the environment: `OPENCODE_SERVER_BASE_URL`,
    `OPENCODE_SERVER_PASSWORD`, `OPENCODE_SERVER_USERNAME` (`client.rs:99`).
  - The app does **not** spawn OpenCode; it connects to a server that runs
    separately.

That last point is why Tracks 2 and 3 are close.

## Track 1 — Multi-platform desktop

**Verdict:** feasible — it is what Tauri is for. One Rust core, a native webview
per OS (WebView2 / WKWebView / WebKitGTK). It is a desktop app, **not** a browser
app.

**What already holds:**

- Tauri 2 + `bundle.targets: "all"` already declares desktop bundles.
- Core dependencies are portable: `reqwest` + `rustls` (no OpenSSL), `notify-rust`.

**Work / risks:**

| Area | Notes |
|---|---|
| Build matrix | `targets: "all"` builds all formats **for the host OS only**. Ship three platforms via CI (windows / macos / ubuntu). Tauri cross-compilation is painful — build on each OS (or use Tauri's CI action). |
| Linux webview | Behavior and version vary by distro (already surfaced with GNOME 50). `webkit2gtk-4.1` is Linux-only — that is why `src-tauri` sets an empty `[workspace]` table. |
| Notifications | `notify-rust` maps to Windows toast / macOS / Linux D-Bus, but the UX differs per OS — test each. |
| Signing | macOS: Developer ID + notarization (else Gatekeeper blocks). Windows: code-signing cert (SmartScreen). Linux: AppImage/deb/rpm, unsigned is normal. |
| Commercialization | Tauri is MIT/Apache → closed-source and paid distribution are allowed. The crate is MIT. The real constraint is the AI **backends'** licenses (Track 3), not Tauri. |

## Track 2 — Cowork (remote + roles)

**Verdict:** feasible, and largely wiring — the client is already network-based.

**How it maps to today's code:**

- "Type the address to point at" == expose `base_url` + `password` in a Settings
  screen instead of the hardcoded default (`client.rs:20`).
- **Host** runs: `opencode serve --hostname 0.0.0.0 --port 4096` with
  `OPENCODE_SERVER_PASSWORD` set. Verified: OpenCode supports `--hostname`,
  `--port`, and HTTP basic auth natively. Clients then connect to
  `http://<host>:4096`.
- Multiple clients attached to one server see the **same sessions** (shared SSE
  state) → that is already shared work on one project.

**The real gap — read-only / roles:**

- OpenCode auth is a **single shared password**: anyone who authenticates can
  also execute (send prompts, answer permissions). There is **no RBAC**.
- "Connect but cannot execute" therefore requires a **role-enforcing proxy owned
  by Orquestador**, placed in front of OpenCode: authenticate per user, allow the
  read endpoints + `/event` for a *viewer*, and block write endpoints
  (prompt / permission / session mutations).

**Security (non-negotiable):**

- A remote agent **runs commands by design** → this is a remote-code-execution
  surface. Require TLS (or a Tailscale/SSH tunnel), per-user tokens, and an audit
  trail. Never expose a passwordless server on `0.0.0.0`.

**Why this is also the business model:** "headless host + connecting clients" is
the self-hosted / SaaS shape. The core is already headless, so a standalone
"Orquestador server" binary wrapping OpenCode + the proxy is a natural step.

## Track 3 — Multi-backend (OpenCode / Claude Code / Codex)

**Verdict:** feasible, but requires a real abstraction — not plug-and-play.

**Why it is close:** today the app depends only on OpenCode's **documented HTTP
contract** (`/session`, `/event`, `/permission`, …), not its internals. That is a
clean seam to abstract.

**Design:** an `AgentBackend` trait (`create_session`, `prompt`,
`stream_events`, `respond_permission`, …) with one implementation per backend.

| Backend | Integration shape | Status |
|---|---|---|
| OpenCode | HTTP + SSE (exists today) | implemented |
| Claude Code | Agent SDK (Python/TS, ships a native binary) or subprocess `claude -p --output-format json` | external, verified |
| Codex | subprocess `codex exec --json` (JSONL event stream) | external, verified |

**The real work:** each backend emits a **different event protocol** (SSE vs
stream-json vs JSONL). The cost is a per-backend adapter that normalizes those
events into `CoreMessage` / `AppState`. OpenCode hands you a full HTTP server;
Claude Code and Codex are **CLI-first**, so they are spawned and their stdout is
parsed.

**Licensing:** Claude Code and Codex are **closed-source** → depend on the user
having them installed and authenticated; do not bundle them. OpenCode is OSS.

## Suggested delivery order

1. **Settings: `base_url` + `password`** (Track 2, easy half) → enables
   "connect to another machine" immediately.
2. **`AgentBackend` trait** (Track 3) → multi-model, and it forces the client
   seam to be clean.
3. **Role-enforcing proxy + TLS** (Track 2, hard half) → real cowork with
   read-only.
4. **Build matrix + signing** (Track 1) → ship the three platforms.

## Out of scope / open questions

- No RBAC in OpenCode today — confirm whether upstream plans it (that would
  remove the need for the proxy).
- Session ownership / who-owns-the-project model in shared mode.
- Per-user model / API-key handling when one server serves many users.
- Data at rest: messages/parts are not persisted today; collaboration may need it.

## Verification (per track, when it starts)

- [ ] Track 1: a signed build launches on Windows, macOS, and a non-Fedora Linux.
- [ ] Track 2: two clients connect to one host and see the same sessions; a
      viewer token is rejected on every write endpoint.
- [ ] Track 3: the same UI flow drives OpenCode and one alternate backend with
      no UI-specific branching.
