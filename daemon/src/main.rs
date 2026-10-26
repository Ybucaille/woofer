mod analysis;
mod app;
mod audio;
mod ctl;
mod gpu;
mod hypr;
mod shader;
mod wallpaper;

use std::path::PathBuf;
use std::process::ExitCode;

use smithay_client_toolkit::{
    compositor::CompositorState,
    output::OutputState,
    reexports::{
        calloop::{self, EventLoop, channel},
        calloop_wayland_source::WaylandSource,
    },
    registry::RegistryState,
    shell::wlr_layer::{Layer, LayerShell},
};
use wayland_client::{Connection, globals::registry_queue_init};

use app::{App, Globals, NoEvents, Settings};
use wallpaper::Source;

#[cfg(feature = "count-alloc")]
pub mod alloc_count {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicU64, Ordering};

    static ALLOCS: AtomicU64 = AtomicU64::new(0);

    pub struct Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            unsafe { System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            unsafe { System.realloc(p, l, n) }
        }
    }

    #[global_allocator]
    static A: Counting = Counting;

    pub fn get() -> u64 {
        ALLOCS.load(Ordering::Relaxed)
    }
}

/// Give freed heap pages (e.g. a decoded wallpaper) back to the kernel so RSS reflects what we keep.
pub fn trim_heap() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    // SAFETY: glibc's malloc_trim has no preconditions.
    unsafe {
        malloc_trim(0);
    }
}

const USAGE: &str = "\
Woofer: your wallpaper, but alive (audio-reactive layer-shell wallpaper)

USAGE:
    woofer [OPTIONS]
    woofer ctl preset <id> | intensity <x> | pause | resume | status
    woofer snapshot <out.png> [--preset <id>] [--wallpaper <path>]   (offscreen test frame)

OPTIONS:
    --preset <id>          shaders/<id>.glsl to run [default: pulse]
    --wallpaper <src>      image path, `caelestia` (follow Caelestia's wallpaper) or `none` [default: none]
    --colors <src>         `caelestia` (follow scheme.json) or `default` [default: caelestia if present]
    --intensity <x>        reactivity multiplier [default: 1.0]
    --fps <n>              frame-rate cap while sound plays [default: 60]
    --idle-fps <n>         frame rate after 2 s of silence; 0 stops until sound returns [default: 10]
    --scale <x>            render scale, upscaled by the compositor [default: 1.0]
    --layer <l>            `background` (opaque, draws the wallpaper) or `bottom` (transparent overlay) [default: background]
    --gpu <p>              `low` (integrated) or `high` (discrete) power preference [default: low]
    --source <node>  capture this sink's monitor (node.name or serial) instead of the default output
    --shaders <dir>        preset directory [default: XDG data dir, /usr/share, or the source checkout]
    --stats                log fps, render CPU time and mode every 5 s
    -h, --help             this text
";

struct Args {
    preset: String,
    wallpaper: Source,
    colors: Option<bool>,
    intensity: f32,
    fps: f32,
    idle_fps: f32,
    scale: f32,
    layer: Layer,
    high_power: bool,
    shaders: Option<PathBuf>,
    source: Option<String>,
    stats: bool,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        preset: "pulse".into(),
        wallpaper: Source::None,
        colors: None,
        intensity: 1.0,
        fps: 60.0,
        idle_fps: 10.0,
        scale: 1.0,
        layer: Layer::Background,
        high_power: false,
        shaders: None,
        source: None,
        stats: false,
    };
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut val = || it.next().cloned().ok_or_else(|| format!("{flag} needs a value"));
        let num = |v: String, lo: f32, hi: f32| {
            v.parse::<f32>()
                .ok()
                .filter(|x| (lo..=hi).contains(x))
                .ok_or_else(|| format!("{flag}: expected a number in {lo}..={hi}, got {v:?}"))
        };
        match flag.as_str() {
            "--preset" => a.preset = val()?,
            "--wallpaper" => a.wallpaper = Source::parse(&val()?),
            "--colors" | "--colours" => {
                a.colors = Some(match val()?.as_str() {
                    "caelestia" => true,
                    "default" => false,
                    v => return Err(format!("--colors: expected caelestia or default, got {v:?}")),
                })
            }
            "--intensity" => a.intensity = num(val()?, 0.0, 10.0)?,
            "--fps" => a.fps = num(val()?, 1.0, 500.0)?,
            "--idle-fps" => a.idle_fps = num(val()?, 0.0, 500.0)?,
            "--scale" => a.scale = num(val()?, 0.1, 2.0)?,
            "--layer" => {
                a.layer = match val()?.as_str() {
                    "background" => Layer::Background,
                    "bottom" => Layer::Bottom,
                    v => return Err(format!("--layer: expected background or bottom, got {v:?}")),
                }
            }
            "--gpu" => {
                a.high_power = match val()?.as_str() {
                    "low" => false,
                    "high" => true,
                    v => return Err(format!("--gpu: expected low or high, got {v:?}")),
                }
            }
            "--shaders" => a.shaders = Some(PathBuf::from(val()?)),
            "--source" => a.source = Some(val()?),
            "--stats" => a.stats = true,
            _ => return Err(format!("unknown argument {flag:?}\n\n{USAGE}")),
        }
    }
    Ok(a)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.first().map(String::as_str) == Some("ctl") {
        return ExitCode::from(ctl::client(&args[1..]) as u8);
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"))
        .init();
    if args.first().map(String::as_str) == Some("snapshot") {
        return match snapshot(&args[1..]) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                log::error!("{e}");
                ExitCode::FAILURE
            }
        };
    }
    match parse_args(&args).and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            log::error!("{e}");
            ExitCode::FAILURE
        }
    }
}

