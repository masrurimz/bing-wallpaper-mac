# 🖼️ Bing Wallpaper for macOS

Bing daily wallpaper for macOS — now a Rust binary. The LaunchAgent just triggers it hourly; the binary does the rest.

## Features

- Region cycling through 12 Bing markets
- Deduplication by image content key (`OHR.{ID}`) across markets
- Auto-cleanup of wallpapers older than `CLEANUP_DAYS`
- Random shuffle of all saved wallpapers on every run
- High quality: UHD, FHD, HD, or auto-detected

## Build

```bash
cargo build --release
```

Binary: `target/release/bing_wallpaper`

## Install

```bash
cargo build --release
cp target/release/bing_wallpaper ~/.local/bin/
cp com.masrurimz.bingwallpaper.plist ~/Library/LaunchAgents/
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.masrurimz.bingwallpaper.plist
```

## Config

`~/.config/bing-wallpaper/config` (KEY=value)

- `RESOLUTION` — `UHD` (default), `FHD`, `HD`, or `auto`
- `AUTO_CLEANUP` — `true` / `false`
- `CLEANUP_DAYS` — default `14`
- `SAVE_PATH` — default `~/.wallpapers`
- `REGION_MODE` — `cycle` (default) or `single`
- `REGION` — default `en-US` (used when `REGION_MODE=single`)

## Usage

```bash
bing_wallpaper          # one hourly update: fetch new, cleanup, set random desktop
bing_wallpaper --prune  # remove byte-duplicate images from the archive
```

## How it works

1. The LaunchAgent runs the binary every hour.
2. The binary fetches Bing's daily JSON for each market.
3. It skips images already seen by their content key, and skips downloads if a matching file already exists.
4. New images are saved to `~/.wallpapers` with a `.txt` sidecar.
5. Old files are removed based on `CLEANUP_DAYS`.
6. A random wallpaper from the archive is picked and applied to all desktops via `osascript`.

## Logs

```bash
~/.config/bing-wallpaper/bing_wallpaper.out
~/.config/bing-wallpaper/bing_wallpaper.err
```

## Uninstall

```bash
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.masrurimz.bingwallpaper.plist
rm ~/Library/LaunchAgents/com.masrurimz.bingwallpaper.plist
rm ~/.local/bin/bing_wallpaper
rm -rf ~/.config/bing-wallpaper
# optional: rm -rf ~/.wallpapers
```

## License

[MIT](LICENSE)
