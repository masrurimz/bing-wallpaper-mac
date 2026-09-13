# 🖼️ Bing Wallpaper for macOS

Bing daily wallpaper for macOS. Written in Rust with a CLI-first interface.

## Build

```bash
cargo build --release
```

Binary: `target/release/bing_wallpaper`

## Install

```bash
cargo build --release
cp target/release/bing_wallpaper ~/.local/bin/
bing_wallpaper init
```

`init` creates `~/.config/bing-wallpaper/config`, installs the LaunchAgent into `~/Library/LaunchAgents/`, and loads it with `launchctl`.

## CLI

```
bing_wallpaper [COMMAND]

Commands:
  run     Run one update cycle (default)
  prune   Remove byte-duplicate images
  status  Print config and archive status
  config  Show or set config values
  init    Create default config and install the LaunchAgent
```

```bash
bing_wallpaper config              # interactive TUI form
bing_wallpaper config --show       # print current config
bing_wallpaper config --set RESOLUTION=FHD
```

`~/.config/bing-wallpaper/config` (KEY=value)

- `RESOLUTION` — `UHD` (default), `FHD`, `HD`, or `auto`
- `AUTO_CLEANUP` — `true` / `false`
- `CLEANUP_DAYS` — default `14`
- `SAVE_PATH` — default `~/.wallpapers`
- `REGION_MODE` — `cycle` (default) or `single`
- `REGION` — default `en-US` (used when `REGION_MODE=single`)

### Read / edit config

```bash
bing_wallpaper config
bing_wallpaper config --show
bing_wallpaper config --set RESOLUTION=FHD
```

## How it works

1. The LaunchAgent runs `bing_wallpaper run` every hour.
2. Fetches Bing's daily JSON for each market.
3. Skips images already seen by their content key, and skips downloads if a matching file already exists.
4. New images are saved to `~/.wallpapers` with a `.txt` sidecar.
5. Old files are removed based on `CLEANUP_DAYS`.
6. A random wallpaper from the archive is picked and applied via the `wallpaper` crate.

## Dependencies

- `reqwest` / `tokio` — HTTP client
- `serde_json` — Bing JSON parsing
- `inquire` — interactive terminal prompts
- `rust-ini` — config parsing
- `rdev` — screen resolution detection
- `wallpaper` — desktop wallpaper setting
- `users` — user id for `launchctl`
- `rand` — random shuffle
- `tracing` / `tracing-subscriber` — logging

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
