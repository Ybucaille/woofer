//! Wayland side: one wlr-layer-shell surface per output, frame-callback-driven rendering, and the pacing policy.

use std::time::{Duration, Instant};

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, FrameCallbackData, Region},
    dispatch2::Dispatch2,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    },
};
use wayland_client::{
    Connection, Proxy, QueueHandle,
    protocol::{wl_output, wl_surface},
};
use wayland_protocols::wp::{
    fractional_scale::v1::client::{wp_fractional_scale_manager_v1 as fsm, wp_fractional_scale_v1 as fs},
    viewporter::client::{wp_viewport, wp_viewporter},
};

use crate::analysis;
use crate::audio::{Shared, Spectrum};
use crate::gpu::Gpu;
use crate::shader::{Frame, FrameBinding, Static};

/// Sound must have been absent this long before dropping to the idle rate.
const QUIET_AFTER: Duration = Duration::from_secs(2);
/// Render this early rather than arm a timer for a sliver of time.
const SLACK: Duration = Duration::from_millis(2);

pub struct Settings {
    pub fps: f32,
    pub idle_fps: f32,
    pub render_scale: f32,
    pub intensity: f32,
    pub opaque: bool,
    pub layer: Layer,
    pub stats: bool,
}

pub struct Out {
    // Field order is drop order: the wgpu surface must go before the wl_surface it wraps.
    surface: wgpu::Surface<'static>,
    config: Option<wgpu::SurfaceConfiguration>,
    frame_ubo: Option<wgpu::Buffer>,
    bind: wgpu::BindGroup,
    viewport: Option<wp_viewport::WpViewport>,
    fractional: Option<fs::WpFractionalScaleV1>,
    layer: LayerSurface,
    output: wl_output::WlOutput,
    pub name: String,
    logical: (u32, u32),
    /// Preferred scale ×120 (wp-fractional-scale units).
    scale120: u32,
    configured: bool,
    frame_pending: bool,
    needs_config: bool,
    dirty: bool,
    last_render: Option<Instant>,
    pub fullscreen: bool,
    frames: u64,
}

impl Out {
    fn ready(&self) -> bool {
        self.configured && !self.frame_pending
    }
}

pub struct App {
    pub conn: Connection,
    registry: RegistryState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    viewporter: Option<wp_viewporter::WpViewporter>,
    fractional: Option<fsm::WpFractionalScaleManagerV1>,
    pub gpu: Gpu,
    pub outputs: Vec<Out>,
    pub settings: Settings,
    pub audio: std::sync::Arc<Shared>,
    spectrum: Spectrum,
    raw: [f64; analysis::BANDS],
    pub analysis: analysis::State,
    pub colours: [[f32; 4]; 3],
    time: f64,
    last_advance: Option<Instant>,
    pub paused: bool,
    pub idle: bool,
    pub exit: bool,
    pub preset_id: String,
    stats_from: Instant,
    render_cpu: Duration,
    stats_frames: u64,
    #[cfg(feature = "count-alloc")]
    allocs_from: u64,
}

pub struct Globals {
    pub registry: RegistryState,
    pub output_state: OutputState,
    pub compositor: CompositorState,
    pub layer_shell: LayerShell,
    pub viewporter: Option<wp_viewporter::WpViewporter>,
    pub fractional: Option<fsm::WpFractionalScaleManagerV1>,
}

impl App {
    pub fn new(
        conn: Connection,
        g: Globals,
        gpu: Gpu,
        settings: Settings,
        audio: std::sync::Arc<Shared>,
        preset_id: String,
        colours: [[f32; 4]; 3],
    ) -> Self {
        App {
            conn,
            registry: g.registry,
            output_state: g.output_state,
            compositor: g.compositor,
            layer_shell: g.layer_shell,
            viewporter: g.viewporter,
            fractional: g.fractional,
            gpu,
            outputs: Vec::new(),
            settings,
            audio,
            spectrum: Spectrum::new(),
            raw: [0.0; analysis::BANDS],
            analysis: analysis::State::default(),
            colours,
            time: 0.0,
            last_advance: None,
            paused: false,
            idle: false,
            exit: false,
            preset_id,
            stats_from: Instant::now(),
            render_cpu: Duration::ZERO,
            stats_frames: 0,
            #[cfg(feature = "count-alloc")]
            allocs_from: crate::alloc_count::get(),
        }
    }

