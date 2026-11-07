use bytemuck::{Pod, Zeroable};

use crate::GlassParams;

/// Maximum number of glass shapes drawn in one frame. Extra shapes are ignored.
pub const MAX_SHAPES: usize = 16;

/// Width of the lensing band inside each shape's rim, in logical pixels.
/// Below this blur sigma (logical pixels) the glass shows the backdrop unblurred.
/// The shader uses the same threshold.
const MIN_BLUR_SIGMA: f32 = 0.25;

/// One glass shape in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassShape {
    pub center: [f32; 2],
    pub half_size: [f32; 2],
    /// Corner radius; clamped to the shorter half side, so a large value gives a capsule.
    pub radius: f32,
    /// 0 = light glass, 1 = dark glass. Values in between blend.
    pub appearance: f32,
}

/// Everything that changes per frame.
#[derive(Clone, Copy, Debug)]
pub struct Frame<'a> {
    /// Size of the target in logical pixels.
    pub viewport: [f32; 2],
    /// Physical pixels per logical pixel.
    pub scale_factor: f32,
    /// Backdrop texels per logical pixel.
    pub backdrop_scale: f32,
    /// Vertical backdrop offset in texels. The backdrop repeats vertically.
    pub scroll: f32,
    pub shapes: &'a [GlassShape],
    pub params: GlassParams,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    viewport: [f32; 2],
    backdrop_size: [f32; 2],
    scale_factor: f32,
    backdrop_scale: f32,
    scroll: f32,
    shape_count: u32,
    blur: f32,
    outline_light: f32,
    lensing: f32,
    rim: f32,
    fringe: f32,
    shadow: f32,
    merge_k: f32,
    outline_dark: f32,
    output_gamma: f32,
    bevel: f32,
    blur_lod: f32,
    _pad1: f32,
    /// offset, gain, curve, chroma
    body_light: [f32; 4],
    body_dark: [f32; 4],
    /// sharp gain and chroma, light then dark
    sharp: [f32; 4],
    /// outline at the ends (light, dark), highlight (light, dark)
    edges: [f32; 4],
    /// blend weight, light then dark; two unused
    blend: [f32; 4],
    shapes: [[f32; 4]; MAX_SHAPES],
    extra: [[f32; 4]; MAX_SHAPES],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BlurUniforms {
    step: [f32; 2],
    sigma: f32,
    to_gamma: f32,
}

/// Format of the blurred copies: gamma-encoded values, stored as they are.
const BLUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// Sigma, in texels of the chosen mip level, above which blurring moves to the
/// next smaller level. Three sigmas must stay under the shader's MAX_TAPS.
const MAX_LEVEL_SIGMA: f32 = 8.0;

/// Bind groups and targets for blurring at one mip level.
struct BlurLevel {
    horizontal: wgpu::BindGroup,
    vertical: wgpu::BindGroup,
    half_view: wgpu::TextureView,
    full_view: wgpu::TextureView,
    size: [u32; 2],
}

/// The content behind the glass, with a full mip chain, and space for the
/// blurred copy the glass reads its frost from.
pub struct Backdrop {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    blur_levels: Vec<BlurLevel>,
    size: [u32; 2],
}

impl Backdrop {
    /// The backdrop texture format. Render content into it, then call
    /// [`GlassRenderer::regenerate_mips`].
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn size(&self) -> [u32; 2] {
        self.size
    }
}

/// Draws glass shapes over a [`Backdrop`] into a target texture.
pub struct GlassRenderer {
    glass_pipeline: wgpu::RenderPipeline,
    blur_pipeline: wgpu::RenderPipeline,
    blur_layout: wgpu::BindGroupLayout,
    blur_buffers: [wgpu::Buffer; 2],
    glass_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    mip_pipeline: wgpu::RenderPipeline,
    mip_layout: wgpu::BindGroupLayout,
    mip_sampler: wgpu::Sampler,
    output_gamma: bool,
}

