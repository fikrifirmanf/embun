# embun-render

Draws translucent glass shapes with wgpu, in the style of macOS 27. This is the renderer behind [Embun](https://crates.io/crates/embun), and you can use it on its own.

Each shape mixes a sharp and a frosted copy of the backdrop behind it, bends it near the edge, tints it, and gets a one-pixel outline and a thin highlight along the top and bottom. Light glass lets bright detail through more than dark detail, and dark glass does the reverse. Shapes that come within about 19 points of each other merge like water drops. All of this was fitted to screenshots of real macOS 27.0.1 glass: the average difference per pixel is 0.9 to 1.8 out of 255 for a focused window, depending on the transparency setting.

`GlassParams::from_level` gives the look of a focused window. Use `GlassParams::unfocused` for a window without focus, which macOS frosts more.

A shape can be tinted for a prominent button: `.tint([0.0, 0.52, 0.98], 1.0)` draws it almost solid system blue, lit slightly by the backdrop, as macOS does. Pass the tint color for the current appearance, since macOS uses a lighter blue in dark mode. Like macOS, an unfocused window draws tinted shapes as plain glass.

Shapes in the same merge group melt together when they come within 19 points. Give separate controls different groups with `.group(n)` so they only touch.

The renderer draws into any texture view you give it, so it fits inside a winit app, an iced `shader` widget, an egui paint callback or a Bevy render node.

```rust
use embun_render::{Frame, GlassLayers, GlassParams, GlassRenderer, GlassShape};

/// Draws one glass toolbar over `photo` (RGBA8, sRGB) into `target`.
fn draw(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    target: &wgpu::TextureView,
    target_format: wgpu::TextureFormat,
    photo: &[u8],
    width: u32,
    height: u32,
) {
    // Create these once and keep them. They are not per frame.
    let renderer = GlassRenderer::new(device, target_format);
    let backdrop = renderer.create_backdrop(device, queue, photo, width, height);

    // Center, half size and corner radius in logical pixels. A radius of half
    // the height gives a capsule.
    let toolbar = GlassShape::new([480.0, 44.0], [230.0, 26.0], 26.0);
    // A prominent button: system blue, drawn almost solid.
    let done = GlassShape::new([880.0, 44.0], [40.0, 14.0], 14.0).tint([0.0, 0.52, 0.98], 1.0);
    let frame = Frame {
        viewport: [960.0, 600.0],
        scale_factor: 2.0,
        backdrop_scale: width as f32 / 960.0,
        scroll: 0.0,
        shapes: &[toolbar, done],
        // 0 is the clear setting, 0.5 the middle, 1 tinted.
        params: GlassParams::from_level(0.5, GlassLayers::default(), false),
    };

    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.render(queue, &mut encoder, target, &backdrop, &frame);
    queue.submit([encoder.finish()]);
}
```

For tests and screenshots, `offscreen::render_rgba` renders a frame and returns its pixels.

The glass shader runs only in a rectangle around each shape, and each pixel only looks at the shapes near it. The rest of the target gets a plain copy of the backdrop, so cost grows with the area the glass covers, not with screen size or shape count. On an Apple M5 at 2880×1864, a frame with 4 glass buttons takes 0.26 ms of GPU time, 16 buttons 0.78 ms, and a full-height sidebar 0.57 ms (`cargo run --release -p embun-render --example bench`). The frost blur is computed once and reused until the backdrop changes: after you draw new content into the backdrop texture, call `regenerate_mips`, which also tells the next `render` to blur again.

Tested on Metal, and on WebGPU and WebGL2 in Chrome. Vulkan and DirectX 12 are not tested yet.

Embun is not affiliated with Apple. It uses no Apple code, fonts or symbols.

License: MIT or Apache-2.0, at your option.
