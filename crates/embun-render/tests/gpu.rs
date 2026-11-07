//! Renders real frames on the GPU and checks pixels.
//! Needs a GPU adapter (Metal, Vulkan, DX12, GL or a software adapter such as
//! llvmpipe). Set WGPU_BACKEND (for example `vulkan` or `gl`) to pick one.

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
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env())
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
    render_image(gpu, &backdrops::solid(64, 64, backdrop_color), shapes, params)
}

/// Renders `image` as the backdrop, one texel per pixel, and returns a pixel reader.
fn render_image(gpu: &Gpu, image: &backdrops::Image, shapes: &[GlassShape], params: GlassParams) -> impl Fn(u32, u32) -> [u8; 3] + use<> {
    let backdrop = gpu.renderer.create_backdrop(&gpu.device, &gpu.queue, &image.rgba, image.width, image.height);
    let frame = Frame { viewport: VIEW, scale_factor: 1.0, backdrop_scale: 1.0, scroll: 0.0, shapes, params };
    let (pixels, width, _) = offscreen::render_rgba(&gpu.device, &gpu.queue, &gpu.renderer, &backdrop, &frame);
    move |x, y| {
        let i = ((y * width + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    }
}

fn circle(x: f32, y: f32, r: f32, appearance: f32) -> GlassShape {
    GlassShape { center: [x, y], half_size: [r, r], radius: r, appearance, ..Default::default() }
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
    // Center pixels read from reference/macos-27.0.1/{clear,middle,tinted}/solid-*-glass.png
    // (focused window) and from the same files under inactive/ (window without focus).
    let (light, dark) = ([242, 242, 247], [28, 28, 30]);
    let focused = [
        (0.0, 0.0, light, [246, 246, 251]),
        (0.5, 0.0, light, [247, 248, 252]),
        (1.0, 0.0, light, [251, 251, 253]),
        (0.0, 1.0, dark, [59, 59, 61]),
        (0.5, 1.0, dark, [59, 59, 61]),
        (1.0, 1.0, dark, [45, 45, 47]),
    ];
    let unfocused = [
        (0.0, 0.0, light, [229, 229, 235]),
        (0.5, 0.0, light, [235, 235, 239]),
        (1.0, 0.0, light, [243, 243, 245]),
        (0.0, 1.0, dark, [48, 48, 51]),
        (0.5, 1.0, dark, [48, 48, 50]),
        (1.0, 1.0, dark, [40, 40, 41]),
    ];
    let gpu = gpu();
    type Make = fn(f32, GlassLayers, bool) -> GlassParams;
    for (cases, make) in [(focused, GlassParams::from_level as Make), (unfocused, GlassParams::unfocused as Make)] {
        for (level, appearance, backdrop, macos) in cases {
            let shapes = [circle(100.0, 50.0, 40.0, appearance)];
            let center = render(&gpu, backdrop, &shapes, make(level, GlassLayers::default(), false))(100, 50);
            assert_near(center, macos, 3);
        }
    }
}

#[test]
fn outline_darkens_the_pixel_just_outside_like_macos() {
    let gpu = gpu();
    // A rectangle with edges on pixel boundaries: left edge x = 60, top edge y = 30.
    let shapes = [GlassShape { center: [100.0, 50.0], half_size: [40.0, 20.0], radius: 10.0, appearance: 0.0, ..Default::default() }];
    let focused = render(&gpu, [242, 242, 247], &shapes, GlassParams::from_level(0.0, GlassLayers::default(), false));
    // macOS clear, light, focused: 176 beside the left edge, 223 above the top edge.
    assert!(focused(59, 50)[1].abs_diff(176) <= 5, "left outline was {:?}", focused(59, 50));
    assert!(focused(100, 29)[1].abs_diff(223) <= 6, "top outline was {:?}", focused(100, 29));
    assert!(focused(57, 50)[1] >= 241, "two pixels out should be untouched, got {:?}", focused(57, 50));
    // Without focus: 178 beside the left edge, 200 above the top edge.
    let unfocused = render(&gpu, [242, 242, 247], &shapes, GlassParams::unfocused(0.0, GlassLayers::default(), false));
    assert!(unfocused(59, 50)[1].abs_diff(178) <= 5, "left outline was {:?}", unfocused(59, 50));
    assert!(unfocused(100, 29)[1].abs_diff(200) <= 6, "top outline was {:?}", unfocused(100, 29));
}

#[test]
fn focused_glass_brightens_the_inside_of_its_top_edge_like_macos() {
    let gpu = gpu();
    let shapes = [GlassShape { center: [100.0, 50.0], half_size: [40.0, 20.0], radius: 10.0, appearance: 1.0, ..Default::default() }];
    let px = render(&gpu, [28, 28, 30], &shapes, GlassParams::from_level(0.0, GlassLayers::default(), false));
    // macOS clear, dark, focused: 108 in the first pixel inside the top edge,
    // 87 in the second, against 59 inside; nothing extra inside the left edge.
    assert!(px(100, 30)[1].abs_diff(108) <= 6, "first row inside the top was {:?}", px(100, 30));
    assert!(px(100, 31)[1].abs_diff(87) <= 6, "second row inside the top was {:?}", px(100, 31));
    assert!(px(60, 50)[1].abs_diff(59) <= 4, "first column inside the left was {:?}", px(60, 50));
    let unfocused = render(&gpu, [28, 28, 30], &shapes, GlassParams::unfocused(0.0, GlassLayers::default(), false));
    assert!(unfocused(100, 30)[1] < 60, "unfocused glass has no highlight, got {:?}", unfocused(100, 30));
}

// unslop-ignore TEST_WITHOUT_ASSERTION -- asserts through the assert_near helper
#[test]
fn clear_glass_keeps_detail_on_its_own_side_of_the_frost_like_macos() {
    // Over 2 px stripes the 16 pt frost is flat gray, so each stripe shows how
    // much sharp detail comes through. In the clear captures, light glass
    // passes detail brighter than the frost about twice as strongly as darker
    // detail (slopes 0.52 and 0.25); dark glass does the reverse, about 3.5
    // times (0.67 and 0.19).
    let gpu = gpu();
    let (dark_stripe, bright_stripe, mean) = (60.0, 180.0, 120.0);
    let mut stripes = backdrops::solid(200, 100, [0, 0, 0]);
    for (i, px) in stripes.rgba.chunks_mut(4).enumerate() {
        let v = if (i as u32 % 200 / 2).is_multiple_of(2) { dark_stripe } else { bright_stripe };
        px[..3].fill(v as u8);
    }
    let level = 0.0;
    for (appearance, name) in [(0.0, "light"), (1.0, "dark")] {
        let shape = [GlassShape { center: [100.0, 50.0], half_size: [90.0, 45.0], radius: 20.0, appearance, ..Default::default() }];
        let params = GlassParams::from_level(level, GlassLayers::default(), false);
        let over_stripes = render_image(&gpu, &stripes, &shape, params);
        let over_mean = render(&gpu, [mean as u8; 3], &shape, params);
        let base = over_mean(100, 50)[1] as f32;
        // Pixel 100 sits on a dark stripe, pixel 102 on a bright one.
        let toward_dark = (base - over_stripes(100, 50)[1] as f32) / (mean - dark_stripe);
        let toward_bright = (over_stripes(102, 50)[1] as f32 - base) / (bright_stripe - mean);
        let (kept, flattened, at_least) =
            if appearance == 0.0 { (toward_bright, toward_dark, 1.5) } else { (toward_dark, toward_bright, 2.5) };
        assert!(
            kept > at_least * flattened && flattened > 0.0,
            "{name} glass passed detail with slopes {toward_bright:.2} (brighter) and {toward_dark:.2} (darker)"
        );
    }
}

#[test]
fn frost_follows_new_backdrop_content_after_regenerate_mips() {
    // The frost blur is reused between frames, so it must be redone when the
    // content behind the glass changes.
    let gpu = gpu();
    let image = backdrops::solid(200, 100, [20, 20, 20]);
    let backdrop = gpu.renderer.create_backdrop(&gpu.device, &gpu.queue, &image.rgba, image.width, image.height);
    let shapes = [circle(100.0, 50.0, 40.0, 0.0)];
    let draw = || {
        let frame = Frame {
            viewport: VIEW,
            scale_factor: 1.0,
            backdrop_scale: 1.0,
            scroll: 0.0,
            shapes: &shapes,
            params: GlassParams::from_level(1.0, GlassLayers::default(), false),
        };
        let (pixels, width, _) = offscreen::render_rgba(&gpu.device, &gpu.queue, &gpu.renderer, &backdrop, &frame);
        let i = ((50 * width + 100) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2]]
    };
    let over_dark = draw();
    assert_eq!(draw(), over_dark, "an unchanged backdrop must keep the same frost");

    let bright = backdrops::solid(200, 100, [230, 230, 230]);
    gpu.queue.write_texture(
        backdrop.texture().as_image_copy(),
        &bright.rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(200 * 4), rows_per_image: Some(100) },
        wgpu::Extent3d { width: 200, height: 100, depth_or_array_layers: 1 },
    );
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    gpu.renderer.regenerate_mips(&mut encoder, &backdrop);
    gpu.queue.submit([encoder.finish()]);

    // Same glass over a fresh backdrop of the new color.
    let fresh = render(&gpu, [230, 230, 230], &shapes, GlassParams::from_level(1.0, GlassLayers::default(), false));
    assert_near(draw(), fresh(100, 50), 1);
}

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
fn merged_neck_has_no_seam() {
    let gpu = gpu();
    // A left-to-right ramp, unblurred, so each pixel shows where its sample came from.
    let mut image = backdrops::solid(256, 100, [0, 0, 0]);
    for (i, px) in image.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        px[0] = (i % 256) as u8;
    }
    let shapes = [circle(100.0, 50.0, 20.0, 0.0), circle(146.0, 50.0, 20.0, 0.0)];
    let no_frost = GlassLayers { frost: false, fringe: false, ..GlassLayers::default() };
    let px = render_image(&gpu, &image, &shapes, GlassParams::from_level(0.0, no_frost, false));
    // The neck's center is x = 123. Two pixels apart, the ramp moves by about
    // two steps; lensing that flips sides at the center would jump by ~20.
    let (left, right) = (px(122, 50)[0], px(124, 50)[0]);
    assert!(left.abs_diff(right) <= 6, "seam at the neck: {left} then {right}");
}

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
    // Both paths use the exact sRGB curve; only rounding differs.
    assert!(worst <= 2, "largest channel difference was {worst}");
}

