# embun-render

Draws translucent glass shapes with wgpu, in the style of macOS 27. This is the renderer behind [Embun](https://crates.io/crates/embun), and you can use it on its own.

Each shape mixes a sharp and a frosted copy of the backdrop behind it, bends it near the edge, tints it, and gets a one-pixel outline and a thin highlight along the top and bottom. Light glass lets bright detail through more than dark detail, and dark glass does the reverse. Shapes that come within about 19 points of each other merge like water drops. All of this was fitted to screenshots of real macOS 27.0.1 glass: the average difference per pixel is 0.9 to 1.8 out of 255 for a focused window, depending on the transparency setting.

`GlassParams::from_level` gives the look of a focused window. Use `GlassParams::unfocused` for a window without focus, which macOS frosts more.

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

    let toolbar = GlassShape {
        center: [480.0, 44.0], // logical pixels
        half_size: [230.0, 26.0],
        radius: 26.0, // a radius of half the height gives a capsule
        appearance: 0.0, // 0 light, 1 dark
    };
    let frame = Frame {
        viewport: [960.0, 600.0],
        scale_factor: 2.0,
        backdrop_scale: width as f32 / 960.0,
        scroll: 0.0,
        shapes: &[toolbar],
        // 0 is the clear setting, 0.5 the middle, 1 tinted.
        params: GlassParams::from_level(0.5, GlassLayers::default(), false),
    };

    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.render(queue, &mut encoder, target, &backdrop, &frame);
    queue.submit([encoder.finish()]);
}
```

For tests and screenshots, `offscreen::render_rgba` renders a frame and returns its pixels.

Tested on Metal and on WebGPU in Chrome. Vulkan, DirectX 12 and WebGL2 are not tested yet.

Embun is not affiliated with Apple. It uses no Apple code, fonts or symbols.

License: MIT or Apache-2.0, at your option.