    /// Largest physical output size, used to downscale the wallpaper before upload.
    pub fn cover_size(&self) -> (u32, u32) {
        let mut best = (1920, 1080);
        let mut area = 0;
        for o in self.output_state.outputs() {
            let Some(info) = self.output_state.info(&o) else { continue };
            let Some(m) = info.modes.iter().find(|m| m.current).or(info.modes.first()) else { continue };
            let (mut w, mut h) = (m.dimensions.0.max(1) as u32, m.dimensions.1.max(1) as u32);
            if matches!(
                info.transform,
                wl_output::Transform::_90 | wl_output::Transform::_270 | wl_output::Transform::Flipped90 | wl_output::Transform::Flipped270
            ) {
                std::mem::swap(&mut w, &mut h);
            }
            if w * h > area {
                area = w * h;
                best = (w, h);
            }
        }
        best
    }

    /// Rebuild bind groups (after a wallpaper change) and redraw everything once.
    pub fn rebind(&mut self) {
        for o in &mut self.outputs {
            o.bind = self.gpu.bind_group(o.frame_ubo.as_ref());
        }
        self.update_static();
    }

    /// Re-uploads colours, intensity and wallpaper size, then redraws once.
    pub fn update_static(&mut self) {
        self.gpu.write_static(&Static {
            color1: self.colours[0],
            color2: self.colours[1],
            color3: self.colours[2],
            wallpaper_size: self.gpu.wall_size,
            intensity: self.settings.intensity,
            opaque: if self.settings.opaque { 1.0 } else { 0.0 },
        });
        self.redraw_all();
    }

    pub fn redraw_all(&mut self) {
        for o in &mut self.outputs {
            o.dirty = true;
        }
    }

    fn interval(&self) -> Option<Duration> {
        let fps = if self.idle { self.settings.idle_fps } else { self.settings.fps };
        (fps > 0.0 && !self.paused).then(|| Duration::from_secs_f32(1.0 / fps))
    }

    /// Renders whatever is due. Returns how long the event loop may sleep (None = until an event).
    pub fn tick(&mut self, qh: &QueueHandle<App>) -> Option<Duration> {
        let now = Instant::now();
        let quiet = self.audio.quiet_for(now) > QUIET_AFTER;
        if quiet != self.idle {
            self.idle = quiet;
            // Arm the capture thread to wake us on the first audible chunk.
            self.audio.idle.store(quiet, std::sync::atomic::Ordering::Release);
            log::debug!("{}", if quiet { "quiet: idle rate" } else { "sound: full rate" });
        }
        let interval = self.interval();
        let mut wait: Option<Duration> = None;
        for i in 0..self.outputs.len() {
            let o = &self.outputs[i];
            if !o.ready() {
                continue;
            }
            let due = if o.dirty {
                Some(now)
            } else if o.fullscreen {
                None
            } else {
                interval.map(|iv| o.last_render.map_or(now, |t| t + iv))
            };
            let Some(due) = due else { continue };
            if due <= now + SLACK {
                self.render(i, now, qh);
            } else {
                let w = due - now;
                wait = Some(wait.map_or(w, |x: Duration| x.min(w)));
            }
        }
        if self.settings.stats {
            self.stats(now);
        }
        // Stats need a wakeup even when nothing renders.
        if self.settings.stats {
            let left = Duration::from_secs(5).saturating_sub(now - self.stats_from);
            wait = Some(wait.map_or(left, |w| w.min(left)));
        }
        wait
    }