// unslop-ignore TEST_WITHOUT_ASSERTION -- asserts through the assert_near helper
#[test]
fn prominent_glass_matches_macos_and_loses_its_color_without_focus() {
    // Center pixels of the extra-large `.glassProminent` button in
    // reference/macos-27.0.1/{clear,tinted}/controls-solid-*-glass.png. The
    // capture app tints with system blue, which is lighter in dark mode.
    let (light, dark) = ([242, 242, 247], [28, 28, 30]);
    let (blue, dark_blue) = ([0.0, 133.0 / 255.0, 250.0 / 255.0], [0.0, 145.0 / 255.0, 1.0]);
    let cases = [
        (0.0, 0.0, light, blue, [0, 133, 251]),
        (1.0, 0.0, light, blue, [0, 135, 253]),
        (0.0, 1.0, dark, dark_blue, [4, 146, 254]),
        (1.0, 1.0, dark, dark_blue, [1, 145, 255]),
    ];
    let gpu = gpu();
    for (level, appearance, backdrop, tint, macos) in cases {
        let shapes = [circle(100.0, 50.0, 40.0, appearance).tint(tint, 1.0)];
        let center = render(&gpu, backdrop, &shapes, GlassParams::from_level(level, GlassLayers::default(), false))(100, 50);
        // The tint model is one least-squares fit over photo, checker, noise
        // and flat backdrops; its RMS error is up to 5 out of 255 (dark, clear).
        assert_near(center, macos, 5);
    }
    // A window without focus draws the same button as plain glass.
    let unfocused = GlassParams::unfocused(0.5, GlassLayers::default(), false);
    let tinted = render(&gpu, light, &[circle(100.0, 50.0, 40.0, 0.0).tint(blue, 1.0)], unfocused)(100, 50);
    let plain = render(&gpu, light, &[circle(100.0, 50.0, 40.0, 0.0)], unfocused)(100, 50);
    assert_eq!(tinted, plain);
}

