# 🖼️ Bing Wallpaper for macOS

Automatically update your macOS wallpaper with Bing's daily images from around the world.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![macOS](https://img.shields.io/badge/macOS-10.12+-brightgreen.svg)](https://www.apple.com/macos)
[![Shell Script](https://img.shields.io/badge/Shell-Bash-4EAA25.svg)](https://www.gnu.org/software/bash/)

## ✨ Features

- 🌍 **Region Cycling** - Rotates through 12 Bing regions for maximum wallpaper variety
- 🔄 **Auto-updates** - Updates wallpaper every hour via LaunchAgent
- 🖥️ **All Desktops** - Updates all Mission Control spaces and displays
- 🖼️ **High Quality** - Supports 4K (UHD), Full HD, and HD resolutions
- 🔍 **Auto-detection** - Automatically detects your screen resolution
- 🧹 **Auto-cleanup** - Removes old wallpapers after configurable days
- ⚙️ **Configurable** - Easy configuration via command line
- 💪 **Lightweight** - Pure bash, minimal dependencies

## 🌍 Supported Regions

Each run cycles to the next region, giving you different wallpapers throughout the day:

| Region | Country |
|--------|---------|
| en-US | 🇺🇸 United States |
| en-GB | 🇬🇧 United Kingdom |
| en-AU | 🇦🇺 Australia |
| en-CA | 🇨🇦 Canada |
| de-DE | 🇩🇪 Germany |
| fr-FR | 🇫🇷 France |
| ja-JP | 🇯🇵 Japan |
| zh-CN | 🇨🇳 China |
| pt-BR | 🇧🇷 Brazil |
| es-ES | 🇪🇸 Spain |
| it-IT | 🇮🇹 Italy |
| en-IN | 🇮🇳 India |

## 🚀 Installation

### Prerequisites

- macOS 10.12 or later
- [jq](https://stedolan.github.io/jq/) (JSON processor)
  ```bash
  brew install jq
  ```

### Quick Install

```bash
git clone https://github.com/masrurimz/bing-wallpaper-mac.git
cd bing-wallpaper-mac
./install.sh
```

### Manual Install

```bash
# Copy script to local bin
mkdir -p ~/.local/bin
cp bing_wallpaper.sh ~/.local/bin/bing_wallpaper
chmod +x ~/.local/bin/bing_wallpaper

# Create config
mkdir -p ~/.config/bing-wallpaper
cat > ~/.config/bing-wallpaper/config << 'EOF'
RESOLUTION=UHD
AUTO_CLEANUP=true
CLEANUP_DAYS=7
SAVE_PATH=$HOME/.wallpapers
REGION_MODE=cycle
REGION=en-US
EOF

# Run manually
~/.local/bin/bing_wallpaper --force
```

## ⚙️ Configuration

```bash
# Interactive configuration wizard
bing_wallpaper --config

# View current settings
bing_wallpaper --show-config

# Force update (skip cache check)
bing_wallpaper --force
```

### Configuration Options

| Option | Values | Description |
|--------|--------|-------------|
| `RESOLUTION` | `auto`, `UHD`, `FHD`, `HD` | Wallpaper resolution |
| `REGION_MODE` | `cycle`, `single` | Rotate regions or use fixed |
| `REGION` | `en-US`, etc. | Fixed region (when mode=single) |
| `AUTO_CLEANUP` | `true`, `false` | Auto-delete old wallpapers |
| `CLEANUP_DAYS` | Number | Days to keep wallpapers |
| `SAVE_PATH` | Path | Where to save wallpapers |

## 🔄 Auto-Update Setup

The install script creates a LaunchAgent that runs hourly. To set up manually:

```bash
cat > ~/Library/LaunchAgents/com.$USER.bingwallpaper.plist << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.$USER.bingwallpaper</string>
    <key>ProgramArguments</key>
    <array>
        <string>$HOME/.local/bin/bing_wallpaper</string>
        <string>--force</string>
    </array>
    <key>StartInterval</key>
    <integer>3600</integer>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
EOF

launchctl load ~/Library/LaunchAgents/com.$USER.bingwallpaper.plist
```

## 📁 File Structure

```
~/.local/bin/bing_wallpaper          # Main script
~/.config/bing-wallpaper/
├── config                            # Configuration file
└── region_state                      # Current region index
~/.wallpapers/                        # Downloaded wallpapers
├── bing_20240122_1200_en-US_UHD.jpg
├── bing_20240122_1200_en-US_UHD.txt  # Metadata
└── ...
~/Library/LaunchAgents/com.$USER.bingwallpaper.plist
```

## 📝 Logs

```bash
# View logs
cat ~/.wallpapers/bing_wallpaper.out
cat ~/.wallpapers/bing_wallpaper.err
```

## 🗑️ Uninstallation

```bash
./uninstall.sh
```

Or manually:

```bash
launchctl unload ~/Library/LaunchAgents/com.$USER.bingwallpaper.plist
rm ~/Library/LaunchAgents/com.$USER.bingwallpaper.plist
rm ~/.local/bin/bing_wallpaper
rm -rf ~/.config/bing-wallpaper
rm -rf ~/.wallpapers  # Optional: keep your wallpapers
```

## 🤝 Contributing

Contributions welcome! Feel free to:
- Report bugs
- Suggest new regions
- Add features (day/night mode coming soon!)

## 📝 License

[MIT License](LICENSE) - feel free to use and modify!

## 🙏 Credits

- Forked from [luoling8192/bing-wallpaper-mac](https://github.com/luoling8192/bing-wallpaper-mac)
- Bing for providing beautiful daily images

---

Made with ❤️ for macOS users who appreciate beautiful wallpapers
