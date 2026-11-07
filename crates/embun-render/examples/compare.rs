//! Compares Embun's glass with a macOS capture from tools/capture.
//!
//! cargo run -p embun-render --example compare -- --capture <prefix> --level 0.5 [--out diff.png]
//!
//! Reads <prefix>-bare.png, <prefix>-glass.png and <prefix>-scene.txt, renders
//! Embun's glass over the bare image with the same shapes, and prints how far
//! Embun is from macOS. As a yardstick it also prints how far the bare image is
//! from macOS, which is the error of drawing no glass at all.
//! The output image is macOS | Embun | difference (x4), at point resolution.
//! --embun-out <png> also saves Embun's render at full resolution.
//! --set name=value overrides one parameter (blur, lensing, bevel, fringe,
//! merge_k, rim, shadow; outline, highlight and sharp scale the measured
//! values); repeat it for several.
//! --unfocused uses the look of a window without focus; compare-all.sh passes
//! it for captures under inactive/.
//!
//! Shape edges are snapped to device pixels first, as macOS layout does.

#[path = "support/png_io.rs"]
mod png_io;

use embun_render::{Frame, GlassLayers, GlassParams, GlassRenderer, GlassShape, offscreen};

/// How far around each shape to measure, in points. Covers the shadow.
const MARGIN: f32 = 16.0;

struct Scene {
    viewport: [f32; 2],
    dark: bool,
    shapes: Vec<GlassShape>,
}

fn read_scene(path: &str) -> Scene {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    let mut scene = Scene { viewport: [0.0, 0.0], dark: false, shapes: Vec::new() };
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let mut words = line.split_whitespace();
        let key = words.next().unwrap_or_default();
        let nums: Vec<f32> = words.clone().filter_map(|w| w.parse().ok()).collect();
        match key {
            "viewport" => scene.viewport = [nums[0], nums[1]],
            "appearance" => scene.dark = words.next() == Some("dark"),
            "shape" => scene.shapes.push(GlassShape {
                center: [nums[0], nums[1]],
                half_size: [nums[2], nums[3]],
                radius: nums[4],
                appearance: 0.0,
            }),
            _ => {}
        }
    }
    assert!(scene.viewport[0] > 0.0 && !scene.shapes.is_empty(), "{path} has no viewport or shapes");
    let appearance = if scene.dark { 1.0 } else { 0.0 };
    for s in &mut scene.shapes {
        s.appearance = appearance;
    }
    scene
}

/// Mean and 95th percentile of per-pixel error (mean of |dr|, |dg|, |db|, 0..255).
struct Error {
    mean: f32,
    p95: f32,
}

fn error(a: &[u8], b: &[u8], pixels: &[usize]) -> Error {
    let mut errs: Vec<f32> = pixels
        .iter()
        .map(|&i| (0..3).map(|c| (a[i * 4 + c] as f32 - b[i * 4 + c] as f32).abs()).sum::<f32>() / 3.0)
        .collect();
    let mean = errs.iter().sum::<f32>() / errs.len().max(1) as f32;
    errs.sort_by(f32::total_cmp);
    let p95 = errs.get(errs.len() * 95 / 100).copied().unwrap_or(0.0);
    Error { mean, p95 }
}

/// Pixel indices within MARGIN of the shape's box.
fn region(shape: &GlassShape, scale: f32, width: u32, height: u32) -> Vec<usize> {
    let lo = |c: usize| ((shape.center[c] - shape.half_size[c] - MARGIN) * scale).max(0.0) as u32;
    let hi = |c: usize, max: u32| (((shape.center[c] + shape.half_size[c] + MARGIN) * scale) as u32).min(max);
    let mut out = Vec::new();
    for y in lo(1)..hi(1, height) {
        for x in lo(0)..hi(0, width) {
            out.push((y * width + x) as usize);
        }
    }
    out
}

/// Rounds each edge to the device pixel grid, keeping the shape's size in points.
fn snap(shape: &mut GlassShape, scale: f32) {
    for c in 0..2 {
        let lo = ((shape.center[c] - shape.half_size[c]) * scale).round() / scale;
        let hi = ((shape.center[c] + shape.half_size[c]) * scale).round() / scale;
        shape.center[c] = (lo + hi) / 2.0;
        shape.half_size[c] = (hi - lo) / 2.0;
    }
}

