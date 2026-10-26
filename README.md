<h1 align="center">Woofer</h1>

<p align="center">
  <img src="docs/hero.webp" alt="Woofer: a wallpaper glitching on each kick, seen through shattered glass, then a synthwave sunset, all driven by the music" width="800">
</p>

<p align="center"><b>Your wallpaper, but alive.</b><br>
A music-reactive wallpaper daemon for Wayland. Keep your favourite image: it breathes on the bass,
ripples on the beat, and drifts gently when it's quiet.</p>

<p align="center">
  <a href="https://ybucaille.github.io/woofer/">Website</a> ·
  <a href="#browser-preview">Browser preview</a> ·
  <a href="shaders/CONTRACT.md">Write an effect</a> ·
  <a href="https://ko-fi.com/pivuche">Support on Ko-fi</a>
</p>

---

## Features

- **Your own wallpaper, animated**: it breathes like a speaker cone, ripples on each kick, splits RGB for a blink,
  or shimmers in a heat haze. 5 wallpaper effects plus 8 generative backgrounds.
- **Any wlr-layer-shell compositor**: Hyprland, Sway, niri, river… (only Hyprland is tested so far, reports welcome).
- **Direct PipeWire capture**: no cava, no extra process. FFT, bass / mid / high, kick detection, 16-band spectrum.
- **Light**: draws only on frame callbacks, drops to 10 fps (or 0) after 2 s of silence, and pauses outputs that show
  a fullscreen window (Hyprland). Measured below.
- **Follows your theme**: with Caelestia, the wallpaper and Material You colours are followed live.
- **Live control**: `woofer ctl preset ripple`, no restart.
- **One-file effects**: a Shadertoy-style `.glsl` runs in the daemon, the Quickshell module and the browser preview.

## Performance

Measured on an i7-11800H laptop, Intel UHD, 1080p, preset `pulse` ([method and details](daemon/BENCH.md)):

| | CPU (% of one core) | RAM (RSS) |
|---|---|---|
| silence | 0.48 % (0 % with `--idle-fps 0`) | 29 MB |
| music, 60 fps | 3.0 % | 29 MB |
| music, 30 fps | 1.8 % | 29 MB |
| paused | 0.08 % | 29 MB |

About 12 MB of the RSS are the Vulkan driver and system libraries, which every Vulkan client maps.
A raw Vulkan backend to go under 1 % with music is on the [roadmap](ROADMAP.md).

## Install

