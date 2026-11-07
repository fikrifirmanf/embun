//! Rendering into memory instead of a window: for snapshots and golden-image tests.

use crate::{Backdrop, Frame, GlassRenderer};

/// The format [`render_rgba`] renders in. Build the renderer with this format.
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Renders one frame and returns tightly packed sRGB RGBA8 pixels, row by row.
///
/// The pixel size is `frame.viewport * frame.scale_factor`, rounded.
pub fn render_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &GlassRenderer,
    backdrop: &Backdrop,
    frame: &Frame<'_>,
) -> (Vec<u8>, u32, u32) {
    render_rgba_as(device, queue, renderer, backdrop, frame, FORMAT)
}

/// Like [`render_rgba`], for a renderer built with another 4-byte RGBA format.
/// With a non-sRGB format such as `Rgba8Unorm` (what browser canvases use),
/// the renderer writes gamma-encoded values itself, so the bytes should match.
pub fn render_rgba_as(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &GlassRenderer,
    backdrop: &Backdrop,
    frame: &Frame<'_>,
    format: wgpu::TextureFormat,
) -> (Vec<u8>, u32, u32) {
    let width = (frame.viewport[0] * frame.scale_factor).round().max(1.0) as u32;
    let height = (frame.viewport[1] * frame.scale_factor).round().max(1.0) as u32;
    let size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("embun offscreen"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let unpadded = width * 4;
    let padded = unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("embun readback"),
        size: padded as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("embun offscreen") });
    renderer.render(queue, &mut encoder, &view, backdrop, frame);
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(height) },
        },
        size,
    );
    queue.submit([encoder.finish()]);

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| result.expect("map readback buffer"));
    device.poll(wgpu::PollType::wait_indefinitely()).expect("wait for GPU");

    let mapped = slice.get_mapped_range().expect("read mapped buffer");
    let mut pixels = Vec::with_capacity((unpadded * height) as usize);
    for row in mapped.chunks(padded as usize) {
        pixels.extend_from_slice(&row[..unpadded as usize]);
    }
    drop(mapped);
    readback.unmap();
    (pixels, width, height)
}
