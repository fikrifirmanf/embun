# Embun

Embun is a Rust UI toolkit built on wgpu. It draws translucent glass controls that match the look of macOS 27, on any platform wgpu runs on. Embun is Indonesian for dew.

**Status: early.** The glass renderer and the spring animations work. The widget framework is not written yet, so there are no buttons or windows to use.

| Crate | What it does |
|---|---|
| [`embun-render`](crates/embun-render) | Draws glass shapes over a backdrop into any wgpu texture. Frost, edge bending, tint, outline, and shapes that merge when they get close. |
| [`embun-motion`](crates/embun-motion) | Springs, described the SwiftUI way with a response time and a damping fraction. |
| [`embun`](crates/embun) | The framework, later. For now it re-exports the two crates above. |

## How close it is to macOS

The look is measured, not guessed. `tools/capture` shows real macOS 27.0.1 glass on a known backdrop and screenshots it. The `compare` example draws Embun's glass over the same backdrop and diffs the two images.

Captures cover the three transparency settings (clear, middle, tinted), light and dark, over photos, a checkerboard, random noise and a flat color. macOS draws glass differently when its window has focus, so both looks are captured and fitted:

| Window | Clear | Middle | Tinted |
|---|---|---|---|
| Focused | 1.8 | 1.4 | 0.9 |
| Without focus | 1.1 | 0.9 | 0.5 |

Each number is the average per-pixel difference, out of 255. Most of what is left sits in the one-pixel ring at the edge of each shape and over very busy backdrops such as random noise.

Real glass buttons are captured too, but not yet toolbars, sidebars or menus.

## Platforms

| Backend | Tested |
|---|---|
| Metal (macOS) | yes, all tests and captures |
| WebGPU in Chrome | yes, the demo page |
| WebGL2 in Chrome | yes, the demo page with WebGPU turned off |
| Vulkan, DirectX 12 | not yet |

## Try it

```sh
cargo run -p embun-render --example snapshot -- --out glass.png
cargo test --workspace   # needs a GPU
```

`snapshot` writes a 1920×1200 image of four glass shapes over a generated backdrop of photo-like tiles. Add `--dark` for dark glass, or `--level 0` for the clear setting and `--level 1` for tinted.

## Not affiliated with Apple

Embun is an independent project. It uses no Apple code, fonts or symbols. The look is matched from screenshots of the macOS UI.

## License

MIT or Apache-2.0, at your option.
