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

The demand is public: OpenCode's request for exactly this (issue **#12463**,
"Sub-agent status sidebar panel", now closed) drew 23 reactions, and a community
TUI plugin,
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
`https://github.com/user-attachments/assets/<uuid>`. That embed plays inline for
anonymous visitors, but the **raw asset URL does not**: GitHub serves the file
only through a signed `private-user-images.githubusercontent.com` URL, so a
plain link to the asset 404s for anyone who is not logged in. The fallback for
anyone who cannot play the inline embed is therefore a **YouTube link**
(`https://youtu.be/7CgfHGHYCKM`), never the asset URL. The render is
deliberately **not** committed: tracking it added ~26 MB to every clone. The
video is the hero.

To update the demo later:

1. Upload the new file by **dragging it into a GitHub comment box** (any issue
   or PR comment works). GitHub uploads it and replies with a URL of the form
   `https://github.com/user-attachments/assets/<uuid>`.
2. Replace the `src` in the README's **Demo** `<video>` tag with the new URL:

   ```html
   <video src="https://github.com/user-attachments/assets/<uuid>" controls playsinline width="900"></video>
   ```

3. Do **not** commit the render. Upload the new cut to YouTube and point the
   README's fallback link there — **never** at the raw asset URL, which 404s
   for anonymous visitors. Verify the video plays inline on the rendered README
   before sharing any link to the repo.

## Where to share it (in order)

1. **OpenCode issues** — the demand is already written down: `#12463` (now
   closed) drew 23 reactions. Join the open discussions that match the app
   best, verified open on 2026-10-15:
   - [#38966](https://github.com/anomalyco/opencode/issues/38966) — a running subagent cannot be steered, cancelled or aborted individually
   - [#42670](https://github.com/anomalyco/opencode/issues/42670) — prompt and steer selected subagents
   - [#51232](https://github.com/anomalyco/opencode/issues/51232) — show the child model beside subagent status
   - [#48548](https://github.com/anomalyco/opencode/issues/48548) — sidebar missing when viewing a subagent session
   - [#48837](https://github.com/anomalyco/opencode/issues/48837) — the V2 interface hurts multi-project/multi-agent workflows
   - [#27110](https://github.com/anomalyco/opencode/issues/27110) — limit the max number of parallel subagents
   - [#27511](https://github.com/anomalyco/opencode/issues/27511) — suspend/resume agent and subagent execution
   - [#36423](https://github.com/anomalyco/opencode/issues/36423) — no cancellation support for background subagents
2. **gentle-ai community + a direct message to the author**
   ([Gentleman-Programming](https://github.com/Gentleman-Programming)).
   Orquestador is a community tool for their ecosystem, and gentle-ai explicitly
   accommodates tools that live outside `opencode.json`. This is the
   highest-leverage contact (7,797 stars).
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
  whenever a `v*` tag is pushed. The latest release ships all GUI installers
  plus the CLI binaries for the three platforms.
- **One-line install** — `scripts/install.sh` (Linux/macOS) and
  `scripts/install.ps1` (Windows) download the right bundle from the latest
  release and install it; with `--cli` / `-Cli` they install the
  `orquestador-proxy` binary instead.

The funnel now runs on the releases page and the one-line installers, with
the demo video and the README at the top of it.
