//! Wraps a `shaders/<id>.glsl` preset (see shaders/CONTRACT.md) into Vulkan-flavoured GLSL for naga.
//! Uniform names and helpers must stay identical to tools/build.mjs so presets behave the same on every host.

use std::path::{Path, PathBuf};

/// Per-frame values. Sent as push constants (wgpu "immediates") when the GPU has them, else as a second UBO.
/// Field order differs from build.mjs only to pack tightly; names are what count.
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Frame {
    pub spec: [[f32; 4]; 4],
    pub resolution: [f32; 2],
    pub time: f32,
    pub flow: f32,
    pub level: f32,
    pub bass: f32,
    pub mid: f32,
    pub high: f32,
    pub beat: f32,
    pub _pad: [f32; 3],
}

/// Values that change only on a theme, intensity or wallpaper change (UBO at binding 0).
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Static {
    pub color1: [f32; 4],
    pub color2: [f32; 4],
    pub color3: [f32; 4],
    pub wallpaper_size: [f32; 2],
    pub intensity: f32,
    /// Daemon-only: 1.0 composites the preset over the wallpaper and outputs opaque pixels.
    pub opaque: f32,
}

const FRAME_MEMBERS: &str = r"
    vec4 uSpec0;
    vec4 uSpec1;
    vec4 uSpec2;
    vec4 uSpec3;
    vec2 iResolution;
    float iTime;
    float uFlow;
    float uLevel;
    float uBass;
    float uMid;
    float uHigh;
    float uBeat;
};
";

const PRELUDE: &str = r"
layout(set = 0, binding = 0, std140) uniform RdStatic {
    vec4 uColor1;
    vec4 uColor2;
    vec4 uColor3;
    vec2 iWallpaperSize;
    float uIntensity;
    float rd_Opaque;
};
layout(set = 0, binding = 1) uniform texture2D rd_WallTex;
layout(set = 0, binding = 2) uniform sampler rd_WallSampler;
layout(location = 0) out vec4 rd_FragColor;

float spec(int i) {
    i = clamp(i, 0, 15);
    vec4 v = i < 4 ? uSpec0 : i < 8 ? uSpec1 : i < 12 ? uSpec2 : uSpec3;
    return v[i & 3];
}
float specAt(float x) {
    float f = clamp(x, 0.0, 1.0) * 15.0;
    int i = int(floor(f));
    return mix(spec(i), spec(i + 1), fract(f));
}
vec4 wall(vec2 uv) {
    float sa = iResolution.x / max(iResolution.y, 1.0);
    float wa = iWallpaperSize.x / max(iWallpaperSize.y, 1.0);
    vec2 s = sa > wa ? vec2(1.0, wa / sa) : vec2(sa / wa, 1.0);
    vec2 t = (uv - 0.5) * s + 0.5;
    return texture(sampler2D(rd_WallTex, rd_WallSampler), vec2(t.x, 1.0 - t.y));
}
";

const EPILOGUE: &str = r"
void main() {
    // Vulkan's gl_FragCoord is top-left; presets expect Shadertoy's bottom-left origin.
    vec2 fc = vec2(gl_FragCoord.x, iResolution.y - gl_FragCoord.y);
    vec4 c = vec4(0.0);
    mainImage(c, fc);
    if (rd_Opaque > 0.5) {
        float a = clamp(c.a, 0.0, 1.0);
        if (a < 1.0) c = c + wall(fc / iResolution) * (1.0 - a);
        c.a = 1.0;
    }
    rd_FragColor = c;
}
";

pub const VERTEX_WGSL: &str = r"
@vertex
fn main(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
}
";

/// How the per-frame block reaches the shader.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FrameBinding {
    Immediates,
    Uniform,
}

pub fn wrap(body: &str, fb: FrameBinding) -> String {
    let mut s = String::with_capacity(1024 + PRELUDE.len() + body.len() + EPILOGUE.len());
    s.push_str("#version 450\n");
    s.push_str(match fb {
        FrameBinding::Immediates => "layout(push_constant) uniform RdFrame {",
        FrameBinding::Uniform => "layout(set = 0, binding = 3, std140) uniform RdFrame {",
    });
    s.push_str(FRAME_MEMBERS);
    s.push_str(PRELUDE);
    s.push_str(body);
    s.push_str(EPILOGUE);
    s
}