    fn advance(&mut self, now: Instant) {
        let dt = self.last_advance.map_or(1.0 / 60.0, |t| (now - t).as_secs_f64());
        // Several outputs rendering within the same couple of ms share one analysis step.
        if self.last_advance.is_some() && dt < 0.002 {
            return;
        }
        self.last_advance = Some(now);
        self.spectrum.read(&self.audio, now, &mut self.raw);
        self.analysis.analyse(&self.raw, dt);
        // Like Qt's FrameAnimation: time only runs while we render, a long pause does not jump.
        self.time += dt.min(0.25);
    }

    fn render(&mut self, i: usize, now: Instant, qh: &QueueHandle<App>) {
        let t0 = Instant::now();
        self.advance(now);
        let s = &self.analysis;
        let App { gpu, outputs, settings, compositor, viewporter, .. } = self;
        let o = &mut outputs[i];
        if o.needs_config {
            configure_surface(gpu, compositor, o, settings, viewporter.is_some());
        }
        let Some(cfg) = &o.config else { return };
        let mut spec = [[0.0f32; 4]; 4];
        for (k, v) in s.spec.iter().enumerate() {
            spec[k / 4][k % 4] = *v as f32;
        }
        let u = Frame {
            spec,
            resolution: [cfg.width as f32, cfg.height as f32],
            time: self.time as f32,
            flow: s.flow as f32,
            level: s.level as f32,
            bass: s.bass as f32,
            mid: s.mid as f32,
            high: s.high as f32,
            beat: s.beat as f32,
            _pad: [0.0; 3],
        };

        let wl = o.layer.wl_surface();
        wl.frame(qh, FrameCallbackData(wl.clone()));
        o.frame_pending = true;
        o.dirty = false;
        o.last_render = Some(now);
        let frame = match o.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => t,
            wgpu::CurrentSurfaceTexture::Suboptimal(t) => {
                o.needs_config = true;
                t
            }
            other => {
                log::debug!("{}: no surface texture ({other:?})", o.name);
                if matches!(other, wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost) {
                    o.needs_config = true;
                }
                // Commit anyway so the frame callback still arrives and we try again.
                wl.commit();
                return;
            }
        };
        let format = cfg.format;
        if let Some(ubo) = &o.frame_ubo {
            gpu.queue.write_buffer(ubo, 0, bytemuck::bytes_of(&u));
        }
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(gpu.pipeline(format));
            pass.set_bind_group(0, &o.bind, &[]);
            if gpu.frame_binding == FrameBinding::Immediates {
                pass.set_immediates(0, bytemuck::bytes_of(&u));
            }
            pass.draw(0..3, 0..1);
        }
        gpu.queue.submit([enc.finish()]);
        gpu.queue.present(frame);
        o.frames += 1;
        self.stats_frames += 1;
        self.render_cpu += t0.elapsed();
    }

    fn stats(&mut self, now: Instant) {
        let el = now - self.stats_from;
        if el < Duration::from_secs(5) {
            return;
        }
        let per = |n: u64| n as f64 / el.as_secs_f64();
        let fps: Vec<String> = self.outputs.iter().map(|o| format!("{} {:.1}", o.name, per(o.frames))).collect();
        let cpu_us = (self.render_cpu.as_micros() as u64).checked_div(self.stats_frames).unwrap_or(0);
        #[cfg(feature = "count-alloc")]
        let allocs = {
            let a = crate::alloc_count::get();
            let d = a - self.allocs_from;
            self.allocs_from = a;
            format!(", {:.1} allocs/frame", if self.stats_frames > 0 { d as f64 / self.stats_frames as f64 } else { d as f64 })
        };
        #[cfg(not(feature = "count-alloc"))]
        let allocs = "";
        log::info!(
            "stats: fps [{}] mode {} render-cpu {cpu_us} µs/frame level {:.2} beat {:.2}{allocs}",
            fps.join(", "),
            if self.paused { "paused" } else if self.idle { "idle" } else { "active" },
            self.analysis.level,
            self.analysis.beat,
        );
        for o in &mut self.outputs {
            o.frames = 0;
        }
        self.stats_frames = 0;
        self.render_cpu = Duration::ZERO;
        self.stats_from = now;
    }

    fn add_output(&mut self, qh: &QueueHandle<App>, output: wl_output::WlOutput) {
        let info = self.output_state.info(&output);
        let name = info.as_ref().and_then(|i| i.name.clone()).unwrap_or_else(|| format!("output-{}", output.id().protocol_id()));
        let scale = info.as_ref().map_or(1, |i| i.scale_factor.max(1)) as u32;
        let wl = self.compositor.create_surface(qh);
        let viewport = self.viewporter.as_ref().map(|v| v.get_viewport(&wl, qh, NoEvents));
        let fractional = self.fractional.as_ref().map(|f| f.get_fractional_scale(&wl, qh, FractionalData(wl.clone())));
        // Click-through: empty input region.
        if let Ok(r) = Region::new(&self.compositor) {
            wl.set_input_region(Some(r.wl_region()));
        }
        let raw_display = raw_window_handle::RawDisplayHandle::Wayland(raw_window_handle::WaylandDisplayHandle::new(
            std::ptr::NonNull::new(self.conn.backend().display_ptr() as *mut _).expect("display"),
        ));
        let raw_window = raw_window_handle::RawWindowHandle::Wayland(raw_window_handle::WaylandWindowHandle::new(
            std::ptr::NonNull::new(wl.id().as_ptr() as *mut _).expect("surface"),
        ));
        // SAFETY: the wl_surface and display outlive the wgpu surface (field order in `Out`, removal order below).
        let surface = match unsafe {
            self.gpu.instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(raw_display),
                raw_window_handle: raw_window,
            })
        } {
            Ok(s) => s,
            Err(e) => {
                log::error!("{name}: cannot create a Vulkan surface: {e}");
                return;
            }
        };
        let layer = self.layer_shell.create_layer_surface(qh, wl, self.settings.layer, Some("woofer"), Some(&output));
        layer.set_anchor(Anchor::all());
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_size(0, 0);
        layer.commit();
        let frame_ubo = self.gpu.frame_buffer();
        let bind = self.gpu.bind_group(frame_ubo.as_ref());
        log::info!("{name}: layer surface created");
        self.outputs.push(Out {
            surface,
            config: None,
            frame_ubo,
            bind,
            viewport,
            fractional,
            layer,
            output,
            name,
            logical: (0, 0),
            scale120: scale * 120,
            configured: false,
            frame_pending: false,
            needs_config: true,
            dirty: true,
            last_render: None,
            fullscreen: false,
            frames: 0,
        });
    }

    fn remove_at(&mut self, i: usize) {
        let o = self.outputs.remove(i);
        log::info!("{}: removed", o.name);
        let Out { surface, viewport, fractional, layer, .. } = o;
        drop(surface);
        if let Some(v) = viewport {
            v.destroy();
        }
        if let Some(f) = fractional {
            f.destroy();
        }
        drop(layer);
    }

    fn find(&self, s: &wl_surface::WlSurface) -> Option<usize> {
        self.outputs.iter().position(|o| o.layer.wl_surface() == s)
    }

    fn set_scale(&mut self, s: &wl_surface::WlSurface, scale120: u32) {
        if let Some(i) = self.find(s) {
            let o = &mut self.outputs[i];
            if o.scale120 != scale120 {
                o.scale120 = scale120;
                o.needs_config = true;
                o.dirty = true;
            }
        }
    }
}

