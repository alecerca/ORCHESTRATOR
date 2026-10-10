# Skills (header panel) — feature document

- **Feature**: `skills-header`
- **Directory**: `/home/sanchiro/projects/orquestador` (git, branch `feat/skills-header`, remote `origin`)
- **Created**: 2026-10-10
- **Delivery strategy**: `ask-on-risk` (default). Work-unit commits land on `feat/skills-header`; push/PR remain the user's decision.
- **Delivery forecast**: authored changed lines (adds+deletes) expected above the ~400 planning heuristic (frontend + backend + new core module). One coherent feature delivered as sequential work-unit commits on the branch; not artificially split.

## Objective

Let the user manage a set of **skills** (markdown instruction files) from the app header and have the
active ones silently accompany **every** prompt, so the agent can apply whichever is relevant. The
user sees a Skills button + a `+` button in the header, a dropdown to toggle skills on/off, and the
ability to import a `.md` skill or author a new one in a new tab.

## Problem

Today the app has no notion of skills (`skill` appears nowhere but `.gitignore:34`). Users cannot
attach reusable instructions to their agents from the UI, and there is no place to keep such
instruction files.

## Verified facts (recon, 2026-10-10)

- **Prompt choke point**: `src-tauri/src/commands.rs:128-142` (`send_message_async`) and `:112-126`
  (`send_message`) build `PromptBody::text(text)` then set `body.agent`. `PromptBody.system`
  (`crates/orquestador-core/src/types.rs:821`) exists and is serialized as wire key `system`.
- **`system` is safe**: opencode joins the per-prompt `system` after the agent prompt
  (`packages/opencode/src/session/llm/request.ts`: `[agent.prompt, ...input.system, ...input.user.system].join("\n")`).
  It is **appended**, not a replacement → hidden, universal injection with no transcript pollution.
- **Only two frontend senders**: `Composer.tsx:24` and `FollowUpInput.tsx:23`, both via
  `commands.ts:47-51 sendMessageAsync`. `sendCommand` (`commands.rs:178-194`, `CommandBody`) is a
  separate path with no `system` field — **out of scope**.
- **Tabs model is a closed union**: `frontend/src/tabs.ts:12-26` (`Tab = SessionTab | FileTab`).
  Adding a kind touches: the union, a key fn (`sessionTabKey:56` / `fileTabKey:61`), `openTab:131`,
  `isTab:671`/`normalizeTab:723` validators, `TabStrip.tabTitle:25-31`, and
  `PaneGrid.GroupView:190-263` body branch.
- **No close-confirmation exists**: `onCloseTab` fires straight from `TabStrip` → `App.tsx:378-380`
  → pure `closeTab` reducer (`tabs.ts:232`).
- **No local file IO**: existing `list_files`/`read_file`/`find_files` proxy the opencode server
  (`client.rs:433-496`), not local disk. No Tauri plugins are installed (capabilities =
  `["core:default"]` only; `src-tauri/capabilities/default.json`).
- **Persistence pattern**: frontend `localStorage` owned by `App.tsx` (`orquestador.layout` in
  `tabs.ts:51`; three others in `App.tsx`). `store.ts` is in-memory only.
- **opencode has native skills** (`<dir>/<name>/SKILL.md`, on-demand). **Not used here** — Orquestador
  owns its own skill store and injects explicitly, so it stays self-contained and never mutates the
  user's global opencode config.

## Decisions (settled)

- **App-owned skill store**: `<app_config_dir>/skills/<slug>.md` (Tauri v2 `app.path().app_config_dir()`,
  identifier `com.orquestador.app` → e.g. `~/.config/com.orquestador.app/skills/`). One `.md` file per
  skill; the skill **name is the slugified filename stem**.
- **Injection is hidden and universal**: selected skills are read and joined into `PromptBody.system`
  as a delimited block on every `send_message` / `send_message_async`. The user's own text part is
  untouched; nothing appears in the transcript.
- **Selection persists** in `localStorage` (`orquestador.skills`, array of names) — matches the
  existing persistence pattern; survives restart.
- **Import** uses a webview `<input type="file" accept=".md">` + `FileReader` (no new plugin);
  extension is validated client-side; content is written via the new `save_skill` command.
- **New skill** asks for a name, opens a `skill` tab with a generated scaffold, and is written to disk
  only on save.
