#!/usr/bin/env bash
# Builds the presets and prints how to start the layer with Hyprland. Changes nothing outside this folder.
set -euo pipefail
cd "$(dirname "$0")"

missing=()
for cmd in qs cava node; do command -v "$cmd" >/dev/null || missing+=("$cmd"); done
QSB="${QSB:-$(command -v qsb || echo /usr/lib/qt6/bin/qsb)}"
[[ -x "$QSB" ]] || missing+=("qsb (qt6-shadertools)")
if ((${#missing[@]})); then
    echo "Missing: ${missing[*]}" >&2
    echo "Arch: sudo pacman -S cava nodejs qt6-shadertools  (quickshell: AUR quickshell-git)" >&2
    exit 1
fi

QSB="$QSB" node tools/build.mjs

cat <<MSG

Built. Try it now:
  qs -p "$PWD/quickshell/shell.qml"

Start it with Hyprland — add to hyprland.conf:
  exec-once = qs -p "$PWD/quickshell/shell.qml"

Switch preset live:
  qs -p "$PWD/quickshell/shell.qml" ipc call reactive setPreset <id>
Presets: $(ls shaders/*.glsl | xargs -n1 basename | sed 's/\.glsl$//' | paste -sd' ')
MSG
