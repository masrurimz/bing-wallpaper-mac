# bing-wallpaper

Update your macOS desktop with Bing's daily images. The `bing_wallpaper` agent runs every hour, downloads one new image per market, deduplicates by content key, and sets a random wallpaper from the archive.

## Install

1. Build the binary:

	```bash
	git clone https://github.com/masrurimz/bing-wallpaper-mac.git
	cd bing-wallpaper-mac
	cargo build --release
	```

2. Install the binary and the LaunchAgent:

	```bash
	cp target/release/bing_wallpaper ~/.local/bin/
	bing_wallpaper init
	```

`init` creates `~/.config/bing-wallpaper/config`, writes `~/Library/LaunchAgents/com.masrurimz.bingwallpaper.plist`, and loads it with `launchctl`.

## Use

The agent runs every hour automatically. To run once by hand:

```bash
bing_wallpaper run
```

To see the current state:

```bash
bing_wallpaper status
```

To remove duplicate files:

```bash
bing_wallpaper prune
```

To change settings interactively:

```bash
bing_wallpaper config
```

To set one value directly:

```bash
bing_wallpaper config --set RESOLUTION=FHD
```

To print the current config:

```bash
bing_wallpaper config --show
```

## Config

`~/.config/bing-wallpaper/config` uses `KEY=value` format.

- `RESOLUTION`: `UHD` (default), `FHD`, `HD`, or `auto`
- `AUTO_CLEANUP`: `true` (default) or `false`
- `CLEANUP_DAYS`: number of days to keep wallpapers (default `14`)
- `SAVE_PATH`: wallpaper directory (default `~/.wallpapers`)
- `REGION_MODE`: `cycle` (default) or `single`
- `REGION`: the market to use when `REGION_MODE=single` (default `en-US`)

## Uninstall

```bash
launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.masrurimz.bingwallpaper.plist
rm ~/Library/LaunchAgents/com.masrurimz.bingwallpaper.plist
rm ~/.local/bin/bing_wallpaper
rm -rf ~/.config/bing-wallpaper
```

Keep `~/.wallpapers` if you want to save the images.