enum Event {
    Ctl(ctl::Request),
    Fullscreen(hypr::Fullscreen),
    Changed(wallpaper::Change),
}

fn run(a: Args) -> Result<(), String> {
    let shader_dir = shader::find_dir(a.shaders.as_deref()).ok_or("no shader directory found (use --shaders)")?;

    let conn = Connection::connect_to_env().map_err(|e| format!("wayland: {e}"))?;
    let (globals, mut queue) = registry_queue_init::<App>(&conn).map_err(|e| format!("wayland: {e}"))?;
    let qh = queue.handle();
    let g = Globals {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        compositor: CompositorState::bind(&globals, &qh).map_err(|e| format!("wl_compositor: {e}"))?,
        layer_shell: LayerShell::bind(&globals, &qh)
            .map_err(|e| format!("this compositor has no wlr-layer-shell ({e}); GNOME is not supported"))?,
        viewporter: globals.bind(&qh, 1..=1, NoEvents).ok(),
        fractional: globals.bind(&qh, 1..=1, NoEvents).ok(),
    };

    let mut gpu = make_gpu(a.high_power)?;
    let fb = gpu.frame_binding;
    let preset = shader::load(&shader_dir, &a.preset, fb)?;
    log::info!("preset {} from {}", preset.id, shader_dir.display());
    gpu.set_preset(&preset);
    drop(preset);

    let follow_theme = a.colors.unwrap_or_else(|| wallpaper::caelestia_dir().join("scheme.json").is_file());
    let colours = follow_theme.then(wallpaper::caelestia_colours).flatten().unwrap_or(wallpaper::DEFAULT_COLOURS);

    let shared = audio::Shared::new();
    let settings = Settings {
        fps: a.fps,
        idle_fps: a.idle_fps,
        render_scale: a.scale,
        intensity: a.intensity,
        opaque: a.layer == Layer::Background,
        layer: a.layer,
        stats: a.stats,
    };
    let mut app = App::new(conn.clone(), g, gpu, settings, shared.clone(), a.preset.clone(), colours);
    queue.roundtrip(&mut app).map_err(|e| format!("wayland: {e}"))?;
    if app.outputs.is_empty() {
        log::warn!("no outputs yet; waiting for one");
    }
    load_wallpaper(&mut app, &a.wallpaper);

    let mut event_loop: EventLoop<App> = EventLoop::try_new().map_err(|e| e.to_string())?;
    let handle = event_loop.handle();
    WaylandSource::new(conn.clone(), queue).insert(handle.clone()).map_err(|e| e.to_string())?;

    // Before any thread starts, so they all inherit the blocked mask and the loop alone receives these.
    let signals = calloop::signals::Signals::new(&[calloop::signals::Signal::SIGTERM, calloop::signals::Signal::SIGINT])
        .map_err(|e| e.to_string())?;
    handle.insert_source(signals, |_, _, app| app.exit = true).map_err(|e| e.to_string())?;

    let (ping, ping_source) = calloop::ping::make_ping().map_err(|e| e.to_string())?;
    // Just waking the loop is enough: tick() re-reads the audio state.
    handle.insert_source(ping_source, |_, _, _| {}).map_err(|e| e.to_string())?;
    shared.set_waker(move || ping.ping());
    audio::spawn_capture(shared, a.source.clone());

    let (tx, rx) = channel::channel::<Event>();
    let wall_src = a.wallpaper.clone();
    handle
        .insert_source(rx, move |ev, _, app: &mut App| {
            let channel::Event::Msg(ev) = ev else { return };
            match ev {
                Event::Ctl((cmd, reply)) => {
                    let _ = reply.send(handle_ctl(app, &shader_dir, cmd));
                }
                Event::Fullscreen(map) => {
                    for o in &mut app.outputs {
                        let full = map.get(&o.name).copied().unwrap_or(false);
                        if full != o.fullscreen {
                            log::info!("{}: {}", o.name, if full { "fullscreen window, pausing" } else { "visible again" });
                            o.fullscreen = full;
                        }
                    }
                }
                Event::Changed(wallpaper::Change::Wallpaper) => load_wallpaper(app, &wall_src),
                Event::Changed(wallpaper::Change::Theme) => {
                    if let Some(c) = wallpaper::caelestia_colours() {
                        app.colours = c;
                        app.update_static();
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;

    let t = tx.clone();
    ctl::serve(move |r| {
        let _ = t.send(Event::Ctl(r));
    })?;
    let t = tx.clone();
    if hypr::spawn(move |m| {
        let _ = t.send(Event::Fullscreen(m));
    }) {
        log::info!("hyprland: pausing outputs that show a fullscreen window");
    }
    let t = tx;
    wallpaper::watch(a.wallpaper == Source::Caelestia, follow_theme, move |c| {
        let _ = t.send(Event::Changed(c));
    });

    let result = loop {
        let timeout = app.tick(&qh);
        if let Err(e) = conn.flush() {
            break Err(format!("wayland: {e}"));
        }
        if let Err(e) = event_loop.dispatch(timeout, &mut app) {
            break Err(format!("event loop: {e}"));
        }
        if app.exit {
            log::info!("exiting");
            break Ok(());
        }
        if app.outputs.is_empty() && conn.protocol_error().is_some() {
            break Err("wayland protocol error".into());
        }
    };
    ctl::remove_socket();
    result
}

/// `snapshot <out.png> [--preset id] [--wallpaper path] [--shaders dir]`: one offscreen 960×540 frame with fixed,
/// mid-song audio values, to check a preset (and the wrapper) without touching the desktop.
fn snapshot(args: &[String]) -> Result<(), String> {
    let (out, rest) = args.split_first().ok_or("snapshot <out.png> [--preset id] [--wallpaper path]")?;
    let a = parse_args(rest)?;
    let dir = shader::find_dir(a.shaders.as_deref()).ok_or("no shader directory found (use --shaders)")?;
    let mut gpu = make_gpu(a.high_power)?;
    let preset = shader::load(&dir, &a.preset, gpu.frame_binding)?;
    gpu.set_preset(&preset);
    let (w, h) = (960, 540);
    let img = a.wallpaper.resolve().map(|p| wallpaper::load(&p, (w, h))).transpose()?;
    gpu.set_wallpaper(img);
    let colours = wallpaper::DEFAULT_COLOURS;
    gpu.write_static(&shader::Static {
        color1: colours[0],
        color2: colours[1],
        color3: colours[2],
        wallpaper_size: gpu.wall_size,
        intensity: a.intensity,
        opaque: if a.layer == Layer::Background { 1.0 } else { 0.0 },
    });
    let mut spec = [[0.0f32; 4]; 4];
    for k in 0..16 {
        spec[k / 4][k % 4] = 0.8 * (1.0 - k as f32 / 16.0);
    }
    let frame = shader::Frame {
        spec,
        resolution: [w as f32, h as f32],
        time: 12.0,
        flow: 9.0,
        level: 0.5,
        bass: 0.7,
        mid: 0.4,
        high: 0.25,
        beat: 0.6,
        _pad: [0.0; 3],
    };
    gpu.snapshot(&frame, w, h)?.save(out).map_err(|e| format!("{out}: {e}"))?;
    log::info!("wrote {out}");
    Ok(())
}

fn new_instance() -> wgpu::Instance {
    wgpu::Instance::new(
        wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            flags: wgpu::InstanceFlags::empty(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        }
        .with_env(),
    )
}

/// The Vulkan loader maps every installed driver into the process, even unused ones: lavapipe pulls in all of
/// libLLVM (~40 MB RSS) and an idle NVIDIA driver another ~55 MB. Unless the user chose drivers themselves, hide
/// the ones we will not pick (software rasterizers; NVIDIA when preferring low power) and skip implicit layers.
/// If that leaves no usable adapter, retry with everything.
fn make_gpu(high_power: bool) -> Result<gpu::Gpu, String> {
    const USER_VARS: &[&str] =
        &["VK_LOADER_DRIVERS_SELECT", "VK_LOADER_DRIVERS_DISABLE", "VK_ICD_FILENAMES", "VK_DRIVER_FILES", "VK_ADD_DRIVER_FILES"];
    if USER_VARS.iter().all(|v| std::env::var_os(v).is_none()) {
        let disable = if high_power { "*lvp*,*swiftshader*" } else { "*lvp*,*swiftshader*,*nvidia*" };
        let layers = std::env::var_os("VK_LOADER_LAYERS_DISABLE").is_none() && std::env::var_os("VK_INSTANCE_LAYERS").is_none();
        // SAFETY: called before any other thread is started.
        unsafe {
            std::env::set_var("VK_LOADER_DRIVERS_DISABLE", disable);
            if layers {
                std::env::set_var("VK_LOADER_LAYERS_DISABLE", "~implicit~");
            }
        }
        match gpu::Gpu::new(new_instance(), high_power) {
            Ok(g) => return Ok(g),
            Err(e) => log::info!("{e} with drivers {disable} hidden; retrying with all drivers"),
        }
        // SAFETY: as above.
        unsafe {
            std::env::remove_var("VK_LOADER_DRIVERS_DISABLE");
            if layers {
                std::env::remove_var("VK_LOADER_LAYERS_DISABLE");
            }
        }
    }
    gpu::Gpu::new(new_instance(), high_power)
}

fn load_wallpaper(app: &mut App, src: &Source) {
    let img = src.resolve().and_then(|p| {
        let t = std::time::Instant::now();
        match wallpaper::load(&p, app.cover_size()) {
            Ok(img) => {
                log::info!("wallpaper {} ({}×{}, {:?})", p.display(), img.width(), img.height(), t.elapsed());
                Some(img)
            }
            Err(e) => {
                log::warn!("wallpaper: {e}");
                None
            }
        }
    });
    if img.is_none() && *src != Source::None {
        log::warn!("wallpaper unavailable, using a neutral background");
    }
    app.gpu.set_wallpaper(img);
    app.rebind();
}

fn handle_ctl(app: &mut App, dir: &std::path::Path, cmd: ctl::Command) -> String {
    use ctl::Command::*;
    match cmd {
        Preset(id) => match shader::load(dir, &id, app.gpu.frame_binding) {
            Ok(p) => {
                app.gpu.set_preset(&p);
                app.preset_id = id;
                app.redraw_all();
                "ok".into()
            }
            Err(e) => format!("error: {e}"),
        },
        Intensity(x) => {
            app.settings.intensity = x;
            app.update_static();
            "ok".into()
        }
        Pause => {
            app.paused = true;
            "ok".into()
        }
        Resume => {
            app.paused = false;
            app.redraw_all();
            "ok".into()
        }
        Status => {
            let outs: Vec<String> = app
                .outputs
                .iter()
                .map(|o| format!("{}{}", o.name, if o.fullscreen { " (fullscreen)" } else { "" }))
                .collect();
            format!(
                "preset {} intensity {} mode {} level {:.2} outputs [{}]",
                app.preset_id,
                app.settings.intensity,
                if app.paused { "paused" } else if app.idle { "idle" } else { "active" },
                app.analysis.level,
                outs.join(", ")
            )
        }
    }
}
