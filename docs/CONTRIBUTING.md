# Contributing to Orquestador

Thanks for wanting to help. This project is small but deliberate: a Rust core,
a Tauri shell and a React frontend, all under one roof. The rules below keep it
reviewable and headless-testable.

## Ground rules

- **One feature, one branch.** Name it `feat/<short-name>` (or `docs/`,
  `fix/`, `chore/`). Branch from `main`, never commit straight to it.
- **Conventional Commits** with scopes: `feat(ui)`, `feat(core)`, `docs(odd)`,
  `fix(cowork)`, `chore(meta)`, … Keep each commit a reviewable work unit:
  behavior, its tests and its docs land together.
- **No AI attribution** in commits (no `Co-Authored-By`). The work speaks for
  itself.
- **Features get a plan.** Substantial work tracks a lightweight feature
  document under `odd/tasks/<feature>.md` (objective, tasks, acceptance
  criteria, progress) before the first write.

## Repository layout

```
crates/orquestador-core/   # all core logic — headless, no Tauri dependency
src-tauri/                 # Tauri 2 desktop shell (standalone workspace)
frontend/                  # Vite + React + TypeScript + Monaco
scripts/                   # setup-system-deps.sh · verify-headless.sh · …
docs/                      # INSTALL · ARCHITECTURE · COWORK · RUNBOOK · …
odd/tasks/                 # per-feature task documents
SPEC.md                    # canonical specification
```

## Verifying your change (headless — no system deps needed)

```sh
bash scripts/verify-headless.sh
```

It runs: `cargo fmt --check`, `cargo check -p orquestador-core`,
`cargo test -p orquestador-core`, `cargo clippy -p orquestador-core -- -D
warnings`, then `npm install && npm run build` in `frontend/`, plus the
`todo!`/`unimplemented!` audit.

Individually:

```sh
cargo test -p orquestador-core
cargo clippy -p orquestador-core --all-targets -- -D warnings
cd frontend && npx tsc --noEmit && npm run build
```

The core must stay headless-testable: it deliberately has **no Tauri
dependency**, so `cargo check`/`cargo test` at the root never compiles
tauri/wry/webkit. Keep it that way.

## Code conventions

- Core: `async fn` over the `AgentBackend` seam (boxed futures), typed HTTP
  client + SSE event hub, session/state model in `crates/orquestador-core`.
- Frontend: strict TypeScript, React function components, tokens from
  `styles.css` (never new raw colors), Monaco bundled locally.
- Behavior changes ship with tests; UI changes ship with a passing
  `tsc --noEmit` + `npm run build`.

## Desktop shell

The GUI itself needs the Tauri system dependencies (Linux: webkit2gtk-4.1, see
[`docs/INSTALL.md`](docs/INSTALL.md)). They are **not** needed to develop or
test the core — a change that only touches core/frontend never requires them.

## Pull requests

1. Open a PR from your branch to `main`.
2. Summarise what changed and why; link the feature doc if one exists.
3. Note any manual checks that need a human (visual pass, running GUI).
4. Keep the diff focused: a clean 200–400-line PR beats a sprawling one; split
   big features into chained PRs when you can.

## Docs

User-facing docs live in `docs/`. Update them with the feature that changes
them (install steps, cowork, runbook). Public artifacts stay in English;
commit messages follow Conventional Commits.