#[test]
fn shapes_in_different_merge_groups_do_not_merge() {
    let gpu = gpu();
    // Two circles with a 6 px gap, centered on (123, 50), as in the merge test.
    let params = GlassParams::from_level(1.0, GlassLayers::default(), false);
    let grouped = [circle(100.0, 50.0, 20.0, 0.0), circle(146.0, 50.0, 20.0, 0.0)];
    let apart = [circle(100.0, 50.0, 20.0, 0.0), circle(146.0, 50.0, 20.0, 0.0).group(1)];
    let merged = render(&gpu, [20, 20, 20], &grouped, params);
    let separate = render(&gpu, [20, 20, 20], &apart, params);
    assert!(merged(123, 50)[1] > 150, "same group should fill the gap, got {:?}", merged(123, 50));
    assert_near(separate(123, 50), [20, 20, 20], 1);
    // Both shapes are still drawn in full.
    assert_near(separate(100, 50), merged(100, 50), 1);
    assert_near(separate(146, 50), merged(146, 50), 1);
}

#[test]
fn frames_over_two_backdrops_can_share_one_encoder() {
    let gpu = gpu();
    let make = |color| {
        let image = backdrops::solid(200, 100, color);
        gpu.renderer.create_backdrop(&gpu.device, &gpu.queue, &image.rgba, image.width, image.height)
    };
    let (dark_backdrop, light_backdrop) = (make([20, 20, 20]), make([230, 230, 230]));
    let dark_shapes = [circle(100.0, 50.0, 40.0, 1.0)];
    let light_shapes = [circle(100.0, 50.0, 40.0, 0.0)];
    let frame = |shapes, level| Frame {
        viewport: VIEW,
        scale_factor: 1.0,
        backdrop_scale: 1.0,
        scroll: 0.0,
        shapes,
        params: GlassParams::from_level(level, GlassLayers::default(), false),
    };
    let frames = [(&dark_backdrop, frame(&dark_shapes, 0.0)), (&light_backdrop, frame(&light_shapes, 1.0))];

    // Each frame on its own.
    let alone: Vec<[u8; 3]> = frames
        .iter()
        .map(|(backdrop, frame)| {
            let (pixels, width, _) = offscreen::render_rgba(&gpu.device, &gpu.queue, &gpu.renderer, backdrop, frame);
            let i = ((50 * width + 100) * 4) as usize;
            [pixels[i], pixels[i + 1], pixels[i + 2]]
        })
        .collect();

    // Both recorded into one encoder before a single submit.
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let readbacks: Vec<wgpu::Buffer> = frames
        .iter()
        .map(|(backdrop, frame)| {
            let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: 200, height: 100, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: offscreen::FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            gpu.renderer.render(&gpu.queue, &mut encoder, &target.create_view(&Default::default()), backdrop, frame);
            let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: 1024 * 100,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            encoder.copy_texture_to_buffer(
                target.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(1024), rows_per_image: Some(100) },
                },
                wgpu::Extent3d { width: 200, height: 100, depth_or_array_layers: 1 },
            );
            buffer
        })
        .collect();
    gpu.queue.submit([encoder.finish()]);
    for (buffer, expected) in readbacks.iter().zip(&alone) {
        buffer.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("map"));
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).expect("poll");
        let data = buffer.slice(..).get_mapped_range().expect("mapped");
        let i = 50 * 1024 + 100 * 4;
        assert_eq!([data[i], data[i + 1], data[i + 2]], *expected);
    }
    assert_ne!(alone[0], alone[1], "the two frames should differ, or this test proves nothing");
}

