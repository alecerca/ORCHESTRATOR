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
| 3. Multi-backend (OpenCode / Claude Code / Codex) | Feasible | Needs an `AgentBackend` trait + per-backend event adapters; protocol research done (Track 3 + Appendix A) |
| 4. Onboarding + backend selector | Design-only for now | Connection model differs per backend (Server vs Process); ship OpenCode-only in v1 |

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
Protocol research was completed on 2026-10-12; full detail, flags, and sources
live in **Appendix A**.

**Why it is close:** today the app depends only on OpenCode's **documented HTTP
contract** (`/session`, `/event`, `/permission`, …), not its internals. That is a
clean seam to abstract.

**Design:** an `AgentBackend` trait (`create_session`, `prompt`,
`stream_events`, `respond_permission`, …) with one implementation per backend.

**Integration shapes (researched):**

| Backend | Integration shape | Wire protocol | Status |
|---|---|---|---|
| OpenCode | HTTP + SSE to a running `opencode serve` | documented HTTP contract | implemented |
| Codex | spawn `codex app-server`, speak JSON-RPC 2.0 over stdio (default); `codex exec --json` for one-shot JSONL | bidirectional JSON-RPC (app-server) | official, verified |
| Claude Code | **no server**; subprocess `claude --print --output-format stream-json --input-format stream-json`, or the Agent SDK streaming mode | bidirectional stdio control protocol — **input side undocumented** | external, risky |
| Any ACP agent | spawn an ACP adapter, JSON-RPC over stdio | Agent Client Protocol (Zed) | open standard, adapters immature |

**The real work:** each backend emits a **different event protocol** (SSE vs
stream-json vs JSONL vs ACP). The cost is a per-backend adapter that normalizes
those events into `CoreMessage` / `AppState`.

**Topology shift (important):** `opencode serve` is an HTTP **server you connect
to**; `codex app-server` and ACP are a **child process you spawn** and speak
JSON-RPC to over stdio. Orquestador already manages agent processes, so the
spawn-and-speak model fits — arguably better than the connect-to-a-server model.

**Two viable strategies:**

1. **Per-backend native adapter (Codex `app-server` first).** Lowest friction,
   officially maintained by OpenAI, UI-ready (streaming items + server-initiated
   approvals). One new agent per adapter; Claude Code still needs the risky stdio
   path.
2. **ACP as a unifying client.** Implement one ACP client in the core
   (`agent-client-protocol` Rust crate) and reach Codex, Claude Code, Gemini CLI,
   Goose, and OpenCode itself through a single protocol. But it depends on
   immature adapters.

These are not exclusive: ACP could be the base layer, with a native `codex
app-server` adapter where ACP falls short.

**Local environment finding (this dev machine, 2026-10-12):** only `opencode`
(`1.18.35`) is installed; `codex` and `claude` are **not** installed. Any new
backend requires installing the agent binary before the adapter can be verified
end-to-end.

**Licensing:** Claude Code and Codex are **closed-source** → depend on the user
having them installed and authenticated; do not bundle them. OpenCode is OSS;
ACP (`agent-client-protocol`) is Apache-2.0.

**Connection settings:** the base-URL / user / password UI built for Track 2 is
reusable, but the *transport* it drives is **OpenCode-specific**. See **Track 4**.

## Track 4 — Onboarding + backend selector (connection model)

**Verdict:** the pattern is right, but it should stay **design-only until a second
backend actually exists**. Shipping v1 with OpenCode alone is the correct call.

**The insight:** what changes per backend is not just the form fields — it is the
**connection model**. "IP + port + user + password" is *not* a generic agent
capability; it is **OpenCode-specific**, because OpenCode *is* an HTTP server. The
others are **processes**.

| Backend | What the user is asked | How the app connects |
|---|---|---|
| OpenCode | base URL + user + password | HTTP + SSE (exists today) |
| Codex | path to the binary + auth | spawn the process, JSON-RPC over stdio |
| Claude Code | binary + auth | spawn + stdio (or the Agent SDK) |
| Gateway / shim | base URL (+ auth) | HTTP + SSE (OpenCode contract, translated underneath) |

**Design (for when it is built):** a **backend registry** where each entry
declares:

- `id` + label;
- **mode**: `Server` (url + auth) or `Process` (binary + args + auth);
- a **field schema** the onboarding / Settings panel renders from;
- a **test probe** (the existing "Test connection");
- its `AgentBackend` implementation.

The first-run onboarding and the later Settings screen would be the **same
component**, driven by that schema: dropdown of backends → the selected backend's
fields → test → save. Adding a future CLI agent = writing one adapter; the panel
picks it up automatically.