fn prelude_lines() -> usize {
    wrap("", FrameBinding::Uniform).lines().count() - EPILOGUE.lines().count()
}

/// Parses and validates a preset. Errors name preset-relative line numbers.
pub fn compile(body: &str, fb: FrameBinding) -> Result<naga::Module, String> {
    let src = wrap(body, fb);
    let opts = naga::front::glsl::Options::from(naga::ShaderStage::Fragment);
    let module = naga::front::glsl::Frontend::default()
        .parse(&opts, &src)
        .map_err(|e| relines(&e.emit_to_string(&src)))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::IMMEDIATES)
        .validate(&module)
        .map_err(|e| relines(&e.emit_to_string(&src)))?;
    Ok(module)
}

/// naga reports wrapped-source lines; shift them so they point into the preset file.
fn relines(msg: &str) -> String {
    format!("{msg}(line numbers include a {}-line wrapper prelude)", prelude_lines())
}

pub struct Preset {
    pub id: String,
    pub module: naga::Module,
}

pub fn find_dir(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    let mut dirs = Vec::new();
    if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(d).join("woofer/shaders"));
    } else if let Some(h) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(h).join(".local/share/woofer/shaders"));
    }
    dirs.push(PathBuf::from("/usr/share/woofer/shaders"));
    // Development checkout: daemon/../shaders.
    dirs.push(PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../shaders")));
    dirs.into_iter().find(|d| d.is_dir())
}

pub fn load(dir: &Path, id: &str, fb: FrameBinding) -> Result<Preset, String> {
    if id.is_empty() || id.contains(['/', '.']) {
        return Err(format!("invalid preset id {id:?}"));
    }
    let path = dir.join(format!("{id}.glsl"));
    let body = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let module = compile(&body, fb).map_err(|e| format!("{id}: {e}"))?;
    Ok(Preset { id: id.to_owned(), module })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_sizes() {
        assert_eq!(std::mem::size_of::<Frame>(), 112);
        assert_eq!(std::mem::size_of::<Static>(), 64);
    }

    /// Every preset in shaders/ must go through the wrapper and validate, in both bindings.
    #[test]
    fn compile_all_presets() {
        let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../shaders"));
        let mut ids: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok()?.file_name().into_string().ok()?.strip_suffix(".glsl").map(str::to_owned))
            .collect();
        ids.sort();
        assert!(!ids.is_empty());
        let mut failures = Vec::new();
        for id in &ids {
            let r: Vec<_> = [FrameBinding::Immediates, FrameBinding::Uniform].map(|fb| load(&dir, id, fb).err()).into();
            let errs: Vec<String> = r.into_iter().flatten().collect();
            eprintln!("{} {id}", if errs.is_empty() { "ok  " } else { "FAIL" });
            failures.extend(errs);
        }
        assert!(failures.is_empty(), "{} failure(s):\n{}", failures.len(), failures.join("\n"));
    }

    #[test]
    fn block_offsets_match_structs() {
        let m = compile("void mainImage(out vec4 f, in vec2 c) { f = vec4(iTime + uIntensity + uBeat); }", FrameBinding::Immediates)
            .unwrap();
        let off = |block: &str, n: &str| {
            let (_, ty) = m.types.iter().find(|(_, t)| t.name.as_deref() == Some(block)).unwrap();
            let naga::TypeInner::Struct { members, .. } = &ty.inner else { panic!() };
            members.iter().find(|m| m.name.as_deref() == Some(n)).unwrap().offset
        };
        assert_eq!(off("RdFrame", "iResolution"), 64);
        assert_eq!(off("RdFrame", "iTime"), 72);
        assert_eq!(off("RdFrame", "uBeat"), 96);
        assert_eq!(off("RdStatic", "iWallpaperSize"), 48);
        assert_eq!(off("RdStatic", "rd_Opaque"), 60);
    }
}
