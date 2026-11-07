//! Draws a [`Scene`] with wgpu: content into the glass backdrop, the glass,
//! then whatever sits on the glass.

use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use embun_render::{Backdrop, Frame, GlassRenderer};
use glyphon::{Cache, ColorMode, Resolution, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer, Viewport};

use crate::Theme;
use crate::layout::Rect4;
use crate::scene::{Prim, Scene, content_hash};
use crate::text::TextSystem;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    size: [f32; 2],
    output_gamma: f32,
    _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Quad {
    rect: [f32; 4],
    params: [f32; 4],
    top: [f32; 4],
    bottom: [f32; 4],
    border: [f32; 4],
    clip: [f32; 4],
}

/// Pipelines for one target format.
struct Layer {
    rects: wgpu::RenderPipeline,
    images: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    output_gamma: bool,
    atlas: TextAtlas,
    viewport: Viewport,
    text: Vec<TextRenderer>,
}

/// One run of a layer, drawn in order.
enum Batch {
    Quads(std::ops::Range<u32>),
    Image(u64, u32),
    Text(usize),
}

struct GpuImage {
    group: wgpu::BindGroup,
    used: u64,
}

/// Draws scenes into a window surface or any texture of the format it was
/// created for.
pub struct Renderer {
    glass: GlassRenderer,
    backdrop: Option<Backdrop>,
    backdrop_view: Option<wgpu::TextureView>,
    content_hash: Option<u64>,
    content: Layer,
    foreground: Layer,
    image_layout: wgpu::BindGroupLayout,
    image_sampler: wgpu::Sampler,
    images: HashMap<u64, GpuImage>,
    swash: SwashCache,
    quad_buffer: Option<wgpu::Buffer>,
    frame: u64,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("embun quads"),
            source: wgpu::ShaderSource::Wgsl(include_str!("quad.wgsl").into()),
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("embun globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(size_of::<Globals>() as u64),
                },
                count: None,
            }],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("embun image"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let cache = Cache::new(device);
        let layer = |format: wgpu::TextureFormat| {
            let globals = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("embun globals"),
                size: size_of::<Globals>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("embun globals"),
                layout: &globals_layout,
                entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
            });
            let pipeline = |entry: &str, layouts: &[Option<&wgpu::BindGroupLayout>]| {
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(entry),
                    bind_group_layouts: layouts,
                    ..Default::default()
                });
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs"),
                        buffers: &[Some(wgpu::VertexBufferLayout {
                            array_stride: size_of::<Quad>() as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4],
                        })],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(entry),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                })
            };
            let output_gamma = !format.is_srgb();
            let color_mode = if output_gamma { ColorMode::Web } else { ColorMode::Accurate };
            Layer {
                rects: pipeline("fs_rect", &[Some(&globals_layout)]),
                images: pipeline("fs_image", &[Some(&globals_layout), Some(&image_layout)]),
                globals,
                globals_group,
                output_gamma,
                atlas: TextAtlas::with_color_mode(device, queue, &cache, format, color_mode),
                viewport: Viewport::new(device, &cache),
                text: Vec::new(),
            }
        };
        Self {
            glass: GlassRenderer::new(device, format),
            backdrop: None,
            backdrop_view: None,
            content_hash: None,
            content: layer(Backdrop::FORMAT),
            foreground: layer(format),
            image_layout,
            image_sampler: device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("embun image"),
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
            images: HashMap::new(),
            swash: SwashCache::new(),
            quad_buffer: None,
            frame: 0,
        }
    }

    /// Draws `scene` into `target`, which is `size` physical pixels for a
    /// window `scale_factor` physical pixels per logical pixel.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: [u32; 2],
        scale_factor: f32,
        scene: &Scene,
        theme: &Theme,
        text: &mut TextSystem,
    ) {
        self.frame += 1;
        let size = [size[0].max(1), size[1].max(1)];
        if self.backdrop.as_ref().map(|b| b.size()) != Some(size) {
            let blank = vec![0u8; size[0] as usize * size[1] as usize * 4];
            let backdrop = self.glass.create_backdrop(device, queue, &blank, size[0], size[1]);
            self.backdrop_view = Some(backdrop.texture().create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: 0,
                mip_level_count: Some(1),
                ..Default::default()
            }));
            self.backdrop = Some(backdrop);
            self.content_hash = None;
        }

        // Every quad of both layers goes into one instance buffer.
        let mut quads = Vec::new();
        let hash = content_hash(scene);
        let redraw_content = self.content_hash != Some(hash);
        let content_batches = if redraw_content {
            Some(self.prepare(device, queue, Which::Content, &scene.content, scale_factor, size, text, &mut quads))
        } else {
            None
        };
        let foreground_batches =
            self.prepare(device, queue, Which::Foreground, &scene.foreground, scale_factor, size, text, &mut quads);
        self.upload_quads(device, queue, &quads);

        let backdrop = self.backdrop.as_ref().expect("backdrop");
        if let Some(batches) = content_batches {
            let background = scene.background.unwrap_or(theme.background()).linear_premultiplied();
            let clear = wgpu::Color {
                r: background[0] as f64,
                g: background[1] as f64,
                b: background[2] as f64,
                a: background[3] as f64,
            };
            {
                let view = self.backdrop_view.as_ref().expect("backdrop view");
                let mut pass = begin(encoder, view, wgpu::LoadOp::Clear(clear));
                draw(&mut pass, &self.content, &batches, self.quad_buffer.as_ref(), &self.images);
            }
            self.glass.regenerate_mips(encoder, backdrop);
            self.content_hash = Some(hash);
        }

        let logical = [size[0] as f32 / scale_factor, size[1] as f32 / scale_factor];
        let frame = Frame {
            viewport: logical,
            scale_factor,
            backdrop_scale: scale_factor,
            scroll: 0.0,
            shapes: &scene.glass,
            params: theme.glass(),
        };
        self.glass.render(queue, encoder, target, backdrop, &frame);
        {
            let mut pass = begin(encoder, target, wgpu::LoadOp::Load);
            draw(&mut pass, &self.foreground, &foreground_batches, self.quad_buffer.as_ref(), &self.images);
        }
        self.content.atlas.trim();
        self.foreground.atlas.trim();
        let frame = self.frame;
        self.images.retain(|_, image| frame - image.used < 120);
        text.end_frame();
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        which: Which,
        prims: &[Prim],
        scale: f32,
        size: [u32; 2],
        text: &mut TextSystem,
        quads: &mut Vec<Quad>,
    ) -> Vec<Batch> {
        let frame = self.frame;
        for prim in prims {
            if let Prim::Image { image, .. } = prim {
                let entry = self.images.entry(image.id).or_insert_with(|| GpuImage {
                    group: upload_image(device, queue, &self.image_layout, &self.image_sampler, image),
                    used: frame,
                });
                entry.used = frame;
            }
        }
        let layer = match which {
            Which::Content => &mut self.content,
            Which::Foreground => &mut self.foreground,
        };
        let globals = Globals { size: [size[0] as f32, size[1] as f32], output_gamma: layer.output_gamma as u32 as f32, _pad: 0.0 };
        queue.write_buffer(&layer.globals, 0, bytemuck::bytes_of(&globals));
        layer.viewport.update(queue, Resolution { width: size[0], height: size[1] });

        let px = |r: Rect4| [r[0] * scale, r[1] * scale, r[2] * scale, r[3] * scale];
        let mut batches = Vec::new();
        let mut text_runs: Vec<Vec<usize>> = Vec::new();
        for (i, prim) in prims.iter().enumerate() {
            match prim {
                Prim::Rect { rect, radius, top, bottom, border, clip } => {
                    let start = quads.len() as u32;
                    let (bw, bc) = border.map_or((0.0, [0.0; 4]), |(w, c)| (w * scale, c.linear_premultiplied()));
                    quads.push(Quad {
                        rect: px(*rect),
                        params: [radius.min(1e6) * scale, bw, 0.0, 0.0],
                        top: top.linear_premultiplied(),
                        bottom: bottom.linear_premultiplied(),
                        border: bc,
                        clip: px(*clip),
                    });
                    match batches.last_mut() {
                        Some(Batch::Quads(range)) if range.end == start => range.end += 1,
                        _ => batches.push(Batch::Quads(start..start + 1)),
                    }
                }
                Prim::Image { image, rect, radius, clip } => {
                    let index = quads.len() as u32;
                    quads.push(Quad {
                        rect: px(*rect),
                        params: [radius.min(1e6) * scale, 0.0, 0.0, 0.0],
                        top: [0.0; 4],
                        bottom: [0.0; 4],
                        border: [0.0; 4],
                        clip: px(*clip),
                    });
                    batches.push(Batch::Image(image.id, index));
                }
                Prim::Text { .. } => match batches.last() {
                    Some(Batch::Text(run)) if *run == text_runs.len() - 1 => text_runs.last_mut().expect("run").push(i),
                    _ => {
                        batches.push(Batch::Text(text_runs.len()));
                        text_runs.push(vec![i]);
                    }
                },
            }
        }

        while layer.text.len() < text_runs.len() {
            layer.text.push(TextRenderer::new(&mut layer.atlas, device, wgpu::MultisampleState::default(), None));
        }
        for (run, renderer) in text_runs.iter().zip(&mut layer.text) {
            // Shape everything first, then borrow the buffers for drawing.
            for &i in run {
                if let Prim::Text { text: s, style, width, .. } = &prims[i] {
                    text.buffer(s, style, Some(*width));
                }
            }
            let (fonts, buffers) = text.split();
            let areas: Vec<TextArea> = run
                .iter()
                .filter_map(|&i| match &prims[i] {
                    Prim::Text { text: s, style, width, origin, color, clip } => {
                        let clip = px(*clip);
                        Some(TextArea {
                            buffer: buffers.get(s, style, Some(*width))?,
                            left: origin[0] * scale,
                            top: origin[1] * scale,
                            scale,
                            bounds: TextBounds {
                                left: clip[0].max(-1e6) as i32,
                                top: clip[1].max(-1e6) as i32,
                                right: (clip[0] + clip[2]).min(1e6) as i32,
                                bottom: (clip[1] + clip[3]).min(1e6) as i32,
                            },
                            default_color: glyphon::Color::rgba(
                                (color.r * 255.0).round() as u8,
                                (color.g * 255.0).round() as u8,
                                (color.b * 255.0).round() as u8,
                                (color.a * 255.0).round() as u8,
                            ),
                            custom_glyphs: &[],
                        })
                    }
                    _ => None,
                })
                .collect();
            if let Err(e) = renderer.prepare(device, queue, fonts, &mut layer.atlas, &layer.viewport, areas, &mut self.swash) {
                // The atlas is full or a glyph failed: skip this text for the frame.
                eprintln!("embun: text not drawn this frame: {e}");
            }
        }
        batches
    }

    fn upload_quads(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, quads: &[Quad]) {
        if quads.is_empty() {
            return;
        }
        let bytes = bytemuck::cast_slice(quads);
        if self.quad_buffer.as_ref().is_none_or(|b| b.size() < bytes.len() as u64) {
            self.quad_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("embun quads"),
                size: (bytes.len() as u64).next_power_of_two().max(4096),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(self.quad_buffer.as_ref().expect("quad buffer"), 0, bytes);
    }
}

