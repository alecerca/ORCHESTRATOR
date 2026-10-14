# Launch kit

How Orquestador reaches the people who already have the problem. Everything here
rests on one idea: **distribution is not "publish and wait" — it is joining the
conversations where the pain is already written down.**

## The one-liner

> Orquestador is the desktop control tower for orchestrator sub-agents: watch
> every sub-agent live, message *just one*, and stop it without killing the rest.

## The pain (say it before any feature)

Run [gentle-ai](https://github.com/Gentleman-Programming/gentle-ai) or any
OpenCode orchestrator and you get five sub-agents working at once. The terminal
shows one transcript at a time, and correcting one agent means interrupting all
of them.

The demand is public: OpenCode has an open request for exactly this (issue
**#12463**, "Sub-agent status sidebar panel"), and a community TUI plugin,
[`giovannic96/opencode-subagents-view`](https://github.com/giovannic96/opencode-subagents-view),
already fills part of it — in the terminal.

Orquestador is that same idea as a **desktop app**: live panes, per-agent
control, a real diff viewer, and desktop notifications.

## The 90-second demo

Record it once; reuse it everywhere. Screen-only, no voice-over needed.

| Time | Shot | On-screen text |
|------|------|----------------|
| 0:00–0:10 | Terminal: fire a big prompt at an OpenCode + gentle-ai session. Three sub-agents spawn. | *Five agents. No idea who's doing what.* |
| 0:10–0:25 | Open Orquestador on the same session. Three live panes, each with a status. | *Every agent, live.* |
| 0:25–0:45 | Click one agent, send a correction to **only** it; the other two keep running. | *Fix one. Without stopping the rest.* |
| 0:45–1:00 | One agent goes off the rails → Stop that pane only. | *Stop one, keep the others.* |
| 1:00–1:15 | Diff viewer + a desktop notification. | *Orquestador — control tower for your agents.* |

Rule: pain first, relief second. Never open with a feature list.

## Publishing the demo

Status: **published.** The video is embedded in the README's **Demo** section
through a GitHub user-attachments link — a `<video>` tag whose `src` is
`https://github.com/user-attachments/assets/<uuid>` — with
[`orquestador-demo.mp4`](../orquestador-demo.mp4) committed in the repo as a
fallback for anyone who cannot play the inline embed. Screenshots are still
pending ([`screenshots/README.md`](screenshots/README.md)); the video is the
hero instead.

To update the demo later:

1. Upload the new file by **dragging it into a GitHub comment box** (any issue
   or PR comment works). GitHub uploads it and replies with a URL of the form
   `https://github.com/user-attachments/assets/<uuid>`.
2. Replace the `src` in the README's **Demo** `<video>` tag with the new URL:

   ```html
   <video src="https://github.com/user-attachments/assets/<uuid>" controls playsinline width="900"></video>
   ```

3. Commit the render as `orquestador-demo.mp4` so the fallback link stays
   current, and verify the video plays inline on the rendered README before
   sharing any link to the repo.

## Where to share it (in order)

1. **OpenCode issue #12463** — say you built a desktop solution and link the
   demo. It is the exact conversation, not spam.
2. **gentle-ai community + a direct message to the author**
   ([Gentleman-Programming](https://github.com/Gentleman-Programming)).
   Orquestador is a community tool for their ecosystem, and gentle-ai explicitly
   accommodates tools that live outside `opencode.json`. This is the
   highest-leverage contact.
3. **r/opencode, r/LocalLLaMA, Hacker News (Show HN).**
4. **X/Twitter** with the video, tagging OpenCode and gentle-ai.

## The golden rule

Every post is **pain + demo + link**. Never "I made a new project, feedback
please" — that is noise, and it buries the post.

## Making it installable

A one-command install is the adoption enabler, not vanity. The pieces now
exist:

- **Release pipeline** — `.github/workflows/release.yml` builds the native GUI
  installers (deb/rpm/AppImage, msi/nsis, one universal dmg) on
  ubuntu/windows/macos **and** the headless `orquestador-proxy` CLI for
  Linux/macOS/Windows, publishing everything to the GitHub Releases page
  whenever a `v*` tag is pushed. Nothing is published yet: the first `v*` tag
  flips this from "build from source only" to "grab your installer".
- **One-line install** — `scripts/install.sh` (Linux/macOS) and
  `scripts/install.ps1` (Windows) download the right bundle from the latest
  release and install it; with `--cli` / `-Cli` they install the
  `orquestador-proxy` binary instead. They need a published release to point at.

So the funnel until the first tag: demo + repository + build-from-source docs
([`INSTALL.md`](INSTALL.md)). After the first tag: releases page + the
one-liners (GUI and CLI).
