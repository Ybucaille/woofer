#!/bin/sh
# Installs this Woofer release for the current user: ~/.local/bin/woofer and ~/.local/share/woofer/shaders.
# Needs PipeWire and a Vulkan driver at runtime (vulkan-intel, vulkan-radeon or nvidia-utils on Arch).
set -e
cd "$(dirname "$0")"
bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
data="${XDG_DATA_HOME:-$HOME/.local/share}/woofer"
install -Dm755 woofer "$bin/woofer"
mkdir -p "$data/shaders"
cp shaders/*.glsl shaders/CONTRACT.md "$data/shaders/"
echo "Installed $("$bin/woofer" --version) to $bin, presets to $data/shaders"
case ":$PATH:" in *":$bin:"*) ;; *) echo "Note: $bin is not in your PATH." ;; esac
echo "Try: woofer --wallpaper ~/Pictures/your-wallpaper.jpg --preset woofer"