/// The format [`snapshot`] renders in. Create the renderer with this format.
pub const SNAPSHOT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Renders `ui` into memory and returns tightly packed sRGB RGBA8 pixels
/// with the width and height: for tests, golden images and screenshots.
/// `renderer` must be created with [`SNAPSHOT_FORMAT`].
pub fn snapshot<A: crate::App>(
    ui: &mut crate::Ui<A>,
    renderer: &mut Renderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    scale_factor: f32,
) -> (Vec<u8>, u32, u32) {
    let [w, h] = ui.viewport();
    let width = (w * scale_factor).round().max(1.0) as u32;
    let height = (h * scale_factor).round().max(1.0) as u32;
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("embun snapshot"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: SNAPSHOT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let padded = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("embun snapshot readback"),
        size: padded as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("embun snapshot") });
    ui.render(renderer, device, queue, &mut encoder, &view, [width, height], scale_factor);
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(height) },
        },
        size,
    );
    queue.submit([encoder.finish()]);
    readback.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map snapshot"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("wait for GPU");
    let mapped = readback.slice(..).get_mapped_range().expect("snapshot mapped");
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for row in mapped.chunks(padded as usize) {
        pixels.extend_from_slice(&row[..(width * 4) as usize]);
    }
    drop(mapped);
    readback.unmap();
    (pixels, width, height)
}