#[test]
fn shapes_with_nan_or_infinite_values_are_skipped() {
    let gpu = gpu();
    let params = GlassParams::from_level(1.0, GlassLayers::default(), false);
    let good = circle(60.0, 50.0, 20.0, 0.0);
    let alone = render(&gpu, [20, 20, 20], &[good], params);
    let bad = [circle(f32::NAN, 50.0, 20.0, 0.0), GlassShape { radius: f32::INFINITY, ..circle(140.0, 50.0, 20.0, 0.0) }];
    let mixed = render(&gpu, [20, 20, 20], &[bad[0], good, bad[1]], params);
    for (x, y) in [(60, 50), (140, 50), (100, 50), (5, 5)] {
        assert_eq!(mixed(x, y), alone(x, y), "pixel ({x}, {y})");
    }
}

#[test]
fn up_to_max_shapes_are_drawn_and_the_rest_ignored() {
    use embun_render::MAX_SHAPES;
    let gpu = gpu();
    let params = GlassParams::from_level(1.0, GlassLayers { merge: false, ..GlassLayers::default() }, false);
    // A 13 x 5 grid of 6 px circles, one more than the limit.
    let shapes: Vec<GlassShape> =
        (0..MAX_SHAPES + 1).map(|i| circle(10.0 + (i % 13) as f32 * 15.0, 10.0 + (i / 13) as f32 * 20.0, 6.0, 0.0)).collect();
    let px = render(&gpu, [20, 20, 20], &shapes, params);
    let at = |i: usize| px(10 + (i % 13) as u32 * 15, 10 + (i / 13) as u32 * 20);
    assert!(at(MAX_SHAPES - 1)[1] > 150, "shape {} should be glass, got {:?}", MAX_SHAPES - 1, at(MAX_SHAPES - 1));
    assert_near(at(MAX_SHAPES), [20, 20, 20], 1);
}