fn params(args: &[String], level: f32) -> GlassParams {
    let base = if args.iter().any(|a| a == "--unfocused") {
        GlassParams::unfocused(level, GlassLayers::default(), false)
    } else {
        GlassParams::from_level(level, GlassLayers::default(), false)
    };
    overrides(args, base)
}

/// Same as the shader's sd_round_box, continuous corners included.
fn sd_round_box(p: [f32; 2], s: &GlassShape) -> f32 {
    const REACH: f32 = 1.24;
    const POWER: f32 = 2.6;
    let half = s.half_size[0].min(s.half_size[1]);
    let r = s.radius.min(half);
    let t = ((half - r) / ((REACH - 1.0) * r).max(1e-4)).clamp(0.0, 1.0);
    let a = r * (1.0 + (REACH - 1.0) * t);
    let n = 2.0 + (POWER - 2.0) * t;
    let q = [(p[0] - s.center[0]).abs() - s.half_size[0] + a, (p[1] - s.center[1]).abs() - s.half_size[1] + a];
    if q[0] > 0.0 && q[1] > 0.0 {
        let m = (q[0].powf(n) + q[1].powf(n)).powf(1.0 / n);
        let g = [(q[0] / m).powf(n - 1.0), (q[1] / m).powf(n - 1.0)];
        return (m - a) / g[0].hypot(g[1]).max(1e-4);
    }
    q[0].max(q[1]) - a
}

/// The shader's merged distance field, in points.
fn scene_distance(p: [f32; 2], shapes: &[GlassShape], k: f32) -> f32 {
    shapes.iter().fold(1e5, |d, s| {
        let b = sd_round_box(p, s);
        if k <= 0.0 {
            return d.min(b);
        }
        let h = (0.5 + 0.5 * (b - d) / k).clamp(0.0, 1.0);
        b + (d - b) * h - k * h * (1.0 - h)
    })
}

