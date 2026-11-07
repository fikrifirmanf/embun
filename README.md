# Embun

Embun is a Rust UI toolkit built on wgpu. It draws translucent glass controls that match the look of macOS 27, on any platform wgpu runs on. Embun is Indonesian for dew.

**Status: early.** The glass renderer, the spring animations and a first version of the framework work: windows, layout, text, single-line text fields, images, glass buttons, toolbars, sidebars and menus. Menus and popovers draw glass over glass, and screen readers work through AccessKit. Multi-line text editing is not written yet.

| Crate | What it does |
|---|---|
| [`embun-render`](crates/embun-render) | Draws glass shapes over a backdrop into any wgpu texture. Frost, edge bending, tint, outline, and shapes that merge when they get close. |
| [`embun-motion`](crates/embun-motion) | Springs, described the SwiftUI way with a response time and a damping fraction. |
| [`embun`](crates/embun) | The framework: an Elm-style `App` with `update` and `view`, Tailwind-style layout methods, text, images and glass controls, in a winit window. |

## How close it is to macOS

The look is measured, not guessed. `tools/capture` shows real macOS 27.0.1 glass on a known backdrop and screenshots it. The `compare` example draws Embun's glass over the same backdrop and diffs the two images.

Captures cover the three transparency settings (clear, middle, tinted), light and dark, over photos, a checkerboard, random noise and a flat color. macOS draws glass differently when its window has focus, so both looks are captured and fitted:

| Window | Clear | Middle | Tinted |
|---|---|---|---|
| Focused | 1.8 | 1.4 | 0.9 |
| Without focus | 1.1 | 0.9 | 0.5 |

Each number is the average per-pixel difference, out of 255. Most of what is left sits in the one-pixel ring at the edge of each shape and over very busy backdrops such as random noise.

Real glass buttons are captured too. Plain glass buttons score the same as the shapes above. Prominent buttons, tinted system blue, average 2.0, 1.9 and 1.6 at clear, middle and tinted. Toolbars, sidebars and menus are not captured yet.

## Speed

Measured with `cargo run --release -p embun-render --example bench` on an Apple M5 at 2880×1864 pixels, with the GPU otherwise idle:

| Frame | Time |
|---|---|
| 4 glass buttons | 0.26 ms |
| 16 glass buttons | 0.78 ms |
| 1 full-height sidebar | 0.57 ms |
| 4 buttons, content behind them changing every frame | 0.87 ms |

A 120 Hz display allows 8.3 ms per frame. Slower GPUs take longer and are not measured yet.

Without a GPU, wgpu can draw on the CPU through Mesa's llvmpipe on Linux (or WARP on Windows). Embun works there, but slowly: on 10 CPU cores at the same size, 4 buttons take 22.6 ms and 16 buttons 45.3 ms per frame. That is fine for a still UI, not for smooth animation.

## Platforms

| Backend | Tested |
|---|---|
| Metal (macOS) | yes, all tests and captures |
| Vulkan on Linux | yes, all tests, on the llvmpipe software GPU |
| OpenGL on Linux | yes, all tests, on llvmpipe |
| WebGPU in Chrome | yes, the demo page |
| WebGL2 in Chrome | yes, the demo page with WebGPU turned off |
| DirectX 12, Vulkan on real GPUs | not yet |

CI runs the tests on llvmpipe through both Vulkan and OpenGL.

## Try it

```sh
cargo run -p embun --example counter     # a window with two glass buttons
cargo run -p embun --example photos      # a photo library with a glass sidebar and toolbar
cargo run -p embun-render --example snapshot -- --out glass.png
cargo test --workspace   # needs a GPU or a software one such as llvmpipe
```

`snapshot` writes a 1920×1200 image of four glass shapes over a generated backdrop of photo-like tiles. Add `--dark` for dark glass, or `--level 0` for the clear setting and `--level 1` for tinted.

## Not affiliated with Apple

Embun is an independent project. It uses no Apple code, fonts or symbols. The look is matched from screenshots of the macOS UI.

## License

MIT or Apache-2.0, at your option.
