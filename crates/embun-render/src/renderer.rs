use bytemuck::{Pod, Zeroable};

use crate::GlassParams;

/// Maximum number of glass shapes drawn in one frame. Extra shapes are ignored.
pub const MAX_SHAPES: usize = 16;

/// Width of the lensing band inside each shape's rim, in logical pixels.
const BEVEL: f32 = 18.0;

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
    _pad1: [f32; 2],
    /// offset, gain, curve, chroma
    body_light: [f32; 4],
    body_dark: [f32; 4],
    shapes: [[f32; 4]; MAX_SHAPES],
    extra: [[f32; 4]; MAX_SHAPES],
}

/// The content behind the glass, with a full mip chain for frosting.
pub struct Backdrop {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
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

        let glass_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("embun glass"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Uniforms>() as u64),
                    },
                    count: None,
                },
                texture_entry(1),
                sampler_entry(2),
            ],
        });
        let mip_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("embun mip"),
            entries: &[texture_entry(0), sampler_entry(1)],
        });

        let glass_pipeline = fullscreen_pipeline(device, "embun glass", &glass_shader, &glass_layout, target_format);
        let mip_pipeline = fullscreen_pipeline(device, "embun mip", &mip_shader, &mip_layout, Backdrop::FORMAT);

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

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("embun glass"),
            layout: &self.glass_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });

        let backdrop = Backdrop { texture, bind_group, size: [width, height] };
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
    /// Uses one uniform buffer, so submit the encoder before calling `render` again.
    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        backdrop: &Backdrop,
        frame: &Frame<'_>,
    ) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&self.uniforms(backdrop, frame)));
        let mut pass = begin_pass(encoder, "embun glass", target);
        pass.set_pipeline(&self.glass_pipeline);
        pass.set_bind_group(0, &backdrop.bind_group, &[]);
        pass.draw(0..3, 0..1);
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
        out.output_gamma = if self.output_gamma { 1.0 } else { 0.0 };
        out.bevel = BEVEL;
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
