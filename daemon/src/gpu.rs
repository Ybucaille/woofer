//! wgpu device, per-format pipelines for the current preset, and the wallpaper texture.

use std::borrow::Cow;

use crate::shader::{self, Frame, FrameBinding, Preset, Static};

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    bgl: wgpu::BindGroupLayout,
    layout: wgpu::PipelineLayout,
    vertex: wgpu::ShaderModule,
    fragment: Option<wgpu::ShaderModule>,
    sampler: wgpu::Sampler,
    static_ubo: wgpu::Buffer,
    pub frame_binding: FrameBinding,
    pub wall_view: wgpu::TextureView,
    pub wall_size: [f32; 2],
    /// One pipeline per surface format in use, rebuilt when the preset changes.
    pipelines: Vec<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
}

impl Gpu {
    pub fn new(instance: wgpu::Instance, high_power: bool) -> Result<Self, String> {
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: if high_power {
                wgpu::PowerPreference::HighPerformance
            } else {
                wgpu::PowerPreference::LowPower
            },
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| format!("no Vulkan adapter: {e}"))?;
        let info = adapter.get_info();
        log::info!("gpu: {} ({:?}, {})", info.name, info.device_type, info.driver);
        let frame_size = std::mem::size_of::<Frame>() as u32;
        // Push constants skip a staging buffer + copy per frame, which is most of wgpu's per-frame CPU cost.
        let frame_binding = if adapter.features().contains(wgpu::Features::IMMEDIATES)
            && adapter.limits().max_immediate_size >= frame_size
        {
            FrameBinding::Immediates
        } else {
            FrameBinding::Uniform
        };
        let immediates = frame_binding == FrameBinding::Immediates;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("woofer"),
            required_features: if immediates { wgpu::Features::IMMEDIATES } else { wgpu::Features::empty() },
            required_limits: wgpu::Limits {
                max_immediate_size: if immediates { frame_size } else { 0 },
                ..wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits())
            },
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            ..Default::default()
        }))
        .map_err(|e| format!("request_device: {e}"))?;

        let uniform = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let mut entries = vec![
            uniform(0),
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ];
        if !immediates {
            entries.push(uniform(3));
        }
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: if immediates { frame_size } else { 0 },
        });
        let vertex = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fullscreen-triangle"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(shader::VERTEX_WGSL)),
        });
        log::info!("gpu: per-frame data via {frame_binding:?}");
        let static_ubo = buffer(&device, std::mem::size_of::<Static>());
        // Same sampling as the web preview: trilinear, mirrored repeat.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::MirrorRepeat,
            address_mode_v: wgpu::AddressMode::MirrorRepeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let (wall_view, wall_size) = upload_wallpaper(&device, &queue, None);
        Ok(Gpu {
            instance,
            adapter,
            device,
            queue,
            bgl,
            layout,
            vertex,
            fragment: None,
            sampler,
            static_ubo,
            frame_binding,
            wall_view,
            wall_size,
            pipelines: Vec::new(),
        })
    }

    fn fragment_module(device: &wgpu::Device, preset: &Preset) -> wgpu::ShaderModule {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&preset.id),
            source: wgpu::ShaderSource::Naga(Cow::Owned(preset.module.clone())),
        })
    }

    pub fn set_preset(&mut self, preset: &Preset) {
        self.fragment = Some(Self::fragment_module(&self.device, preset));
        let formats: Vec<_> = self.pipelines.drain(..).map(|(f, _)| f).collect();
        for f in formats {
            self.pipeline(f);
        }
    }

    /// Returns the pipeline for `format`, building it on first use.
    pub fn pipeline(&mut self, format: wgpu::TextureFormat) -> &wgpu::RenderPipeline {
        if let Some(i) = self.pipelines.iter().position(|(f, _)| *f == format) {
            return &self.pipelines[i].1;
        }
        let p = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&self.layout),
            vertex: wgpu::VertexState {
                module: &self.vertex,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: self.fragment.as_ref().expect("set_preset before drawing"),
                entry_point: Some("main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        self.pipelines.push((format, p));
        &self.pipelines.last().unwrap().1
    }

    /// Per-output frame UBO, only when push constants are unavailable.
    pub fn frame_buffer(&self) -> Option<wgpu::Buffer> {
        (self.frame_binding == FrameBinding::Uniform).then(|| buffer(&self.device, std::mem::size_of::<Frame>()))
    }

    pub fn write_static(&self, s: &Static) {
        self.queue.write_buffer(&self.static_ubo, 0, bytemuck::bytes_of(s));
    }

    pub fn bind_group(&self, frame: Option<&wgpu::Buffer>) -> wgpu::BindGroup {
        let mut entries = vec![
            wgpu::BindGroupEntry { binding: 0, resource: self.static_ubo.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.wall_view) },
            wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
        ];
        if let Some(f) = frame {
            entries.push(wgpu::BindGroupEntry { binding: 3, resource: f.as_entire_binding() });
        }
        self.device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout: &self.bgl, entries: &entries })
    }

    /// Uploads the wallpaper with a full CPU-built mip chain (once), or a 1×1 neutral texel when `None`.
    /// Callers must rebuild bind groups afterwards.
    pub fn set_wallpaper(&mut self, img: Option<image::RgbaImage>) {
        (self.wall_view, self.wall_size) = upload_wallpaper(&self.device, &self.queue, img);
        crate::trim_heap();
    }
}