impl GlassRenderer {
    /// Creates the pipelines for drawing into textures of `target_format`.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let glass_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("embun glass"),
            source: wgpu::ShaderSource::Wgsl(include_str!("glass.wgsl").into()),
        });
        let blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("embun blur"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blur.wgsl").into()),
        });
        let mip_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("embun mip"),
            source: wgpu::ShaderSource::Wgsl(include_str!("mip.wgsl").into()),
        });

        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };

        let uniform_entry = |size: usize| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(size as u64),
            },
            count: None,
        };

        let glass_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("embun glass"),
            entries: &[uniform_entry(size_of::<Uniforms>()), texture_entry(1), sampler_entry(2), texture_entry(3)],
        });
        let blur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("embun blur"),
            entries: &[uniform_entry(size_of::<BlurUniforms>()), texture_entry(1), sampler_entry(2)],
        });
        let mip_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("embun mip"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });

        let glass_pipeline = fullscreen_pipeline(device, "embun glass", &glass_shader, &glass_layout, target_format);
        let mip_pipeline = fullscreen_pipeline(device, "embun mip", &mip_shader, &mip_layout, Backdrop::FORMAT);
        let blur_pipeline = fullscreen_pipeline(device, "embun blur", &blur_shader, &blur_layout, BLUR_FORMAT);
        let blur_buffers = ["embun blur horizontal", "embun blur vertical"].map(|label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size_of::<BlurUniforms>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("embun glass uniforms"),
            size: size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("embun backdrop"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let mip_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("embun mip"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            glass_pipeline,
            blur_pipeline,
            blur_layout,
            blur_buffers,
            glass_layout,
            uniform_buffer,
            sampler,
            mip_pipeline,
            mip_layout,
            mip_sampler,
            output_gamma: !target_format.is_srgb(),
        }
    }

    /// Uploads tightly packed RGBA8 (sRGB) pixels as a backdrop and builds its mips.
    ///
    /// # Panics
    /// If `rgba.len() != width * height * 4` or either side is zero.
    pub fn create_backdrop(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Backdrop {
        assert!(width > 0 && height > 0, "backdrop must not be empty");
        assert_eq!(rgba.len(), width as usize * height as usize * 4, "backdrop must be width * height * 4 bytes");

        let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("embun backdrop"),
            size,
            mip_level_count: size.max_mips(wgpu::TextureDimension::D2),
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Backdrop::FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(width * 4), rows_per_image: Some(height) },
            size,
        );

        // Two blurred copies with the backdrop's mip chain: the horizontal pass
        // writes "half", the vertical pass writes "full", at whichever level the
        // frame's blur needs.
        let blur_texture = |label| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: texture.mip_level_count(),
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: BLUR_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let half = blur_texture("embun blur half");
        let full = blur_texture("embun blur full");
        let level_view = |t: &wgpu::Texture, level| {
            t.create_view(&wgpu::TextureViewDescriptor { base_mip_level: level, mip_level_count: Some(1), ..Default::default() })
        };
        let blur_group = |buffer: &wgpu::Buffer, src: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("embun blur"),
                layout: &self.blur_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(src) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            })
        };
        let blur_levels = (0..texture.mip_level_count())
            .map(|level| {
                let half_view = level_view(&half, level);
                let full_view = level_view(&full, level);
                BlurLevel {
                    horizontal: blur_group(&self.blur_buffers[0], &level_view(&texture, level)),
                    vertical: blur_group(&self.blur_buffers[1], &half_view),
                    half_view,
                    full_view,
                    size: [(width >> level).max(1), (height >> level).max(1)],
                }
            })
            .collect();

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let full_all = full.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("embun glass"),
            layout: &self.glass_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&full_all) },
            ],
        });

        let backdrop = Backdrop { texture, bind_group, blur_levels, size: [width, height] };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("embun mips") });
        self.regenerate_mips(device, &mut encoder, &backdrop);
        queue.submit([encoder.finish()]);
        backdrop
    }

    /// Rebuilds mip levels 1.. from level 0. Call after drawing new content into the backdrop.
    pub fn regenerate_mips(&self, device: &wgpu::Device, encoder: &mut wgpu::CommandEncoder, backdrop: &Backdrop) {
        let levels = backdrop.texture.mip_level_count();
        let level_view = |level| {
            backdrop.texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        for level in 1..levels {
            let src = level_view(level - 1);
            let dst = level_view(level);
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("embun mip"),
                layout: &self.mip_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&src) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.mip_sampler) },
                ],
            });
            let mut pass = begin_pass(encoder, "embun mip", &dst);
            pass.set_pipeline(&self.mip_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }

    /// Draws the backdrop and the glass shapes into `target`, replacing its contents.
    ///
    /// Uses one set of uniform buffers, so submit the encoder before calling
    /// `render` again.
    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        backdrop: &Backdrop,
        frame: &Frame<'_>,
    ) {
        let mut uniforms = self.uniforms(backdrop, frame);
        uniforms.blur_lod = self.blur(queue, encoder, backdrop, frame);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        let mut pass = begin_pass(encoder, "embun glass", target);
        pass.set_pipeline(&self.glass_pipeline);
        pass.set_bind_group(0, &backdrop.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Blurs the backdrop for this frame's frost and returns the mip level the
    /// result is in. Does nothing when the frost is off.
    fn blur(&self, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, backdrop: &Backdrop, frame: &Frame<'_>) -> f32 {
        let sigma = frame.params.blur_sigma * frame.backdrop_scale;
        if frame.params.blur_sigma < MIN_BLUR_SIGMA || sigma <= 0.0 {
            return 0.0;
        }
        let last = backdrop.blur_levels.len() - 1;
        let level = ((sigma / MAX_LEVEL_SIGMA).log2().ceil().max(0.0) as usize).min(last);
        let blur = &backdrop.blur_levels[level];
        let level_sigma = sigma / (1u32 << level) as f32;
        let passes = [
            (&blur.horizontal, &blur.half_view, [1.0 / blur.size[0] as f32, 0.0], 1.0),
            (&blur.vertical, &blur.full_view, [0.0, 1.0 / blur.size[1] as f32], 0.0),
        ];
        for ((group, target, step, to_gamma), buffer) in passes.into_iter().zip(&self.blur_buffers) {
            let u = BlurUniforms { step, sigma: level_sigma, to_gamma };
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&u));
            let mut pass = begin_pass(encoder, "embun blur", target);
            pass.set_pipeline(&self.blur_pipeline);
            pass.set_bind_group(0, group, &[]);
            pass.draw(0..3, 0..1);
        }
        level as f32
    }

    fn uniforms(&self, backdrop: &Backdrop, frame: &Frame<'_>) -> Uniforms {
        let shapes = &frame.shapes[..frame.shapes.len().min(MAX_SHAPES)];
        let mut out = Uniforms::zeroed();
        for (i, s) in shapes.iter().enumerate() {
            out.shapes[i] = [s.center[0], s.center[1], s.half_size[0].max(0.0), s.half_size[1].max(0.0)];
            out.extra[i] = [s.radius.max(0.0), s.appearance.clamp(0.0, 1.0), 0.0, 0.0];
        }
        let p = frame.params;
        out.viewport = frame.viewport;
        out.backdrop_size = [backdrop.size[0] as f32, backdrop.size[1] as f32];
        out.scale_factor = frame.scale_factor.max(0.01);
        out.backdrop_scale = frame.backdrop_scale;
        out.scroll = frame.scroll.rem_euclid(backdrop.size[1] as f32);
        out.shape_count = shapes.len() as u32;
        out.blur = p.blur_sigma;
        out.outline_light = p.outline[0];
        out.outline_dark = p.outline[1];
        out.lensing = p.lensing;
        out.rim = p.rim;
        out.fringe = p.fringe;
        out.shadow = p.shadow;
        out.merge_k = p.merge_k;
        let body = |b: crate::Body| [b.offset, b.gain, b.curve, b.chroma];
        out.body_light = body(p.light);
        out.body_dark = body(p.dark);
        out.sharp = [p.light.sharp_gain, p.light.sharp_chroma, p.dark.sharp_gain, p.dark.sharp_chroma];
        out.edges = [p.outline_ends[0], p.outline_ends[1], p.highlight[0], p.highlight[1]];
        out.blend = [p.light.blend, p.dark.blend, 0.0, 0.0];
        out.output_gamma = if self.output_gamma { 1.0 } else { 0.0 };
        out.bevel = p.bevel.max(0.01);
        out
    }
}

fn fullscreen_pipeline(
    device: &wgpu::Device,
    label: &str,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[Some(layout)],
        ..Default::default()
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}

fn begin_pass<'e>(encoder: &'e mut wgpu::CommandEncoder, label: &str, view: &wgpu::TextureView) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
        })],
        ..Default::default()
    })
}
