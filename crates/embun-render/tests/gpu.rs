//! Renders real frames on the GPU and checks pixels.
//! Needs a GPU adapter (Metal, Vulkan, DX12 or a software adapter).

#[path = "../examples/support/backdrops.rs"]
mod backdrops;

use embun_render::{Frame, GlassLayers, GlassParams, GlassRenderer, GlassShape, offscreen};

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: GlassRenderer,
}

fn gpu() -> Gpu {
    let (device, queue) = pollster::block_on(async {
        let adapter = wgpu::Instance::default()
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .expect("these tests need a GPU adapter");
        adapter.request_device(&wgpu::DeviceDescriptor::default()).await.expect("request device")
    });
    let renderer = GlassRenderer::new(&device, offscreen::FORMAT);
    Gpu { device, queue, renderer }
}

const VIEW: [f32; 2] = [200.0, 100.0];

/// Renders a flat backdrop at scale 1 and returns a pixel reader.
fn render(gpu: &Gpu, backdrop_color: [u8; 3], shapes: &[GlassShape], params: GlassParams) -> impl Fn(u32, u32) -> [u8; 3] + use<> {
    let image = backdrops::solid(64, 64, backdrop_color);
    let backdrop = gpu.renderer.create_backdrop(&gpu.device, &gpu.queue, &image.rgba, image.width, image.height);
    let frame = Frame { viewport: VIEW, scale_factor: 1.0, backdrop_scale: 1.0, scroll: 0.0, shapes, params };
    let (pixels, width, _) = offscreen::render_rgba(&gpu.device, &gpu.queue, &gpu.renderer, &backdrop, &frame);
    move |x, y| {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    }
}

fn circle(x: f32, y: f32, r: f32, appearance: f32) -> GlassShape {
    GlassShape { center: [x, y], half_size: [r, r], radius: r, appearance }
}

fn assert_near(actual: [u8; 3], expected: [u8; 3], tolerance: u8) {
    for c in 0..3 {
        assert!(
            actual[c].abs_diff(expected[c]) <= tolerance,
            "pixel {actual:?} is not within {tolerance} of {expected:?}"
        );
    }
}

// unslop-ignore TEST_WITHOUT_ASSERTION -- asserts through the assert_near helper
#[test]
fn pixels_outside_glass_show_the_backdrop_unchanged() {
    let gpu = gpu();
    let px = render(&gpu, [40, 80, 160], &[circle(100.0, 50.0, 30.0, 0.0)], GlassParams::from_level(1.0, GlassLayers::default(), false));
    assert_near(px(2, 2), [40, 80, 160], 1);
    assert_near(px(197, 97), [40, 80, 160], 1);
}

// unslop-ignore TEST_WITHOUT_ASSERTION -- asserts through the assert_near helper
#[test]
fn glass_over_flat_backdrops_matches_macos_captures() {
    // Center pixels read from reference/macos-27.0.1/*/solid-*-glass.png.
    let cases = [
        (0.0, 0.0, [242, 242, 247], [229, 229, 235]),
        (0.5, 0.0, [242, 242, 247], [235, 235, 239]),
        (1.0, 0.0, [242, 242, 247], [243, 243, 245]),
        (0.0, 1.0, [28, 28, 30], [48, 48, 51]),
        (0.5, 1.0, [28, 28, 30], [48, 48, 50]),
        (1.0, 1.0, [28, 28, 30], [40, 40, 41]),
    ];
    let gpu = gpu();
    for (level, appearance, backdrop, macos) in cases {
        let shapes = [circle(100.0, 50.0, 40.0, appearance)];
        let center = render(&gpu, backdrop, &shapes, GlassParams::from_level(level, GlassLayers::default(), false))(100, 50);
        assert_near(center, macos, 3);
    }
}

