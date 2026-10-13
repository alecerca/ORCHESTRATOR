# Orquestador — First-run onboarding + UI/UX coherence

## Objective

Make a brand-new install feel guided, coherent and trustworthy: greet the user
with a first-run Welcome screen instead of a cowork-only "Join a workspace"
form, replace dead-end/ambiguous states with helpful ones, fix the real design
defects found in the audit, and land the basic accessibility the shell is
missing — without changing product behaviour or the wire contract.

## Problem (evidence from the component audit)

1. **First run is cowork-first.** `App.tsx:183` opens `JoinWorkspaceDialog`
   whenever `!hasPersistedConfig()`. The primary persona (a solo dev with a local
   `opencode serve` at `http://127.0.0.1:4096`) is greeted with a username +
   password form titled "Join a workspace" — misleading and scary.
2. **Misleading / dead-end states.**
   - `AgentPanel.tsx:149` says "Connecting to the orchestrator…" even when the
     server is unreachable or never connected.
   - `AgentPanel.tsx:114-132` renders a `<select>` with a single
     "No orchestrator session…" option that looks interactive but does nothing.
   - `FileTree.tsx` renders nothing on an empty root (blank panel, no cue).
   - `MessageStream.tsx:57` empty copy is bare ("No messages yet.").
   - `ConnectionBanner.tsx:30` says "Lost connection…" even on a never-connected
     server, and never names the URL or how to start the server.
   - `PaneGrid.tsx:66-72` empty workspace is two plain sentences.
3. **Design defects.**
   - `styles.css:378` `.project-picker__lock` uses `var(--muted-text)`, a token
     that does not exist (the token is `--muted`) → uncoloured text.
   - `ActivityBar.tsx` uses `▤` for BOTH "Show editor" (line 41) and "Close all
     panes" (line 57) → two different actions share one icon.
   - No global `:focus-visible` treatment: several controls set `outline:none`
     with only a border-colour change, so keyboard focus is easy to lose.
   - `StatusChip.tsx:14` `title={status}` exposes the raw enum
     (`waiting_approval`) as a tooltip.
4. **Accessibility gaps.**
   - `ApprovalDialog.tsx` and `SkillSavePrompt.tsx` render `.modal` with no
     `role="dialog"` / `aria-modal` / labelled heading.
   - `EditorPanel.tsx` tabs have no `role="tablist"` / `role="tab"` /
     `role="tabpanel"`.
   - `AgentCard.tsx:215-217` errors lack `role="alert"` (whereas
     `AgentPanel.tsx:134` has it).
   - `FollowUpInput.tsx` input has no accessible name; its error lacks
     `role="alert"`. `Composer.tsx` error likewise.
   - `MessageStream.tsx` / `NotificationsDrawer.tsx` have no live region, so
     screen readers never hear new output or toasts.

## Scope

- Frontend only: `frontend/src/**` (one new component, several edits) and
  `frontend/src/styles.css`. No Rust, no wire types, no `SPEC.md` change.
- No behaviour change to commands, layout logic, or the store.

## Out of scope (candidates, not now)

- Real focus trap / roving focus in modals beyond initial focus + Escape.
- Restructuring `TabStrip` so the close button is not nested inside `role="tab"`.
- Making git "changed files" clickable (needs a new open-file wiring path).
- A "replay onboarding" entry in Settings.

## Tasks

- [x] **U1 — First-run Welcome (highest priority).** New
  `components/WelcomeDialog.tsx`: brand mark, value prop, live connection line,
  a 3-step "how it works" list, primary **Connect / Get started**, secondary
  **Join a shared workspace**, ghost **Skip**. `App.tsx`: show it on first run
  (`!hasPersistedConfig() && !welcomeSeen`) instead of the Join form; persist
  `orquestador.welcomeSeen` on any dismiss so it never nags; `Connect` persists
  the default local config via the same `setServerConfig` path. `styles.css`:
  welcome styles.
- [x] **U2 — Guidance & coherent states.** `PaneGrid` getting-started empty
  state; `AgentPanel` context-aware copy + disabled picker when empty;
  `ConnectionBanner` names the URL and distinguishes "can't reach" from "lost";
  `FileTree` empty/loading states + `aria-expanded` + `role="alert"`; richer
  `MessageStream` empty + `role="log"`.
- [x] **U3 — Design polish + bugs.** Fix `--muted-text` → `--muted`; global
  `:focus-visible`; replace `ActivityBar` glyphs with inline SVG icons (no more
  duplicate `▤`) + `aria-pressed`; clearer `TopBar` meta; friendlier
  `StatusChip` titles.
- [x] **U4 — Accessibility.** Dialog roles/labels/initial-focus in
  `ApprovalDialog` + `SkillSavePrompt`; `role="tablist|tab|tabpanel"` in
  `EditorPanel`; `aria-label` + `role="alert"` in `FollowUpInput` / `Composer`;
  `aria-live` + labelled dismiss buttons in `NotificationsDrawer`.

## Acceptance criteria

1. A fresh install shows the Welcome screen (not "Join a workspace"); skipping
   it does not reappear on the next launch; joining/comms still reachable.
2. No screen shows a raw enum, an ice-cold dead end, or copy that contradicts
   the actual connection state.
3. `--muted-text` is gone; the two activity-bar actions no longer share an icon;
   every interactive control shows a visible keyboard focus ring.
4. The two modals announce themselves as dialogs; the editor tabs expose tab
   semantics; the stream and toasts are live regions.
5. Build stays green: `npx tsc --noEmit` + `npm run build`.

## Checks (per task)

- `cd frontend && npx tsc --noEmit && npm run build`
- `bash scripts/verify-headless.sh` if anything outside the frontend is touched
  (not expected here).

## Progress log

- [x] Feature document created (2026-10-12) on branch `feat/ui-ux-onboarding`.
- [x] U1..U4 implemented.
- [x] Verification: `npx tsc --noEmit` (exit 0) + `npm run build` (exit 0).
- [x] Committed + pushed after explicit user approval:
  - `167ea46` feat(ui): add first-run onboarding, guided states and an a11y pass
  - `df52fe5` docs(odd): record the ui/ux onboarding feature plan
  - upstream: `origin/feat/ui-ux-onboarding` (PR not opened).

## Status notes

- Receipt-driven development is enabled (global), but the review START returned a
  `gentle-ai.review-integration.consent/v3` envelope that must be relayed through
  the native `question` UI, which this runtime does not expose. Per the contract
  the consent is not relayable from chat, so no review was started and no
  authority was mutated; delivery proceeded under ordinary policy with explicit
  user approval.
- Follow-ups (out of scope here): open the PR, then optionally run the native
  review from an interactive runtime that exposes the `question` UI.
