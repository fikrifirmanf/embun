//! Times glass frames at a 14-inch MacBook Pro screen size (2880x1864 pixels).
//!
//! cargo run --release -p embun-render --example bench [-- --rounds N]
//!
//! Set WGPU_BACKEND (vulkan, metal, dx12, gl) to pick a backend.
//!
//! Keeps two frames in flight, like a swapchain. Cycles through the cases in
//! short bursts, eight times by default, and reports the fastest burst per case: other
//! apps using the GPU only ever add time, and their load changes over seconds.
//! Wall-clock time per frame; on an idle GPU this is the GPU cost.

use embun_render::{Frame, GlassLayers, GlassParams, GlassRenderer, GlassShape};
use std::collections::VecDeque;
use std::time::Instant;

const SIZE: [u32; 2] = [2880, 1864];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rounds: usize = match args.iter().position(|a| a == "--rounds") {
        Some(i) => args.get(i + 1).and_then(|n| n.parse().ok()).expect("--rounds takes a number"),
        None => 8,
    };
    let (device, queue, name) = pollster::block_on(async {
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env())
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .expect("no GPU adapter");
        let info = adapter.get_info();
        let name = format!("{} ({:?})", info.name, info.backend);
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("request device");
        (device, queue, name)
    });
    let format = wgpu::TextureFormat::Bgra8UnormSrgb;
    let renderer = GlassRenderer::new(&device, format);
    let backdrop = renderer.create_backdrop(&device, &queue, &busy_backdrop(), SIZE[0], SIZE[1]);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench target"),
        size: wgpu::Extent3d { width: SIZE[0], height: SIZE[1], depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());

    let button = |i: usize| GlassShape {
        center: [120.0 + (i % 4) as f32 * 320.0, 80.0 + (i / 4) as f32 * 200.0],
        half_size: [120.0, 28.0],
        radius: 28.0,
        ..Default::default()
    };
    let none: Vec<GlassShape> = vec![];
    let four: Vec<GlassShape> = (0..4).map(button).collect();
    let sixteen: Vec<GlassShape> = (0..16).map(button).collect();
    let sidebar = vec![GlassShape { center: [180.0, 466.0], half_size: [170.0, 456.0], radius: 20.0, ..Default::default() }];
    let frost_off = GlassLayers { frost: false, ..GlassLayers::default() };
    let cases: [(&str, &[GlassShape], GlassLayers, bool); 7] = [
        ("backdrop only, no glass", &none, frost_off, false),
        ("frost blur, no shapes", &none, GlassLayers::default(), false),
        ("4 buttons, no frost", &four, frost_off, false),
        ("4 buttons", &four, GlassLayers::default(), false),
        ("16 buttons", &sixteen, GlassLayers::default(), false),
        ("1 sidebar", &sidebar, GlassLayers::default(), false),
        ("4 buttons, content changing", &four, GlassLayers::default(), true),
    ];

    println!("{name}, {}x{} pixels", SIZE[0], SIZE[1]);
    let mut best = [f64::MAX; 7];
    for _ in 0..rounds {
        for (i, (_, shapes, layers, content_changes)) in cases.iter().enumerate() {
            let frame = Frame {
                viewport: [SIZE[0] as f32 / 2.0, SIZE[1] as f32 / 2.0],
                scale_factor: 2.0,
                backdrop_scale: 2.0,
                scroll: 0.0,
                shapes,
                params: GlassParams::from_level(0.5, *layers, false),
            };
            let run = |frames: usize| {
                let start = Instant::now();
                let mut in_flight = VecDeque::new();
                for _ in 0..frames {
                    let mut encoder = device.create_command_encoder(&Default::default());
                    if *content_changes {
                        renderer.regenerate_mips(&mut encoder, &backdrop);
                    }
                    renderer.render(&queue, &mut encoder, &view, &backdrop, &frame);
                    in_flight.push_back(queue.submit([encoder.finish()]));
                    if in_flight.len() > 2 {
                        let oldest = in_flight.pop_front();
                        device.poll(wgpu::PollType::Wait { submission_index: oldest, timeout: None }).expect("poll");
                    }
                }
                device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
                start.elapsed().as_secs_f64() * 1000.0 / frames as f64
            };
            run(30); // let the GPU clock ramp up
            best[i] = best[i].min(run(100));
        }
    }
    for ((label, ..), ms) in cases.iter().zip(best) {
        println!("{label:30} {ms:6.2} ms per frame");
    }
}

/// Large color blocks with per-pixel noise, so nothing compresses or caches
/// unusually well.
fn busy_backdrop() -> Vec<u8> {
    let mut pixels = vec![0u8; (SIZE[0] * SIZE[1] * 4) as usize];
    let mut seed = 1u32;
    for (i, px) in pixels.chunks_mut(4).enumerate() {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let (x, y) = ((i as u32 % SIZE[0]) / 40, (i as u32 / SIZE[0]) / 40);
        px.copy_from_slice(&[(x * 37 % 255) as u8, (y * 53 % 255) as u8, (seed >> 24) as u8, 255]);
    }
    pixels
}