**Recommended strategy — ship OpenCode-only, keep the door open:**

- **v1 ships with OpenCode only.** Do **not** build Codex/Claude adapters now
  (YAGNI).
- **Make the seam genuinely open** for contributors: replace the hardcoded
  `OpenCodeClient::new` in `bridge.rs:86` with a pluggable factory, and add a
  short "how to add a backend" doc.
- **Do not build the selector until there are ≥ 2 backends.** A dropdown with one
  option is ceremony; the current Settings (base URL / user / password) is enough
  today.

**Honesty note — the abstraction leaks today:** `AgentBackend`'s events are
OpenCode-shaped. `backend.rs` states that a non-OpenCode backend "must synthesize
a `raw` payload the UI understands", so a contributor must currently **fake
OpenCode's event shape**. A truly pluggable backend needs a normalized core event
model — that is the real, deferred cost (not the trait itself).

**Second honesty note — "anyone can add a backend" has a floor:** Orquestador is
Rust/Tauri, so a new backend is a **compiled** adapter submitted as a PR, not a
drop-in plugin. Frame the open-source invitation as "contributors can submit an
adapter", not "users can plug in anything".

## Suggested delivery order

1. **Settings: `base_url` + `password`** (Track 2, easy half) → enables
   "connect to another machine" immediately.
2. **`AgentBackend` trait** (Track 3) → multi-model, and it forces the client
   seam to be clean.
3. **Role-enforcing proxy + TLS** (Track 2, hard half) → real cowork with
   read-only. → **v0 implemented** (`crates/orquestador-proxy`: viewer/editor
   role gate over HTTP Basic, SSE passthrough, audit log). TLS termination is
   still a follow-up; v0 assumes a private network / tunnel.
4. **Build matrix + signing** (Track 1) → ship the three platforms.
5. **Onboarding + backend selector** (Track 4) → **only after a second backend
   exists**; until then ship OpenCode-only with the current Settings.

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
- [ ] Track 4: a new backend is added by writing one adapter (PR) and registering
      it, with the onboarding/Settings panel picking up its fields from the
      schema — no UI changes.

---

# Appendix A — Backend protocol research log (2026-10-12)

> Non-authoritative research notes feeding Track 3. Read-only investigation; no
> code was changed. Every claim below was checked against the primary sources
> listed at the end.

## A.1 Codex — `codex app-server` (there IS a real server)

Codex has a first-class, OpenAI-maintained **application server**: `codex
app-server`. It is the closest analogue to `opencode serve`, and richer than the
one-shot `codex exec`.

| Aspect | Detail |
|---|---|
| Command | `codex app-server` |
| Protocol | bidirectional **JSON-RPC 2.0**, with the `jsonrpc` header omitted on the wire — messages are **newline-delimited JSON (JSONL)**, one per line |
| Default transport | `stdio://` (also `--stdio` shorthand, added in CLI **v0.136 / June 2026**) |
| Other transports (`--listen`) | `ws://IP:PORT` (**experimental / unsupported**), unix socket, `off` |
| WebSocket auth flags | `--ws-auth capability-token --ws-token-file <abs>`, `--ws-token-sha256 <hex>`, `--ws-auth signed-bearer-token --ws-shared-secret-file <abs>`, `--ws-issuer`, `--ws-audience`, `--ws-max-clock-skew-se…` |
| WS default security | **non-loopback WS listeners are unauthenticated by default** during this rollout |
| Handshake | `initialize` request → `result`, then an `initialized` notification (required before any other call) |
| Thread API | `thread/start`, `thread/resume`, `thread/fork`, `thread/archive`, `thread/list` |
| Turn API | `turn/start`, `turn/steer` (steer a live turn) |
| Streaming | server pushes `item/*` notifications and `turn/completed` |
| Interactive | the server can make **client-bound requests mid-turn** (approvals / permission prompts) — the same shape Orquestador already renders for OpenCode |
| Schema | **version-specific**; generate with `codex app-server generate-ts` / `generate-json-schema` — **pin the CLI version** |
| Source | `openai/codex` repo, `codex-rs/app-server` crate (open source) |
| Consumers | Codex VS Code extension, Codex macOS app, Codex Web; OpenAI presents it as the first-class harness integration |
| SDKs | `@openai/codex-sdk` (TS), `openai-codex` (Python), community Go; for the Rust core, speak the protocol directly |
| Remote | `codex remote-control start|stop` daemon, then `codex --remote ws://…` from another machine |

**Caveats:** the schema must be generated from the installed binary and pinned per
version; WebSocket transport is explicitly experimental; remote WS is
unauthenticated by default; it requires the Codex binary on `PATH` plus auth
(`~/.codex/auth.json` or `OPENAI_API_KEY`).

