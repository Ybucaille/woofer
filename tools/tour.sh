#!/usr/bin/env bash
# Shows every preset on your desktop, one after another, with a notification naming each one.
# Usage: tools/tour.sh [seconds-per-preset] [woofer flags...]   e.g. tools/tour.sh 20 --wallpaper ~/Pictures/wall.jpg
# Defaults to Caelestia's wallpaper on the bottom layer, so it sits above an existing wallpaper tool.
# Ctrl+C stops it and the daemon.
set -euo pipefail
cd "$(dirname "$0")/.."

secs="${1:-20}"
shift || true
flags=("$@")
((${#flags[@]})) || flags=(--wallpaper caelestia --layer bottom)

bin=daemon/target/release/woofer
[[ -x "$bin" ]] || (cd daemon && cargo build --release -j 4)

presets=(woofer ripple glitch drift heat aurora bokeh chrome shards nebula synthwave ribbon pulse)

"$bin" --shaders shaders --preset "${presets[0]}" "${flags[@]}" &
pid=$!
trap 'kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null' EXIT INT TERM

note() { command -v notify-send >/dev/null && notify-send -a Woofer -t 4000 "$@" || echo "$*"; }
for _ in $(seq 50); do "$bin" ctl status >/dev/null 2>&1 && break; sleep 0.2; done
note "Woofer tour" "Play some music and switch to an empty workspace. ${#presets[@]} presets, ${secs} s each."

for i in "${!presets[@]}"; do
    id="${presets[$i]}"
    "$bin" ctl preset "$id" >/dev/null
    desc=$(sed -n 's#^// @description ##p' "shaders/$id.glsl")
    note "$((i + 1))/${#presets[@]} · $id" "$desc"
    sleep "$secs"
done
note "Woofer tour" "Done. Which ones did you like?"
