# Roadmap

Ideas and planned work, roughly in order. Open an issue to discuss any of them.

## Next

- **Packages**: AUR (`woofer-git`, then a tagged release), then Nix.
- **More compositors tested**: Sway, niri, river, Wayfire. Only Hyprland is tested so far.
- **Fullscreen pause beyond Hyprland**: today the per-output fullscreen pause uses Hyprland's event socket; other
  compositors rely on frame callbacks stopping. `wlr-foreign-toplevel` could cover the rest.
- **Caelestia integration**: an option in Caelestia's background instead of a separate layer.
- **Presets from equations**: generate an effect from a math formula.
- **Media-aware effects**: react to track changes (MPRIS), not just the spectrum.

## Later: raw Vulkan backend

The daemon renders through wgpu. With music playing at 60 fps it costs about 3 % of one CPU core
(see [daemon/BENCH.md](daemon/BENCH.md)). The largest single item is wgpu-core's per-submit tracking,
~170 µs of the ~330 µs spent per frame. A small backend on raw Vulkan (`ash`) could:

- record the one full-screen draw once and replay it, with per-frame inputs as push constants;
- skip wgpu's resource tracking and validation entirely;
- target **< 1 % CPU at 60 fps** with music, against 3 % today.

Scope: only `daemon/src/gpu.rs` and the surface setup in `app.rs` change. Presets stay GLSL; naga, or
shaderc / glslang, still produces the SPIR-V. Keep wgpu as the default until the Vulkan path matches it on Intel,
AMD and NVIDIA, then ship Vulkan behind a `--backend vulkan` flag and a cargo feature.

It is not planned for the first releases. The current numbers (0 % in silence with `--idle-fps 0`,
~3 % of a core with music, 29 MB RSS) are fine for a wallpaper, and the rewrite is a large job.