#[derive(Clone, Copy)]
enum Which {
    Content,
    Foreground,
}

fn begin<'e>(encoder: &'e mut wgpu::CommandEncoder, view: &wgpu::TextureView, load: wgpu::LoadOp<wgpu::Color>) -> wgpu::RenderPass<'e> {
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("embun layer"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
        })],
        ..Default::default()
    })
}

fn draw(pass: &mut wgpu::RenderPass<'_>, layer: &Layer, batches: &[Batch], quads: Option<&wgpu::Buffer>, images: &HashMap<u64, GpuImage>) {
    for batch in batches {
        // Text drawing binds its own vertex buffer, so bind ours every time.
        if let (Some(quads), Batch::Quads(_) | Batch::Image(..)) = (quads, batch) {
            pass.set_vertex_buffer(0, quads.slice(..));
        }
        match batch {
            Batch::Quads(range) => {
                pass.set_pipeline(&layer.rects);
                pass.set_bind_group(0, &layer.globals_group, &[]);
                pass.draw(0..6, range.clone());
            }
            Batch::Image(id, index) => {
                if let Some(image) = images.get(id) {
                    pass.set_pipeline(&layer.images);
                    pass.set_bind_group(0, &layer.globals_group, &[]);
                    pass.set_bind_group(1, &image.group, &[]);
                    pass.draw(0..6, *index..*index + 1);
                }
            }
            Batch::Text(run) => {
                if let Some(renderer) = layer.text.get(*run)
                    && let Err(e) = renderer.render(&layer.atlas, &layer.viewport, pass)
                {
                    eprintln!("embun: text not drawn this frame: {e}");
                }
            }
        }
    }
}

fn upload_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    image: &crate::Image,
) -> wgpu::BindGroup {
    let size = wgpu::Extent3d { width: image.width, height: image.height, depth_or_array_layers: 1 };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("embun image"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &image.rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(image.width * 4), rows_per_image: Some(image.height) },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("embun image"),
        layout,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
        ],
    })
}