- **Close-save prompt applies to `skill` tabs only** (spec: "SOLAMENTE ESTA PREGUNTA SE HARA PARA LAS
  SKILLS"); every other tab keeps today's behavior.
- **`skill` tabs are not persisted across restart** (unsaved content cannot be restored safely); the
  layout normalizer must drop them gracefully rather than trust stale content.

## Scope

**In scope**: header Skills dropdown + `+` button; skill list/read/save/delete commands; skill store
on disk + selection in localStorage; `skill` tab kind + editor; save-on-close prompt (skills only);
.md import + new-skill scaffold; hidden `system` injection on both send paths.

**Out of scope**: opencode-native SKILL.md discovery/registration; per-skill enable/disable beyond
toggle; slash commands; `send_command` injection; skill sharing/sync; description/frontmatter UI.

## Edge cases to cover

- Skill name collision on import/new → overwrite confirmation (or refuse with a clear message).
- Deleting a skill that is selected → drop it from the selection.
- Empty/whitespace-only skill → still saveable but skipped by the injected preamble (no empty block).
- Slugify: trim, lowercase, collapse non-`[a-z0-9-]` runs to `-`, drop leading/trailing `-`; empty
  result → reject with a message.
- Missing/unreadable skill file while selected → skip it silently in the preamble; never fail the send.
- Skill tab closed clean/unchanged → no prompt (matches "only if there is something to save").
- Closing a skill tab while save fails → keep the tab open and surface the error.
- Layout restore containing a `skill` tab → drop it (do not resurrect stale content).

## Tasks

- [x] **T1 — Core skills module (headless, testable)**: add `crates/orquestador-core/src/skills.rs`
  with `slugify(&str) -> String`, `SkillInfo { name, path }`, and
  `build_preamble(&[(String, String)]) -> String` (delimited `<skills>` block, skips empty content).
  Declare the module in `lib.rs`. Unit tests: slugify normalization/rejection, preamble skips empties
  and preserves order. Route: inline (one new file + module decl, no research left). Verification:
  `cargo test -p orquestador-core`. **Evidence: commit `8850d73`; `cargo test -p orquestador-core skills`
  → 4 passed, 0 failed.**
- [x] **T2 — Tauri skill commands + hidden injection (shell)**: in `src-tauri/src/commands.rs` add
  `skills_dir(app)`, `list_skills`, `read_skill`, `save_skill`, `delete_skill`; extend
  `send_message`/`send_message_async` with `skills: Option<Vec<String>>` and set `body.system` from
  `skills::build_preamble` (reading each file; skip missing). Register the new commands in
  `src-tauri/src/lib.rs`. Route: delegated backend writer. Verification: `cargo check` /
  `cargo test` (compile + core). **Evidence: commit `88ca366`; `cargo check --manifest-path
  src-tauri/Cargo.toml` clean; `cargo test -p orquestador-core` → 71 passed.** Worker flagged: `skills`
  payload shape must match T3's frontend arg name; out-of-band non-slug `.md` files are returned by
  file stem (spec says name = stem, so left as-is).
- [x] **T3 — Frontend skill IPC + selection store (frontend)**: `commands.ts` gains
  `listSkills`/`readSkill`/`saveSkill`/`deleteSkill` and an optional `skills?: string[]` on
  `sendMessageAsync`/`sendMessage`; new `frontend/src/skills.ts` owns the selected-name state with
  `localStorage` persistence, `toggle`, `remove`, `isSelected`, `subscribe`, `getSelectedNames`. 
  Also wired send call sites (`Composer.tsx`, `FollowUpInput.tsx`) to pass `getSelectedNames()`.
  Route: inline for call-site wiring + committed after Worker A delivered IPC/store. Verification:
  `npm run typecheck`. **Evidence: commit `4918662`; `cd frontend && npm run typecheck` → clean.**
- [ ] **T4 — `skill` tab kind (frontend)**: extend `frontend/src/tabs.ts` (`SkillTab` union member, key
  fn, `openTab`/`openSkillTab`, `isTab`/`normalizeTab` + drop-on-restore), `TabStrip.tsx` title, and
  `PaneGrid.tsx` body branch (`kind==="skill"` → `SkillEditor`). Route: delegated frontend writer.
- [ ] **T5 — Header Skills + `+` (frontend)**: `TopBar.tsx` gains a `topbar__actions` region: a
  **Skills** button + popover (list from `listSkills`, toggle per skill, selection highlighted) and a
  **`+`** button + menu (**New skill…** name prompt → open skill tab with scaffold; **Import .md…** →
  hidden file input → `saveSkill`). Route: delegated frontend writer.
- [ ] **T6 — Skill editor + save-on-close (frontend)**: new `frontend/src/components/SkillEditor.tsx`
  (Monaco markdown, dirty tracking, load via `readSkill`); wire the close path so **only** `skill` tabs
  prompt "Save skill?" (Save → `saveSkill` then close; Discard → close), covering both the `TabStrip` ✕
  and the context-menu close in `App.tsx:378-380`. Route: delegated frontend writer.
- [ ] **T7 — Docs + feature finalization**: update `docs/PLAN.md` (command list, module) and
  `docs/ARCHITECTURE.md` (skills store + injection) and check off this doc's tasks with evidence.
  Route: inline.
- [ ] **T8 — Verification**: `cargo test --workspace`, `cd frontend && npm run typecheck && npm run
  build`; record results. Manual in-app steps deferred to the user (needs `webkit2gtk-4.1` + a running
  opencode server). Route: delegated verifier.

## Authorized scope

Only files listed in each task's route plus this document and the docs in T7. No changes to
`send_command`, no new Tauri plugins, no writes outside `frontend/`, `src-tauri/`, `crates/`, `docs/`,
`odd/`.

## Acceptance criteria

- Header shows a Skills button (dropdown of saved skills, toggle) and a `+` (New skill / Import .md);
  selection is visibly reflected and survives an app restart.
- New skill opens a `skill` tab with a rooted scaffold; closing it prompts "Save skill?" and saving
  writes `<app_config_dir>/skills/<slug>.md`; non-skill tabs are unaffected.
- Importing a `.md` adds it to the dropdown (extension enforced).
- With skills selected, **every** message from Composer and Follow-Up carries them in `PromptBody.system`
  and nothing extra appears in the user's transcript.
- `cargo test --workspace` and frontend `typecheck`/`build` pass.

## Verification commands

- `cargo test --workspace`
- `cd frontend && npm run typecheck && npm run build`
- Manual (user): restart persistence; close-save prompt scope; a selected skill reaching the agent.