fn configure_surface(gpu: &Gpu, compositor: &CompositorState, o: &mut Out, settings: &Settings, has_viewport: bool) {
    o.needs_config = false;
    let (lw, lh) = o.logical;
    if lw == 0 || lh == 0 {
        return;
    }
    let (w, h) = if has_viewport {
        let k = o.scale120 as f64 / 120.0 * settings.render_scale as f64;
        (((lw as f64 * k).round() as u32).max(1), ((lh as f64 * k).round() as u32).max(1))
    } else {
        // No viewporter: integer buffer scale only, render scale unsupported.
        let s = (o.scale120 / 120).max(1);
        o.layer.wl_surface().set_buffer_scale(s as i32);
        (lw * s, lh * s)
    };
    if let Some(v) = &o.viewport {
        v.set_destination(lw as i32, lh as i32);
    }
    let caps = o.surface.get_capabilities(&gpu.adapter);
    if caps.formats.is_empty() {
        log::error!("{}: surface not supported by this adapter", o.name);
        return;
    }
    // Non-sRGB target: presets write gamma-space values directly, like WebGL and Qt.
    // 8-bit first: a 16-bit float swapchain doubles the bandwidth for nothing.
    let format = [wgpu::TextureFormat::Bgra8Unorm, wgpu::TextureFormat::Rgba8Unorm]
        .into_iter()
        .find(|f| caps.formats.contains(f))
        .or_else(|| caps.formats.iter().copied().find(|f| !f.is_srgb()))
        .unwrap_or(caps.formats[0]);
    let want_alpha = if settings.opaque { wgpu::CompositeAlphaMode::Opaque } else { wgpu::CompositeAlphaMode::PreMultiplied };
    let alpha_mode = if caps.alpha_modes.contains(&want_alpha) { want_alpha } else { caps.alpha_modes[0] };
    // We pace ourselves with frame callbacks, so never let present block: mailbox if possible.
    let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else {
        wgpu::PresentMode::Fifo
    };
    let Some(mut cfg) = o.surface.get_default_config(&gpu.adapter, w, h) else { return };
    cfg.format = format;
    cfg.alpha_mode = alpha_mode;
    cfg.present_mode = present_mode;
    cfg.desired_maximum_frame_latency = 1;
    o.surface.configure(&gpu.device, &cfg);
    if settings.opaque
        && let Ok(r) = Region::new(compositor) {
            r.add(0, 0, lw as i32, lh as i32);
            o.layer.wl_surface().set_opaque_region(Some(r.wl_region()));
        }
    log::info!(
        "{}: {lw}×{lh} logical, scale {:.2}, rendering {w}×{h} {format:?} {present_mode:?} {alpha_mode:?}",
        o.name,
        o.scale120 as f64 / 120.0
    );
    o.config = Some(cfg);
}

impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, s: &wl_surface::WlSurface, factor: i32) {
        // Integer scale is only a fallback when wp-fractional-scale is missing.
        if self.fractional.is_none() {
            self.set_scale(s, factor.max(1) as u32 * 120);
        }
    }
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, s: &wl_surface::WlSurface, _: u32) {
        if let Some(i) = self.find(s) {
            self.outputs[i].frame_pending = false;
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.add_output(qh, output);
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        let name = self.output_state.info(&output).and_then(|i| i.name);
        if let Some(o) = self.outputs.iter_mut().find(|o| o.output == output)
            && let Some(n) = name {
                o.name = n;
            }
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if let Some(i) = self.outputs.iter().position(|o| o.output == output) {
            self.remove_at(i);
        }
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(i) = self.find(layer.wl_surface()) {
            self.remove_at(i);
        }
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, c: LayerSurfaceConfigure, _: u32) {
        let Some(i) = self.find(layer.wl_surface()) else { return };
        let o = &mut self.outputs[i];
        if c.new_size != o.logical {
            o.logical = c.new_size;
            o.needs_config = true;
        }
        o.configured = true;
        o.dirty = true;
    }
}

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState];
}

smithay_client_toolkit::delegate_registry!(App);
smithay_client_toolkit::delegate_dispatch2!(App);

/// User data for protocol objects that send no events we care about.
pub struct NoEvents;

impl<I: Proxy> Dispatch2<I, App> for NoEvents {
    fn event(&self, _: &mut App, _: &I, _: I::Event, _: &Connection, _: &QueueHandle<App>) {}
}

pub struct FractionalData(wl_surface::WlSurface);

impl Dispatch2<fs::WpFractionalScaleV1, App> for FractionalData {
    fn event(&self, app: &mut App, _: &fs::WpFractionalScaleV1, e: fs::Event, _: &Connection, _: &QueueHandle<App>) {
        if let fs::Event::PreferredScale { scale } = e {
            app.set_scale(&self.0, scale.max(1));
        }
    }
}
