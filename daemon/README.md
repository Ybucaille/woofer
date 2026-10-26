# Woofer daemon

Your wallpaper, but alive: a small Rust daemon that draws an audio-reactive shader preset as the wallpaper of
every output, on any Wayland compositor with `wlr-layer-shell` (Hyprland, Sway, niri, river, Wayfire, …).
It reads the presets in `../shaders/` (see [`shaders/CONTRACT.md`](../shaders/CONTRACT.md)) and runs them the same
way the Quickshell layer and the web preview do. Measured cost is in [BENCH.md](BENCH.md).

## Build

Needs Rust ≥ 1.85, PipeWire headers and clang (for the PipeWire bindings) and a Vulkan driver.

```bash
# Arch: pacman -S --needed rust clang pipewire vulkan-icd-loader  (+ vulkan-intel / vulkan-radeon / nvidia-utils)
cargo build --release -j 4
cargo test -j 4          # includes a test that compiles every preset in ../shaders through the wrapper
```

The binary is `target/release/woofer`.

## Run

```bash
woofer --wallpaper ~/Pictures/wall.jpg --preset pulse
woofer --wallpaper caelestia          # follow Caelestia's wallpaper (and its colours) live
woofer ctl preset nebula              # switch preset on the fly
woofer ctl intensity 0.6
woofer ctl pause | resume | status
woofer snapshot out.png --preset ripple --wallpaper ~/Pictures/wall.jpg   # offscreen 960×540 test frame
```

Start it from your compositor's autostart once you're happy (e.g. `exec-once = woofer --wallpaper caelestia`
in `hyprland.conf`, `exec woofer …` in Sway). With Caelestia or another wallpaper tool also on the
background layer, either stop that tool's wallpaper or use `--layer bottom`, which draws transparently above it
(`wall()` still samples `--wallpaper`).

| flag | default | |
|---|---|---|
| `--preset <id>` | `pulse` | runs `shaders/<id>.glsl` |
| `--wallpaper <path\|caelestia\|none>` | `none` | image for `wall()`; `caelestia` reads `~/.local/state/caelestia/wallpaper/path.txt` and reloads when it changes |
| `--colors <caelestia\|default>` | caelestia if `scheme.json` exists | theme colours `uColor1..3` (primary, secondary, tertiary), followed live |
| `--intensity <x>` | `1.0` | `uIntensity` |
| `--fps <n>` | `60` | cap while sound plays |
| `--idle-fps <n>` | `10` | rate after 2 s of silence; `0` stops drawing until sound returns |
| `--scale <x>` | `1.0` | render scale (`0.5` = quarter of the pixels, upscaled by the compositor via `wp_viewporter`) |
| `--layer <background\|bottom>` | `background` | `background` is opaque and composites the preset over the wallpaper |
| `--gpu <low\|high>` | `low` | adapter power preference (integrated vs discrete) |
| `--source <node>` | default output | capture another sink's monitor (`node.name` from `wpctl status`/`pw-cli ls Node`) |
| `--shaders <dir>` | see below | preset directory |
| `--stats` | off | log fps, render CPU time per frame and mode every 5 s |

Presets are searched in `--shaders`, `$XDG_DATA_HOME/woofer/shaders`, `/usr/share/woofer/shaders`,
then the source checkout's `shaders/`. Logging: `RUST_LOG=debug`.

## Architecture

- **`app.rs`**: smithay-client-toolkit, one Background-layer surface per output (hotplug, fractional scale via
  `wp_fractional_scale_v1`, render scale via `wp_viewporter`, empty input region). A frame is drawn only after the
  previous frame's `wl_surface.frame` callback, and only when due under the fps cap, so hidden or DPMS-off outputs
  cost nothing. After 2 s of silence it drops to `--idle-fps`; the capture thread pings the loop when sound returns.
  Outputs that show a fullscreen window are paused (Hyprland event socket; absent elsewhere).
- **`gpu.rs`**: wgpu on Vulkan, low-power adapter by default, one full-screen triangle, one pipeline per surface format.
  The wallpaper is decoded once, shrunk to what the largest output needs, uploaded with a CPU-built mip chain.
- **`shader.rs`**: wraps a preset for naga's GLSL frontend: std140 uniform block at set 0, separate `texture2D` and
  `sampler`, the contract's `spec()`, `specAt()` and `wall()` (same cover-fit maths as `tools/build.mjs`).
  On the background layer, a preset's alpha < 1 lets `wall()` show through, as the contract promises.
- **`audio.rs`**: PipeWire capture of the default sink's monitor as a *passive* node (it never keeps the sink awake),
  then an emulation of the web preview's WebAudio analyser (4096-point FFT, Blackman window, smoothing 0.5,
  −100…−30 dB → byte, 16 log bands 40 Hz–14 kHz, max per band, squared).
- **`analysis.rs`**: line-for-line port of `quickshell/analysis.js` (auto-gain, smoothing, band split, kick detection, `flow`).
- **`ctl.rs`**: one-line commands over `$XDG_RUNTIME_DIR/woofer.sock`. **`hypr.rs`**, **`wallpaper.rs`**:
  Hyprland fullscreen state and Caelestia files (inotify), each on a thread that sleeps until something happens.

`cargo build --profile measure --features count-alloc` (no LTO, quick) adds an allocation counter to `--stats`;
`bench/bench.sh` reproduces [BENCH.md](BENCH.md).
