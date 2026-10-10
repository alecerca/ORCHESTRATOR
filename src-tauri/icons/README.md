# Icons

`icon.png` here is a **placeholder** (a solid-color square) so that the Tauri
config resolves and `tauri dev` is not blocked by a missing icon.

Before producing a real bundle, replace it with a generated icon set:

```sh
cd src-tauri
npx @tauri-apps/cli icon path/to/logo.png
```

That command writes `icon.png`, `icon.ico`, `icon.icns` and the
`Square*Logo.png` / `StoreLogo.png` variants used by the platform bundlers.
Keep the file name `icon.png` referenced by `tauri.conf.json` → `bundle.icon`.
