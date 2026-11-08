# Changelog

## 0.3.0 - 2026-11-08

### Added

- `table`, with `Column`: a header row over a virtual list of rows, with fixed and growing column widths, right-aligned columns, sort arrows and pressable headers, row selection, and Up and Down to step the selection and scroll it into view. Screen readers see the table, rows, cells and headers.
- `dropdown`, a pop-up picker that opens a glass menu. `disclosure`, `badge`, `divider` and `vertical_divider`.
- `progress`, `progress_indeterminate` and `spinner`. The last two keep frames coming while they are on screen.
- `Element::tooltip` and `Button::tooltip`: a box near the pointer after it rests for 0.6 s.
- `Element::on_dismiss`: sends a message on Escape or on a press outside the element, and that press does nothing else. `Element::clip` cuts children at the element's edges and `Element::no_wrap` keeps text on one line.
- Left and Right move between tabs and segments, selecting each. Up and Down move between menu items.
- New roles `ProgressBar`, `Table`, `Row`, `ColumnHeader` and `Cell`.
- Docs pages for each of these, and a search box on the docs site.

### Changed

- Breaking: `Role` has five new variants, so an exhaustive `match` on it needs new arms.
- Overlays are no longer cut by a scrolling parent.

## 0.2.0 - 2026-11-08

### Added

- Controls: `switch`, `checkbox`, `slider` and `segmented`. Switches and checkboxes announce their on or off state to screen readers, segments announce which one is selected, and sliders report a value that screen readers can increment, decrement or set.
- `Element::on_drag` makes any element a slider: pressing or dragging it, or the arrow keys, Home and End when it has focus, send a position from 0 to 1. Also `Element::checked`, `Element::grow_by` and the roles `Switch`, `Checkbox`, `Tab`, `TabList` and `Slider`.
- `text_area`: a multi-line text field that wraps, starts a new line on Return, moves between lines with Up and Down, and scrolls to keep the caret in view. Screen readers see it as a multi-line text input.
- Option-Left and Option-Right move by word, Option-Backspace deletes a word, and Command-Up and Command-Down go to the start and end of the text. New `Key` variants: `Up`, `Down`, `SelectUp`, `SelectDown`, `WordLeft`, `WordRight`, `SelectWordLeft`, `SelectWordRight`, `DeleteWordBack`, `DocStart` and `DocEnd`.
- `list`: a scrolling list that builds and lays out only the visible rows, so a million rows scroll at about 0.12 ms per step on an Apple M5. The `list` example shows it.
- `Image::open` and `Image::from_bytes` load PNG and JPEG images, with an `ImageError` that names the file and the problem.
- `radio`, and `dialog` with `dialog_panel`: a modal over a dimmed window that closes on Escape or a click outside the panel. `Element::capture_presses` stops a solid panel's presses from reaching an ancestor's `on_press`.
- Dragging in a text field selects text, and the wheel scrolls a text area. At its limit, the wheel goes on to the page behind it.
- Switch knobs slide to the other end instead of jumping. The track color still changes at once.
- The window reads the system Reduce Transparency setting on macOS when it opens.
- The `controls` and `notes` examples, and a documentation site at https://fikrifirmanf.github.io/embun/ with every example rendered by Embun's own renderer, in light and dark, next to its code.

### Changed

- Breaking: `Settings::reduce_transparency` is now an `Option<bool>`; `None`, the default, follows the system. The builder method still takes a `bool`.
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