#[test]
fn outline_darkens_the_pixel_just_outside_like_macos() {
    let gpu = gpu();
    // A rectangle with edges on pixel boundaries: left edge x = 60, top edge y = 30.
    let shapes = [GlassShape { center: [100.0, 50.0], half_size: [40.0, 20.0], radius: 10.0, appearance: 0.0 }];
    let px = render(&gpu, [242, 242, 247], &shapes, GlassParams::from_level(0.0, GlassLayers::default(), false));
    // macOS clear, light: 178 beside the left edge and 200 above the top edge.
    assert!(px(59, 50)[1].abs_diff(178) <= 5, "left outline was {:?}", px(59, 50));
    assert!(px(100, 29)[1].abs_diff(200) <= 6, "top outline was {:?}", px(100, 29));
    assert!(px(57, 50)[1] >= 241, "two pixels out should be untouched, got {:?}", px(57, 50));
}

// unslop-ignore TEST_WITHOUT_ASSERTION -- asserts through the assert_near helper
#[test]
fn close_shapes_merge_only_when_merging_is_on() {
    let gpu = gpu();
    // Two circles with a 6 px gap between them, centered on (123, 50).
    let shapes = [circle(100.0, 50.0, 20.0, 0.0), circle(146.0, 50.0, 20.0, 0.0)];
    let merged = render(&gpu, [20, 20, 20], &shapes, GlassParams::from_level(1.0, GlassLayers::default(), false))(123, 50);
    let apart = GlassLayers { merge: false, ..GlassLayers::default() };
    let separate = render(&gpu, [20, 20, 20], &shapes, GlassParams::from_level(1.0, apart, false))(123, 50);
    assert!(merged[1] > 150, "gap should be filled with glass when merging, got {merged:?}");
    assert_near(separate, [20, 20, 20], 1);
}

// unslop-ignore TEST_WITHOUT_ASSERTION -- asserts through the assert_near helper
#[test]
fn optional_shadow_darkens_the_backdrop_just_below_a_shape() {
    let gpu = gpu();
    let shapes = [circle(100.0, 40.0, 30.0, 0.0)];
    let shadow = GlassLayers { shadow: true, ..GlassLayers::default() };
    let with = render(&gpu, [200, 200, 200], &shapes, GlassParams::from_level(0.5, shadow, false))(100, 74);
    let without = render(&gpu, [200, 200, 200], &shapes, GlassParams::from_level(0.5, GlassLayers::default(), false))(100, 74);
    assert_near(without, [200, 200, 200], 1);
    assert!(with[1] + 10 < without[1], "shadow {with:?} should be darker than {without:?}");
}

#[test]
fn non_srgb_targets_like_browser_canvases_get_the_same_colors() {
    let gpu = gpu();
    let linear_target = GlassRenderer::new(&gpu.device, wgpu::TextureFormat::Rgba8Unorm);
    let image = backdrops::checker(false);
    let shapes = [circle(100.0, 50.0, 40.0, 0.0), circle(160.0, 50.0, 20.0, 1.0)];
    let frame = Frame {
        viewport: VIEW,
        scale_factor: 1.0,
        backdrop_scale: 2.0,
        scroll: 0.0,
        shapes: &shapes,
        params: GlassParams::from_level(0.5, GlassLayers::default(), false),
    };
    let srgb_backdrop = gpu.renderer.create_backdrop(&gpu.device, &gpu.queue, &image.rgba, image.width, image.height);
    let (srgb, ..) = offscreen::render_rgba(&gpu.device, &gpu.queue, &gpu.renderer, &srgb_backdrop, &frame);
    let other_backdrop = linear_target.create_backdrop(&gpu.device, &gpu.queue, &image.rgba, image.width, image.height);
    let (unorm, ..) = offscreen::render_rgba_as(
        &gpu.device,
        &gpu.queue,
        &linear_target,
        &other_backdrop,
        &frame,
        wgpu::TextureFormat::Rgba8Unorm,
    );
    let worst = srgb.iter().zip(&unorm).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    // The sRGB curve and the shader's 2.2 gamma differ slightly in the darks.
    assert!(worst <= 8, "largest channel difference was {worst}");
}
