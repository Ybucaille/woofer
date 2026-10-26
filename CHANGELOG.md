# Changelog

All notable changes to Woofer are listed here. Versions follow [Semantic Versioning](https://semver.org/):
while Woofer is 0.x, a minor version (0.2.0) may change flags, `ctl` commands or the preset contract.

## [Unreleased]

### Fixed

- Kick detection no longer depends on the frame rate. At `--fps 30` (or the idle rate) a strong off-beat bassline
  could take over the beat: in one house track, 22 of 45 kicks were missed and beats fired on the bass. The daemon
  now analyses 60 times per second of audio whatever it renders at, and the detector (shared by all hosts) watches
  the fastest-rising band in 40–240 Hz instead of the bass average, which a saw or sub bass could outscore.

## [0.1.0] - 2026-10-26

First release: your wallpaper, but alive.

### Daemon

- Wallpaper daemon for Wayland compositors with `wlr-layer-shell`: one surface per output, hotplug and
  fractional scale, `--layer background` (draws the wallpaper) or `--layer bottom` (over another wallpaper tool).
- Direct PipeWire capture of the default output (or `--source <node>`) as a passive node, FFT, 16 bands,
  bass / mid / high and kick detection.
- Your wallpaper as input: `--wallpaper <image>`, or `--wallpaper caelestia` to follow Caelestia's wallpaper and
  Material You colours live.
- Light by design: renders only on frame callbacks, drops to `--idle-fps` (10, or 0 to stop) after 2 s of
  silence, pauses outputs showing a fullscreen window on Hyprland. Measured: 0 % CPU in silence with
  `--idle-fps 0`, ~3 % of one core with music at 60 fps, 29 MB RSS ([details](daemon/BENCH.md)).
- Live control: `woofer ctl preset <id> | intensity <x> | pause | resume | status`.
- `woofer snapshot` renders one offscreen frame, `woofer --version` prints the version.

### Presets

- Wallpaper effects: `woofer`, `ripple`, `glitch`, `drift`, `heat`.
- Over your wallpaper: `aurora`, `bokeh`, `chrome`, `shards`.
- Generative: `nebula`, `synthwave`, `ribbon`, `pulse`.
- One-file Shadertoy-style GLSL format, documented in [shaders/CONTRACT.md](shaders/CONTRACT.md).

### Also

- Quickshell / Caelestia module running the same presets (`qs -p quickshell/shell.qml`).
- Browser preview with demo beat, mic or audio file, your own wallpaper, and 9:16 / 16:9 recording.
- Website: https://ybucaille.github.io/woofer/

### Known limits

- Tested on Hyprland only so far. Other layer-shell compositors should work; reports are welcome.
- The fullscreen pause uses Hyprland's event socket; elsewhere the daemon relies on frame callbacks stopping.
- An off-beat bass note louder than ~80 % of the kick can take the first beat after a bass-only intro;
  the detector locks back onto the kick within a beat.

[0.1.0]: https://github.com/Ybucaille/woofer/releases/tag/v0.1.0
