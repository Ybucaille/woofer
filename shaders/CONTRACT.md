# Shader preset contract

A preset is one file `shaders/<id>.glsl` containing a Shadertoy-style body.
`tools/build.mjs` wraps it twice:

- **Qt / Quickshell**: GLSL 440 + `qsb` → `quickshell/shaders/<id>.frag.qsb` (used by `quickshell/ReactiveShader.qml`)
- **Web**: GLSL ES 3.00 → `web/shaders.generated.js` (used by the browser preview in `web/`)

So a preset must compile as both GLSL 440 and GLSL ES 3.00: no `texture()` on undeclared samplers,
no non-constant loop bounds, no `double`, no recursion, write `0.0` not `0`.

## Header

```glsl
// @name Nebula Drift
// @description One line, shown in the preset picker
// @author pivuche
```

## Entry point

```glsl
void mainImage(out vec4 fragColor, in vec2 fragCoord)
```

`fragCoord` is in pixels, origin bottom-left (same as Shadertoy).
**Output premultiplied alpha** (`vec4(rgb * a, a)`). `a < 1` lets the wallpaper show through.

## Uniforms (always available, never redeclare them)

| name | type | meaning |
|---|---|---|
| `iTime` | float | seconds since start |
| `iResolution` | vec2 | pixels |
| `uFlow` | float | music-driven time: grows faster when loud. Prefer it over `iTime` for motion, it never jitters |
| `uLevel` | float | overall loudness 0..1 (smoothed, auto-gained) |
| `uBass` `uMid` `uHigh` | float | band energies 0..1 |
| `uBeat` | float | 1.0 on a detected kick, decays to 0 in ~0.4 s |
| `uSpec0..uSpec3` | vec4 | 16 log-spaced spectrum bins 0..1 (low → high) |
| `uColor1` `uColor2` `uColor3` | vec4 | theme colours (primary, secondary, accent), straight alpha |
| `uIntensity` | float | user multiplier for reactivity, default 1.0 |

Helpers provided by the wrapper:

```glsl
float spec(int i);      // bin 0..15
float specAt(float x);  // x in 0..1, linear interpolation across bins
```

## Taste rules

- Desktop background first: it must look good **silent** (slow idle motion) and not be tiring for hours.
- React to music with restraint; scale everything by `uIntensity`.
- Use the theme colours so it matches the user's Material You palette (Caelestia).
- Cheap: target < 2 ms/frame at 1080p on an integrated GPU. Max ~64 loop iterations, no raymarching with 100+ steps.
