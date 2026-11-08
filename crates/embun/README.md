# embun

A Rust UI toolkit with glass controls that match macOS 27. You describe the UI as a tree of elements, styled with methods named after Tailwind classes, and Embun lays it out, draws it with wgpu and sends your app a message when the user presses something.

```rust,no_run
use embun::*;

struct Counter {
    count: i32,
}

#[derive(Clone)]
enum Message {
    Increment,
    Decrement,
}

impl App for Counter {
    type Message = Message;

    fn update(&mut self, message: Message) {
        match message {
            Message::Increment => self.count += 1,
            Message::Decrement => self.count -= 1,
        }
    }

    fn view(&self, theme: &Theme) -> Element<Message> {
        column()
            .items_center()
            .justify_center()
            .gap_6()
            .bg_gradient(Color::hex(0xffd6a5), Color::hex(0xbde0fe))
            .child(text(self.count.to_string()).text_size(64.0).font_bold())
            .child(
                row()
                    .gap_3()
                    .child(button("Decrease").size(ControlSize::Large).on_press(Message::Decrement))
                    .child(button("Increase").size(ControlSize::Large).prominent().on_press(Message::Increment)),
            )
    }
}

fn main() -> Result<(), RunError> {
    run(Counter { count: 0 }, Settings::new("Counter").size(420.0, 300.0))
}
```

Documentation, with each example rendered next to its code: <https://fikrifirmanf.github.io/embun/>. API reference: <https://docs.rs/embun>.

## How it draws

Each frame has three layers:

1. **Content.** Everything that is not glass: backgrounds, text, images. It is drawn into the backdrop texture, and only when it changes.
2. **Glass.** Every element marked `.glass()` becomes a glass shape drawn by [`embun-render`](https://crates.io/crates/embun-render) over the content.
3. **On the glass.** The children of glass elements: labels and icons on buttons, toolbar items, sidebar rows.

Pressing or hovering a glass button only changes the glass layer, so the frost blur is reused and the frame stays cheap. It also reuses the last layout. On an Apple M5, a UI of 5,000 elements takes 0.38 ms to update after a hover change and 3.9 ms for a full layout, which runs after a message, a resize or a scroll.

## What is there

- `list`: a scrolling list that builds and lays out only the rows on screen. A million rows of 32 px scroll at about 0.12 ms per step on an Apple M5.
- Layout: rows, columns, padding, gap, grow, alignment, wrapping, absolute positioning and vertical scrolling, through [taffy](https://crates.io/crates/taffy).
- Text in Inter, embedded so it measures the same everywhere. Inter is under the SIL Open Font License; see `fonts/OFL.txt`.
- Images from RGBA8 pixels, or from PNG and JPEG files and bytes with `Image::open` and `Image::from_bytes`.
- `text_input` and `text_area`: fields your app owns the value of. Typing, Backspace and Delete, arrow keys, Option-arrows to move by word, Option-Backspace, Home and End, Command-Up and Command-Down, Shift to select, Command-A, copy, cut and paste, clicking or dragging to place the cursor and select, and input methods for languages such as Japanese. A text area wraps long lines, starts a new line on Return, moves between lines with Up and Down, scrolls with the wheel, and follows the caret.
- Controls: `button`, `icon_button`, prominent buttons, `toolbar_group` and `toolbar_item`, `sidebar`, `sidebar_item` and `sidebar_heading`. Button sizes follow the macOS 27.0.1 captures.
- `switch`, `checkbox`, `radio`, `slider` and `segmented`, and a `dialog` that dims the window. Sliders take presses, drags and arrow keys, and every one of these tells screen readers its state. Put them on a solid or content background: glass over glass has nothing to frost, so keep the glass for buttons, thumbs and bars.
- `progress`, `progress_indeterminate` and `spinner`, `badge`, `divider`, `disclosure`, `dropdown` and `tooltip`. A `table` with a sticky header, sortable columns, row selection and arrow keys, built on `list` so a million rows cost the same as ten.
- Menus and popovers: `popover`, `menu`, `menu_item`, `menu_check_item` and `menu_separator`. An overlay's glass frosts the finished window beneath it, so a menu over a toolbar shows the toolbar through it. Clicking outside or pressing Escape closes it.
- Pointer, scroll wheel and trackpad input. Tab and Shift-Tab move keyboard focus between buttons, and Space or Return presses the focused one.
- Light and dark mode from the system, solid surfaces when Reduce Transparency is on (macOS), and glass that frosts more when the window loses focus, as on macOS.
- Screen readers (VoiceOver, Narrator, Orca) through [AccessKit](https://accesskit.dev): every element is in the accessibility tree with its role, name and bounds, buttons can be pressed and text fields edited from the screen reader, and keyboard focus is shared with it.
- `snapshot` renders an app into memory, for tests and screenshots.

Not there yet: the framework in a browser and double-click to select a word. Tables cannot resize columns, and tooltips do not show for keyboard focus. Reduce Transparency is read only on macOS and only when the window opens. The cursor moves by character, so an emoji made of several characters takes several presses.

## Examples

```sh
cargo run -p embun --example counter
cargo run -p embun --example photos
cargo run -p embun --example controls
cargo run -p embun --example notes
cargo run --release -p embun --example list
cargo run -p embun --example gallery -- out/   # renders PNGs, no window
```

Embun is not affiliated with Apple. It uses no Apple code, fonts or symbols.

License: MIT or Apache-2.0, at your option. The embedded Inter font is under the SIL Open Font License 1.1.
