# Benchmarks

Honest numbers, measured on the author's laptop. Reproduce with `bench/bench.sh` (method below).

## Machine

i7-11800H (8C/16T), 16 GB, Intel UHD (TGL GT1) used for rendering (RTX 3060 Mobile present, not used),
Arch Linux, kernel 7.2, Mesa 26.2.5 (ANV, i915), Hyprland 0.56, PipeWire 1.6, one 1920×1080 output at scale 1,
on AC power. Preset `pulse`, wallpaper a 2338×3308 JPEG (downscaled to 1920×2717 to cover the screen).
Release build (fat LTO), 2026-10-26.

## Results (defaults: `--fps 60 --idle-fps 10`)

Median of three runs of 15 s per state; the three runs were within 0.2 percentage points of each other.
CPU is % of one core, all threads of the daemon included.

| state | what is happening | CPU | RSS | PSS |
|---|---|---|---|---|
| silence | nothing playing, the sink is suspended | **0.48 %** | 28.6 MB | 21.1 MB |
| silent stream | an app keeps a stream open but plays zeros | **0.69 %** | 29.1 MB | 21.3 MB |
| music | 124 bpm kick / bass / hats loop, 60 fps | **3.03 %** | 29.1 MB | 21.3 MB |
| paused | `ctl pause` while the music keeps playing | **0.08 %** | 29.1 MB | 21.5 MB |

Variants (one run each, same method):

| flags | silence | silent stream | music | paused |
|---|---|---|---|---|
| `--idle-fps 0` (stop drawing in silence) | **0.00 %** | 0.23 % | 3.12 % | 0.08 % |
| `--fps 30` | 0.47 % | 0.71 % | **1.76 %** | 0.08 % |
| `--scale 0.5` (960×540 render) | 0.51 % | 0.73 % | 3.14 % | 0.07 % |

`--scale` saves GPU time, not CPU: the CPU cost is per frame, not per pixel. The same goes for a fullscreen window
on Hyprland, which pauses that output like `ctl pause` (not benchmarked separately, it is the same code path).

## Where the time goes

At 60 fps the main thread spends about 0.33 ms of CPU per frame (thread CPU clock, instrumented build). About 40 %
of that is kernel time (i915 execbuffer, Wayland socket):

| step | µs / frame |
|---|---|
| audio: copy ring, 4096-point real FFT, 16 bands, analysis | ~30 |
| `get_current_texture` (Mesa WSI acquire) | ~50 |
| encode render pass | ~10 |
| `queue.submit` (wgpu-core tracking + ANV submit) | ~170 |
| `queue.present` (Mesa WSI, Wayland commit) | ~46 |

The rest of the 3 % is the event loop, Wayland dispatch, frame callbacks and the PipeWire thread (~0.3 %).
Paused, the 0.08 % is that PipeWire thread still receiving audio. In silence with the sink suspended it gets nothing,
because the capture stream is a passive node.

The frame inputs (time, audio, spectrum: 112 bytes) go as push constants. Our original uniform-buffer
`write_buffer` path cost another ~70 µs plus a staging buffer per frame.

## Memory

29 MB RSS splits into ~10 MB anonymous (heap, stacks, driver state) and ~19 MB of clean, shared file pages:
the daemon binary (6.0 MB resident), `libvulkan_intel` (4.4 MB), libc / libstdc++ (3.3 MB), and smaller libraries.
PSS, which divides shared pages between processes, is 21 MB.

The biggest win was not loading unused Vulkan drivers. By default the loader maps every installed ICD: lavapipe
pulls in all of libLLVM and the idle NVIDIA driver adds the rest, for **136 MB RSS**. The daemon hides software
rasterizers (and NVIDIA when preferring low power) unless the user set `VK_*` driver variables. That brought it
down to 29 MB.

Allocations (`--features count-alloc`): our code makes 0 heap allocations per frame. The Wayland frame callback
object costs 3 and wgpu internals 83 per frame (encoder, pass, views, submission tracking). These are Rust-side
counts only; C libraries (Mesa, PipeWire) are not counted.

## Against the goals

| goal | measured | |
|---|---|---|
| ~0.3 % CPU | 0.48 % silent (0.00 % with `--idle-fps 0`), 3.0 % with music at 60 fps, 1.8 % at 30 fps | met only in silence with `--idle-fps 0` |
| < 15 MB RAM | 29 MB RSS, 21 MB PSS, ~10 MB anonymous | not met; ~12 MB of it is the Vulkan driver and system libraries any Vulkan client maps |

Getting the music case near 1 % at 60 fps would need dropping wgpu for raw Vulkan (`ash`): wgpu-core's
per-submit tracking is the largest single item above. That is a large rewrite, not a tweak.

## Method

`bench/bench.sh [seconds-per-phase] -- [daemon flags]`:

1. Creates a temporary PipeWire null sink `rd-bench` and starts the daemon with `--audio-target rd-bench --stats`
   under `timeout` (it is always killed at the end, and the sink destroyed). Sound goes to that sink, so the test
   is inaudible and unaffected by whatever else is playing.
2. Waits 6 s (startup, then 2 s of silence so the daemon reaches its idle rate), then measures four phases:
   nothing playing; `pw-play` of a silent WAV; `pw-play` of the beat from `bench/make-beat.py` (same recipe as
   the web preview's demo beat); and `ctl pause` with the beat still playing.
3. CPU: the sum over threads of `/proc/<pid>/task/*/schedstat` (on-CPU ns) at the phase start and end, divided by
   wall time. RSS from `/proc/<pid>/status`, sampled each second (the maximum was within 0.2 MB of the end value in every run).
   PSS and private memory from `/proc/<pid>/smaps_rollup` at the phase end.

Caveats: it is a laptop with frequency scaling. Runs made while builds were going on, or right after, read up to
2× higher, so the table comes from runs on an otherwise idle machine (load average 1.5–4 from desktop apps).
GPU time is not measured (no `intel_gpu_top` access); `pulse` is a light shader.
