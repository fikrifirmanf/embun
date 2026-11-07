//! Renders the demo scene offscreen and writes a PNG.
//!
//! cargo run -p embun-render --example snapshot -- --backdrop photos --level 0.5 --out glass.png
//! Options: --backdrop photos|checker, --level 0..1, --dark, --scroll <texels>, --out <file>

#[path = "support/backdrops.rs"]
mod backdrops;

use embun_render::{Frame, GlassLayers, GlassParams, GlassRenderer, GlassShape, offscreen};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let dark = args.iter().any(|a| a == "--dark");
    let level: f32 = value("--level").map_or(0.5, |v| v.parse().expect("--level must be a number"));
    let scroll: f32 = value("--scroll").map_or(140.0, |v| v.parse().expect("--scroll must be a number"));
    let out = value("--out").unwrap_or_else(|| "glass.png".into());
    let image = match value("--backdrop").as_deref().unwrap_or("photos") {
        "photos" => backdrops::photos(dark),
        "checker" => backdrops::checker(dark),
        other => panic!("unknown backdrop {other:?}; use photos or checker"),
    };

    let (device, queue) = pollster::block_on(gpu());
    let renderer = GlassRenderer::new(&device, offscreen::FORMAT);
    let backdrop = renderer.create_backdrop(&device, &queue, &image.rgba, image.width, image.height);

    let (w, h) = (960.0, 600.0);
    let appearance = if dark { 1.0 } else { 0.0 };
    let shape = |center, half_size, radius| GlassShape { center, half_size, radius, appearance, ..Default::default() };
    let shapes = [
        shape([w * 0.32, h * 0.56], [96.0, 60.0], 34.0),
        shape([w - 118.0, h - 60.0], [28.0, 28.0], 28.0),
        shape([w - 56.0, h - 60.0], [28.0, 28.0], 28.0),
        shape([w / 2.0, 44.0], [230.0, 26.0], 26.0),
    ];
    let frame = Frame {
        viewport: [w, h],
        scale_factor: 2.0,
        backdrop_scale: image.width as f32 / w,
        scroll,
        shapes: &shapes,
        params: GlassParams::from_level(level, GlassLayers::default(), false),
    };

    let (pixels, pw, ph) = offscreen::render_rgba(&device, &queue, &renderer, &backdrop, &frame);
    let file = std::fs::File::create(&out).expect("create output file");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), pw, ph);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut w| w.write_image_data(&pixels)).expect("write PNG");
    println!("wrote {out} ({pw}x{ph})");
}

async fn gpu() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("no GPU adapter found");
    adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("request device")
}
