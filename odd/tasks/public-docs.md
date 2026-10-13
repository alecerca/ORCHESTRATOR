# Orquestador — Public open-source documentation (README, install, contributing, license, screenshots)

## Objective

Make the repository presentable as an open-source project on GitHub:

1. **README.md** rewritten as the public landing page: the "why" story, the
   verified feature set, generic cross-platform install, quick start, docs
   index, license.
2. **docs/INSTALL.md** — generic desktop installation for Windows / macOS /
   Linux (from a release + build from source, per-OS prerequisites).
3. **docs/CONTRIBUTING.md** — contribution standards so outsiders can submit
   changes (commits, branches, tests, review).
4. **LICENSE** — the repo declares `license = "MIT"` in Cargo.toml but has no
   LICENSE file. **Blocked on the user**: copyright holder for the MIT header.
5. **Screenshots** — `docs/screenshots/` with real `.png` of the app embedded
   in the README. **The agent cannot capture the GUI**: the X/Wayland display
   is not reachable from the shell (ImageMagick cannot connect; no Xvfb).
   Either the user captures from their desktop following the shot list, or the
   section stays as placeholders.
6. **Metadata fix** — `Cargo.toml` `repository` points at
   `github.com/anomalyco/opencode` (the opencode repo); it must point at this
   repository (`https://github.com/alecerca/ORCHESTRATOR`).
7. **Verification** — all doc links resolve (note: the old README referenced a
   `SPEC.md` that does not exist in `docs/`; check and drop stale links).

## Scope

- `README.md`, `docs/INSTALL.md`, `docs/CONTRIBUTING.md`, `docs/screenshots/`.
- `Cargo.toml` (metadata only).
- Optional follow-ups (out of scope unless the user asks): `.github/` issue/PR
  templates, CI, CHANGELOG.

## Constraints

- README and user-facing artifacts in English (public open-source audience).
- No product claims that are not verified in the code or docs (rename, branch
  chip, remote server settings, cowork, skills, tabs/splits were verified).
- Install docs stay generic: no per-distro binary paths, honest about the
  release channel (no releases published yet → "from a release" is the future
  path, "build from source" is the current one).
- No code changes: docs + metadata only. Core/frontend unchanged → no review
  ceremony needed beyond the usual build check.

## Tasks

- [ ] **D1 — Public README.** Hero, why (personal tool story, the two
  shareable turns: cowork + bring-your-own OpenCode server), verified features
  (agent orchestration / workspace / skills / cowork), screenshots section with
  placeholders, install links, quick start, architecture one-liner, docs index,
  license. *Exit: links resolve; claims verifiable.*
- [ ] **D2 — docs/INSTALL.md.** Generic desktop install: from a release
  (Windows .msi/.exe, macOS .dmg, Linux .deb/.rpm/.AppImage) and build from
  source with per-OS prerequisites (Linux webkit2gtk-4.1/appindicator/librsvg/
  patchelf via `scripts/setup-system-deps.sh`; Windows WebView2 + MSVC; macOS
  Xcode CLT). Dev run + env config (`OPENCODE_SERVER_BASE_URL`,
  `OPENCODE_SERVER_PASSWORD`) + in-app server settings.
- [ ] **D3 — docs/CONTRIBUTING.md.** Branching (`feat/`, `docs/`), Conventional
  Commits with scopes, no AI attribution, headless verification
  (`scripts/verify-headless.sh`), feature docs under `odd/tasks/`, PR to `main`.
- [ ] **D4 — docs/screenshots/** capture list + destination. Real captures done
  by the user (agent cannot reach the display) or skipped.
- [ ] **D5 — Metadata.** `Cargo.toml` `repository` → this repo. *Exit: URL
  resolves.*
- [ ] **D6 — LICENSE.** MIT text + holder. **Blocked on the user's answer.**
- [ ] **D7 — Verification.** Link audit, `cd frontend && npx tsc --noEmit &&
  npm run build` still green (unchanged), tree clean after commits.

## Delivery

- Branch `docs/public-docs`. Doc work splits cleanly per file → separate
  `docs(public)` / `chore(meta)` commits; no 400-line pressure (cumulative
  forecast ~450–600 authored lines across D1–D6).
- Push/merge remain the user's decision (their established gate).

## Progress log

- [x] Feature document created (2026-10-13) on `main`, before the branch.
- [x] Branch `docs/public-docs` created.
- [x] D1 — public README rewrite. `d3764bd` docs(public): rewrite the README
  for the open-source showcase (127 insertions / 84 deletions).
- [x] D2 — `docs/INSTALL.md` generic cross-platform guide. `352a9ab`.
- [x] D3 — `docs/CONTRIBUTING.md` (+ shot-list README). `03adfc6`.
- [x] D5 — `Cargo.toml` `repository` → `https://github.com/alecerca/ORCHESTRATOR`.
  `68d6edb` chore(meta): point the workspace repository URL at this repository.
- [x] D7 (partial) — link audit passed: every path referenced in the new docs
  exists (`docs/*`, `SPEC.md`, `scripts/*`). Frontend rebuild skipped: no code
  changed on this branch; last build on `main` (0713ac9) was green.
- [ ] D4 — `docs/screenshots/` real captures. **User action**: the agent cannot
  reach the X/Wayland display from the shell (ImageMagick cannot connect, no
  Xvfb). Shot list ready in `docs/screenshots/README.md`.
- [ ] D6 — LICENSE file. **Blocked on the user**: MIT copyright holder for the
  license header.
- [ ] Branch `docs/public-docs` committed (**not pushed**); push/merge/PR are
  the user's decision.