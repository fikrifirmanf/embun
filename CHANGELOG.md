# Changelog

## 0.2.0 (unreleased)

### Added

- Controls: `switch`, `checkbox`, `slider` and `segmented`. Switches and checkboxes announce their on or off state to screen readers, segments announce which one is selected, and sliders report a value that screen readers can increment, decrement or set.
- `Element::on_drag` makes any element a slider: pressing or dragging it, or the arrow keys, Home and End when it has focus, send a position from 0 to 1. Also `Element::checked`, `Element::grow_by` and the roles `Switch`, `Checkbox`, `Tab`, `TabList` and `Slider`.
- `text_area`: a multi-line text field that wraps, starts a new line on Return, moves between lines with Up and Down, and scrolls to keep the caret in view. Screen readers see it as a multi-line text input.
- Option-Left and Option-Right move by word, Option-Backspace deletes a word, and Command-Up and Command-Down go to the start and end of the text. New `Key` variants: `Up`, `Down`, `SelectUp`, `SelectDown`, `WordLeft`, `WordRight`, `SelectWordLeft`, `SelectWordRight`, `DeleteWordBack`, `DocStart` and `DocEnd`.
- The `controls` and `notes` examples.

### Changed

- Home and End go to the start and end of the line. In a single-line field that is the whole text, as before.

### Fixed

- `Ui::accessibility_action` now acts on the UI as it is now. Two increments from a screen reader between frames used to start from the old value.

## 0.1.0 - 2026-11-07

### Added

- The `embun` framework: `App` (`update` and `view`), elements with Tailwind-style methods, layout with taffy, text in the embedded Inter font, images, glass `button`, `icon_button`, `toolbar_group`, `toolbar_item`, `sidebar` and `sidebar_item`, pointer, wheel and keyboard input with focus, spring press and hover animations, `run` for a winit window, and `snapshot` for tests.
- `text_input`: single-line text fields with selection, clipboard and input-method support.
- Menus and popovers: `popover`, `menu`, `menu_item`, `menu_check_item`, `menu_separator`, and `Element::overlay` for your own. Overlay glass frosts the finished frame beneath it.
- Screen reader support through AccessKit: roles, names, values, bounds, focus, and press and edit actions.
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