**One-shot alternative:** `codex exec --json` emits a JSONL event stream for
non-interactive runs. Simpler, but it has no mid-turn interactive approvals, so it
is a poor fit for Orquestador's live control UI.

## A.2 Claude Code — no server, but a (partly undocumented) stdio protocol

Claude Code has **no server mode**. There is no `claude serve`. What exists:

- **Headless output:** `claude --print --output-format text|json|stream-json`
  (add `--verbose --include-partial-messages` for token-level streaming).
- **Streaming input:** `--input-format stream-json`, and the Agent SDK
  "streaming vs single mode" (persistent process, queued messages, interrupts,
  image uploads, permission requests, session management).
- **Control protocol:** bidirectional envelopes `control_request` /
  `control_response`, with `can_use_tool` permission requests and `set_model`.

**The problem:** the **input / control side is undocumented** for direct CLI use;
the community has had to reverse-engineer it (open issues in
`anthropics/claude-code`, e.g. #24594, #77632). The Agent SDK wraps this
officially, so the safer path is either the SDK or an ACP adapter rather than
hand-rolling the raw stdio protocol.

## A.3 ACP — Agent Client Protocol (the "something similar" that unifies)

**ACP** is an open protocol (initiated by Zed, **Apache-2.0**) for editor ↔ agent
communication over **JSON-RPC on stdio** (remote over HTTP/WS). The value: **one
client implementation reaches many agents.**

| Agent | Adapter / path |
|---|---|
| Codex | `codex-acp` (wraps the Codex App Server; community) |
| Claude Code | `zed-industries/claude-code-acp` (wraps the Claude Agent SDK; official Zed) |
| Gemini CLI, Goose, Cursor, Cline, Augment, Copilot (preview) | native/adapter |
| OpenCode | listed as ACP-compatible |

- **SDKs:** official Rust crate `agent-client-protocol` (also TypeScript, Python,
  Kotlin, Java; Go community).
- **Coverage:** prompts, streaming, tool calls, permission requests, file changes,
  hunk review — enough for Orquestador's needs.
- **Maturity caveat:** adapters are early; `codex-acp` states it is "under active
  development" with "breaking changes likely". ACP itself is aimed at editors.

## A.4 Comparison

| Option | Topology | Maturity | What you get |
|---|---|---|---|
| Codex App Server | child process + JSON-RPC over stdio | **official, maintained by OpenAI** | Codex only, but complete and UI-ready |
| ACP | child process + JSON-RPC over stdio | open standard, **adapters immature** | **many agents through one integration** |
| Claude Code raw stdio | child process + JSON-RPC over stdio | **input protocol undocumented** | Claude only, with reverse-engineering risk |

## A.5 Recommendation (surfaced to the user, decision pending)

- **Short term:** build a native adapter against `codex app-server --stdio` — the
  direct analogue of the current OpenCode integration, official and predictable.
- **Strategic:** implement an **ACP client in the Rust core** (`agent-client-protocol`)
  so future agents (Claude Code, Gemini CLI, Goose, OpenCode) come for free.
- **Pragmatic:** could do both — ACP as the unifying layer, native `app-server`
  where ACP is weakest.
- **Prerequisite regardless of choice:** install the agent binary (this machine
  currently has only `opencode`); a previous "in-app adapter vs sidecar gateway"
  A/B question should be answered against this research.

## A.6 Sources

- Codex App Server: `https://developers.openai.com/codex/app-server.md`
- OpenAI blog, *Unlocking the Codex harness* (2026-02-04)
- Codex source: `openai/codex` (`codex-rs/app-server`)
- Claude Code headless: `https://code.claude.com/docs/en/headless`
- Claude Agent SDK streaming vs single mode:
  `https://code.claude.com/docs/en/agent-sdk/streaming-vs-single-mode`
- Open issues on the Claude Code stdio control protocol: `anthropics/claude-code`
  #24594, #77632
- Agent Client Protocol: `https://agentclientprotocol.com` (agents list:
  `/get-started/agents`); Rust crate `agent-client-protocol`

## A.7 Decision checklist (for when Track 3 starts)

- [ ] Choose strategy: Codex `app-server` native · ACP client · both.
- [ ] Install the target agent binary (`codex`, and/or `claude` + ACP adapters).
- [ ] Pin the Codex CLI version and generate its schema (`codex app-server
      generate-ts`).
- [ ] Define the `AgentBackend` trait seam from the OpenCode HTTP contract.
- [ ] Verify: the same UI flow drives OpenCode and one alternate backend with no
      UI-specific branching.
