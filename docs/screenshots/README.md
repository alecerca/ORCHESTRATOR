# Screenshots

Public screenshots of the app live here and are embedded from the README.
Captures must be real app screenshots, not mocked UI.

## Capturing

Run the app (`npx @tauri-apps/cli dev`, or the built binary) with a live
OpenCode server connected, then capture each shot. Any screen-capture tool
works; PNG preferred, high-DPI welcome.

## Shot list

| File | What to show | Notes |
| --- | --- | --- |
| `main-window.png` | The control tower: session tree + agent panes + composer | Hero image, top of the README. Best with an active session and 2+ sub-agents |
| `agent-panes.png` | Per-agent panes: status, timer, branch chip, transcript | Capture a mid-task agent so the pane looks alive |
| `workspace.png` | Editor + diff view / tabs & splits | Show the Monaco diff and the file tree |
| `skills.png` *(optional)* | Skills panel with both OpenCode and app skills | Nice-to-have |
| `cowork.png` *(optional)* | Shared session presence (roles / members) | Requires a second identity; skip if awkward |

## Checklist before publishing

- No personal data: no real repo names that should stay private, no tokens,
  no private session content. Use an innocuous project for the shots if unsure.
- Consistent theme (dark recommended for the hero).
- Crop to the window; consistent width across shots.