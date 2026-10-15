# Icons

Generated application icon set. The master source is `source.png` (1024×1024):
a dark rounded square with a green "O" monogram, using the app palette from
`frontend/src/styles.css` (`#34d07f` on a `#1b2432` → `#0b0e13` gradient).

## Regenerating the set

From the repository root:

```sh
npx @tauri-apps/cli icon src-tauri/icons/source.png
```

This writes the desktop set the platform bundlers consume: `icon.png`,
`icon.ico` (Windows), `icon.icns` (macOS), the raster sizes (`32x32`, `64x64`,
`128x128`, `128x128@2x`) and the `Square*Logo.png` / `StoreLogo.png` variants.

`bundle.icon` in `src-tauri/tauri.conf.json` lists which of these the build
references — keep the two in sync when the set changes.

The command also emits `android/` and `ios/` directories. This app targets
desktop only, so those are not tracked; delete them after regenerating unless
mobile targets are added.

## Rebuilding the master

The mono already needs `source.png`, so this is only for a redraw. Requires
Montserrat Bold (path varies by distribution) and ImageMagick:

```sh
magick -size 1024x1024 gradient:'#1b2432-#0b0e13' \
  \( -size 1024x1024 xc:none -fill white \
     -draw 'roundrectangle 0,0 1023,1023 205,205' \) \
  -alpha off -compose CopyOpacity -composite \
  -font /usr/share/fonts/julietaula-montserrat-fonts/Montserrat-Bold.otf \
  -pointsize 700 -fill '#34d07f' -gravity center -annotate +0-10 'O' \
  -depth 8 source.png
```