impl Gpu {
    /// Renders one frame offscreen (Rgba8Unorm, `w` a multiple of 64) and reads it back. For `snapshot`.
    pub fn snapshot(&mut self, frame: &Frame, w: u32, h: u32) -> Result<image::RgbaImage, String> {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let read = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (w * h * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let frame_ubo = self.frame_buffer();
        if let Some(f) = &frame_ubo {
            self.queue.write_buffer(f, 0, bytemuck::bytes_of(frame));
        }
        let bind = self.bind_group(frame_ubo.as_ref());
        let view = tex.create_view(&Default::default());
        let immediates = self.frame_binding == FrameBinding::Immediates;
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let pipeline = self.pipeline(format).clone();
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
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind, &[]);
            if immediates {
                pass.set_immediates(0, bytemuck::bytes_of(frame));
            }
            pass.draw(0..3, 0..1);
        }
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([enc.finish()]);
        read.map_async(wgpu::MapMode::Read, .., |_| {});
        self.device
            .poll(wgpu::PollType::Wait { submission_index: None, timeout: None })
            .map_err(|e| format!("poll: {e}"))?;
        let data = read.get_mapped_range(..).map_err(|e| format!("map: {e:?}"))?.to_vec();
        image::RgbaImage::from_raw(w, h, data).ok_or_else(|| "readback size".into())
    }
}

fn buffer(device: &wgpu::Device, size: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: size as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn upload_wallpaper(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    img: Option<image::RgbaImage>,
) -> (wgpu::TextureView, [f32; 2]) {
    {
        let img = img.unwrap_or_else(|| image::RgbaImage::from_pixel(1, 1, image::Rgba([16, 16, 20, 255])));
        let (w, h) = img.dimensions();
        let levels = 32 - w.max(h).leading_zeros();
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wallpaper"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Not sRGB: like WebGL / Qt, presets work on the stored (gamma-encoded) values.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut level = img;
        for mip in 0..levels {
            let (lw, lh) = level.dimensions();
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: mip,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &level,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * lw), rows_per_image: Some(lh) },
                wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
            );
            if mip + 1 < levels {
                level = image::imageops::resize(
                    &level,
                    (lw / 2).max(1),
                    (lh / 2).max(1),
                    image::imageops::FilterType::Triangle,
                );
            }
        }
        drop(level);
        queue.submit([]);
        (tex.create_view(&Default::default()), [w as f32, h as f32])
    }
}