/// Pixels grouped by distance to the glass edge: the outline ring, the rim band
/// where lensing happens, and the interior.
fn bands(shapes: &[GlassShape], k: f32, scale: f32, width: u32, height: u32) -> [(&'static str, Vec<usize>); 3] {
    let (mut ring, mut rim, mut inside) = (Vec::new(), Vec::new(), Vec::new());
    for y in 0..height {
        for x in 0..width {
            let p = [(x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale];
            let d = scene_distance(p, shapes, k) * scale; // device pixels
            let i = (y * width + x) as usize;
            if d.abs() <= 1.5 {
                ring.push(i);
            } else if d < -1.5 && d > -14.0 * scale {
                rim.push(i);
            } else if d <= -14.0 * scale {
                inside.push(i);
            }
        }
    }
    [("outline", ring), ("rim band", rim), ("interior", inside)]
}

fn overrides(args: &[String], mut p: GlassParams) -> GlassParams {
    for pair in args.windows(2).filter(|w| w[0] == "--set").map(|w| &w[1]) {
        let (name, value) = pair.split_once('=').unwrap_or_else(|| panic!("--set needs name=value, got {pair:?}"));
        let v: f32 = value.parse().unwrap_or_else(|_| panic!("--set {name}: {value:?} is not a number"));
        match name {
            "blur" => p.blur_sigma = v,
            "lensing" => p.lensing = v,
            "bevel" => p.bevel = v,
            "highlight" => p.highlight = p.highlight.map(|h| h * v),
            "sharp" => {
                for b in [&mut p.light, &mut p.dark] {
                    b.sharp_gain *= v;
                    b.sharp_chroma *= v;
                }
            }
            "fringe" => p.fringe = v,
            "outline" => {
                p.outline = p.outline.map(|o| o * v);
                p.outline_ends = p.outline_ends.map(|o| o * v);
            }
            "merge_k" => p.merge_k = v,
            "rim" => p.rim = v,
            "shadow" => p.shadow = v,
            other => panic!("--set: unknown parameter {other:?}"),
        }
    }
    p
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let prefix = value("--capture").expect("--capture <prefix> is required");
    let level: f32 = value("--level").map_or(0.5, |v| v.parse().expect("--level must be a number"));
    let out = value("--out").unwrap_or_else(|| format!("{prefix}-compare.png"));

    let scene = read_scene(&format!("{prefix}-scene.txt"));
    let (bare, width, height) = png_io::read(format!("{prefix}-bare.png"));
    let (mac, mac_w, mac_h) = png_io::read(format!("{prefix}-glass.png"));
    assert_eq!((width, height), (mac_w, mac_h), "bare and glass captures differ in size");
    let scale = width as f32 / scene.viewport[0];
    let mut scene = scene;
    for s in &mut scene.shapes {
        snap(s, scale);
    }

    let (device, queue) = pollster::block_on(gpu());
    let renderer = GlassRenderer::new(&device, offscreen::FORMAT);
    let backdrop = renderer.create_backdrop(&device, &queue, &bare, width, height);
    let frame = Frame {
        viewport: scene.viewport,
        scale_factor: scale,
        backdrop_scale: scale,
        scroll: 0.0,
        shapes: &scene.shapes,
        params: params(&args, level),
    };
    let (embun, ew, eh) = offscreen::render_rgba(&device, &queue, &renderer, &backdrop, &frame);
    assert_eq!((ew, eh), (width, height), "Embun rendered a different size than the capture");

    let focus = if args.iter().any(|a| a == "--unfocused") { "unfocused" } else { "focused" };
    println!("{prefix}  level {level}  {}  {focus}", if scene.dark { "dark" } else { "light" });
    println!("{:<10} {:>14} {:>14}   (mean / p95 of |difference|, 0..255)", "shape", "embun vs mac", "no glass vs mac");
    let mut all = Vec::new();
    for (i, shape) in scene.shapes.iter().enumerate() {
        let pixels = region(shape, scale, width, height);
        let (e, b) = (error(&embun, &mac, &pixels), error(&bare, &mac, &pixels));
        println!("{:<10} {:>6.1} / {:>5.1} {:>6.1} / {:>5.1}", format!("#{i}"), e.mean, e.p95, b.mean, b.p95);
        all.extend(pixels);
    }
    all.sort_unstable();
    all.dedup();
    let (e, b) = (error(&embun, &mac, &all), error(&bare, &mac, &all));
    println!("{:<10} {:>6.1} / {:>5.1} {:>6.1} / {:>5.1}", "all", e.mean, e.p95, b.mean, b.p95);
    for (name, band) in bands(&scene.shapes, params(&args, level).merge_k, scale, width, height) {
        let (e, b) = (error(&embun, &mac, &band), error(&bare, &mac, &band));
        println!("{name:<10} {:>6.1} / {:>5.1} {:>6.1} / {:>5.1}", e.mean, e.p95, b.mean, b.p95);
    }

    let step = scale.round().max(1.0) as u32;
    let (sw, sh) = (width / step, height / step);
    let mut sheet = vec![0u8; (sw * 3 * sh * 4) as usize];
    for y in 0..sh {
        for x in 0..sw {
            let src = (((y * step) * width + x * step) * 4) as usize;
            let diff: [u8; 3] =
                std::array::from_fn(|c| ((embun[src + c] as i32 - mac[src + c] as i32).unsigned_abs() * 4).min(255) as u8);
            for (panel, rgb) in [&mac[src..src + 3], &embun[src..src + 3], &diff[..]].into_iter().enumerate() {
                let dst = ((y * sw * 3 + panel as u32 * sw + x) * 4) as usize;
                sheet[dst..dst + 3].copy_from_slice(rgb);
                sheet[dst + 3] = 255;
            }
        }
    }
    png_io::write(&out, &sheet, sw * 3, sh);
    println!("wrote {out}");
    if let Some(path) = value("--embun-out") {
        png_io::write(&path, &embun, width, height);
        println!("wrote {path}");
    }
}

async fn gpu() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("no GPU adapter found");
    adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("request device")
}
