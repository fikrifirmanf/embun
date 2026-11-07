# Changelog

## Unreleased (0.0.3)

### Added

- Tinted glass for prominent buttons: `GlassShape::tint` and `GlassParams::tint`, fitted to macOS 27.0.1 `.glassProminent` buttons. Windows without focus draw them as plain glass, as macOS does.
- Merge groups: `GlassShape::group`. Only shapes in the same group melt together.
- `GlassShape::new` and the `appearance`, `tint` and `group` setters. `GlassShape` implements `Default`.
- The `bench` example, which times frames at 2880×1864.
- Tests on Vulkan and OpenGL through the llvmpipe software GPU. Set `WGPU_BACKEND` to choose the backend.

### Changed

- Focused glass blends sharp detail the way macOS does: light glass lightens, dark glass darkens. This adds `Body::blend`. Errors against macOS at clear, middle and tinted dropped from 2.5, 1.8 and 1.0 to 1.8, 1.4 and 0.9.
- Faster: the glass shader runs only near shapes, and the frost blur is reused until the backdrop changes. On an Apple M5 at 2880×1864, 4 buttons went from 1.53 to 0.26 ms and 16 buttons from 5.65 to 0.78 ms.
- `MAX_SHAPES` is 64, up from 16.
- Each `Backdrop` has its own uniform buffers, so frames over different backdrops can share one command encoder.
- Breaking: `GlassRenderer::regenerate_mips` no longer takes a device. Call it after drawing new content into the backdrop; it also makes the next `render` blur again.
- Breaking: `GlassShape`, `Body` and `GlassParams` have new public fields. Struct literals need the new fields or `..Default::default()`.

### Fixed

- Shapes with NaN or infinite values are skipped instead of corrupting the frame.
- Demo page: the "WebGL2 turned off" message no longer covers the canvas, and the round buttons no longer pick up the page's button style.

## 0.0.2

- Fitted to focused windows; `GlassParams::unfocused` keeps the look of a window without focus.

## 0.0.1

- First release: frost, lensing, outline, highlight and merging, fitted to macOS 27.0.1.