**Prebuilt binary** (Linux x86_64), from the [latest release](https://github.com/Ybucaille/woofer/releases/latest):

```bash
curl -LO https://github.com/Ybucaille/woofer/releases/download/v0.1.0/woofer-v0.1.0-x86_64-linux.tar.gz
tar -xzf woofer-v0.1.0-x86_64-linux.tar.gz && ./woofer-v0.1.0-x86_64-linux/install.sh
```

It installs `~/.local/bin/woofer` and the presets in `~/.local/share/woofer/shaders`. It needs PipeWire and a
Vulkan driver (`vulkan-intel`, `vulkan-radeon` or `nvidia-utils` on Arch).

**From source** (AUR package planned):

```bash
# Arch: sudo pacman -S --needed rust clang pipewire vulkan-icd-loader  (+ vulkan-intel / vulkan-radeon / nvidia-utils)
git clone https://github.com/Ybucaille/woofer && cd woofer
cargo install --path daemon --locked
mkdir -p ~/.local/share/woofer && cp -r shaders ~/.local/share/woofer/
```

Requirements: a Wayland compositor with wlr-layer-shell, PipeWire, a Vulkan driver.

## Usage

```bash
woofer --wallpaper ~/Pictures/wall.jpg --preset woofer
woofer --wallpaper caelestia          # follow Caelestia's wallpaper and colours
woofer ctl preset ripple              # switch on the fly
woofer ctl intensity 0.6
woofer ctl pause | resume | status
```

Autostart: `exec-once = woofer --wallpaper ~/Pictures/wall.jpg` (Hyprland), `exec woofer …` (Sway).
If another tool already draws your wallpaper on the background layer (swww, hyprpaper, Caelestia), stop its
wallpaper or run Woofer with `--layer bottom`.

Main flags: `--preset`, `--intensity`, `--fps` (60), `--idle-fps` (10, `0` = stop in silence), `--scale` (render
scale), `--gpu low|high`, `--source <node>`. All of them: [daemon/README.md](daemon/README.md).

## Presets

| id | |
|---|---|
| **Wallpaper effects** | |
| `woofer` | Your wallpaper as a speaker cone: the bass pushes it out, every kick makes it ring |
| `ripple` | Each kick drops a stone in your wallpaper: one clean ring travels out from the centre and fades |
| `glitch` | Untouched until a kick lands: a brief RGB split and a few slipped slices, then calm |
| `drift` | A barely-there breeze moves through your wallpaper; the mids turn it into gusts |
| `heat` | Heat haze rising off your wallpaper, shimmering harder with the highs |
| **Over your wallpaper** | |
| `aurora` | Northern-light curtains rippling and brightening with the mids |
| `bokeh` | Out-of-focus lights, each one breathing with its own slice of the spectrum |
| `chrome` | Mirror-polished metal blobs reflecting your wallpaper, swelling on the bass |
| `shards` | Your wallpaper through faceted glass: kicks crack it open, highs glint on the bevels |
| **Generative** | |
| `nebula` | Slow cosmic clouds that bloom on every kick while stars catch the highs |
| `synthwave` | Retro sunset over a neon grid, with a mountain range drawn by the spectrum |
| `ribbon` | A single glowing silk ribbon whose waves are drawn by the spectrum |
| `pulse` | Soft concentric rings that breathe with the bass |

Try them all in the [browser preview](#browser-preview) with your own picture and music.

## Write your own effect

One file `shaders/<id>.glsl`:

```glsl
// @name My Effect
// @description One line shown in the picker
// @author you

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution;
    uv += (uv - 0.5) * -0.02 * uBass * uIntensity;   // push the picture out on the bass
    fragColor = vec4(wall(uv).rgb, 1.0);
}
```

Uniforms (`uBass`, `uBeat`, the spectrum, theme colours…), helpers and taste rules are in
**[shaders/CONTRACT.md](shaders/CONTRACT.md)**. `node tools/build.mjs --check` validates it, and the
[browser preview](#browser-preview) shows it. PRs welcome.

## Browser preview

Every preset also runs in the browser, with a demo beat, your mic or an audio file, your own wallpaper,
and a 9:16 recorder for short videos:

```bash
node tools/build.mjs
python -m http.server 4420 --bind 127.0.0.1    # then open http://127.0.0.1:4420/web/
```

## Quickshell / Caelestia module

The same presets as a Quickshell layer behind your windows, if you'd rather stay inside Quickshell
(needs `cava` and `qt6-shadertools`):

```bash
./install.sh                                         # builds the presets for Qt
qs -p quickshell/shell.qml
WOOFER_PRESET=nebula WOOFER_SCALE=0.75 qs -p quickshell/shell.qml
qs -p quickshell/shell.qml ipc call woofer setPreset ripple
```

`WOOFER_INTENSITY` sets reactivity, `WOOFER_COLORS=default` ignores the Caelestia colours.

## FAQ

**Wayland only?** Yes, it is a layer-shell client. GNOME and KDE are not targeted.

**NVIDIA?** It prefers the integrated GPU by default (`--gpu high` picks the discrete one). It is developed on Intel.
NVIDIA reports are welcome: open an issue with your driver version.

**Battery?** In silence it draws next to nothing, and `--idle-fps 0` stops drawing completely until sound returns.
`--fps 30` almost halves the cost with music.

**What does it listen to?** Your system output through PipeWire, as a passive node that never keeps the sink awake.
`--source` picks another sink.

## Support

Woofer is a free hobby project and will stay free. If it made your desktop a bit nicer and you'd like to say
thanks, you can [buy me a coffee on Ko-fi](https://ko-fi.com/pivuche). There's also a small premium pack, and commissions if
you'd like an effect made for you. A star, a screenshot or an issue helps just as much.

## Licence

[MIT](LICENSE). Made by [Ybucaille](https://github.com/Ybucaille).

Demo wallpaper in the animation above: "Cosmic Cliffs" in the Carina Nebula, James Webb Space Telescope.
Credit: NASA, ESA, CSA, and STScI ([CC BY 4.0](https://esawebb.org/copyright/)). Shown with Woofer's effects applied.
