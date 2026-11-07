use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Color;

/// One node of the UI: a box with children, a run of text or an image.
///
/// Build trees with [`div()`], [`row()`], [`column()`], [`text()`], [`image()`] and
/// [`spacer()`], and style them with chained methods named after Tailwind
/// classes: `.p_4()` is 16 px of padding, `.gap_2()` 8 px between children,
/// `.rounded_full()` a capsule. Sizes are in logical pixels.
pub struct Element<M> {
    pub(crate) kind: Kind,
    pub(crate) style: Style,
    pub(crate) children: Vec<Element<M>>,
    pub(crate) on_press: Option<M>,
    pub(crate) key: Option<String>,
    pub(crate) role: Role,
    pub(crate) label: Option<String>,
    /// For text fields: turns the edited text into a message.
    pub(crate) on_input: Option<Box<dyn Fn(String) -> M>>,
    /// For sliders: turns a position from 0 to 1 into a message.
    pub(crate) on_drag: Option<Box<dyn Fn(f32) -> M>>,
    /// On, checked or selected, for switches, checkboxes and tabs.
    pub(crate) checked: Option<bool>,
    /// A slider's current position, 0 to 1.
    pub(crate) value: Option<f32>,
    /// For lists: builds rows on demand; see [`list`].
    pub(crate) list: Option<std::rc::Rc<ListSpec<M>>>,
}

/// The rows of a [`list`] and how to build one.
pub(crate) struct ListSpec<M> {
    pub count: usize,
    pub row_height: f32,
    pub build: Box<dyn Fn(usize) -> Element<M>>,
}

/// Width of a slider's thumb. The thumb's center moves over the slider's
/// width minus this, so the pointer maps to a value the same way.
pub(crate) const SLIDER_THUMB: f32 = 28.0;

pub(crate) enum Kind {
    Div,
    Text(String),
    Image(Image),
    /// A single-line text field showing `value`, or `placeholder` when empty.
    Input { value: String, placeholder: String, multiline: bool },
}

/// What an element is, for focus, keyboard activation and screen readers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Role {
    #[default]
    Generic,
    Button,
    Toolbar,
    Navigation,
    Heading,
    Image,
    TextField,
    Menu,
    MenuItem,
    Switch,
    Checkbox,
    Radio,
    Tab,
    TabList,
    Slider,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) enum Length {
    #[default]
    Auto,
    Px(f32),
    /// A fraction of the parent, 0 to 1.
    Fraction(f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Direction {
    Row,
    #[default]
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Justify {
    Start,
    Center,
    End,
    Between,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextAlign {
    Left,
    Center,
    Right,
}

/// A glass surface: the element is drawn as a glass shape and its children
/// float on top of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Glass {
    pub tint: Option<Color>,
    /// Tinted with the theme's accent color, like a prominent button.
    pub accent: bool,
    /// Shapes with the same group melt together when close; `None` gives the
    /// shape a group of its own.
    pub group: Option<u32>,
    /// Grows and brightens while pressed, like a macOS glass control.
    pub interactive: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Style {
    pub direction: Direction,
    pub gap: f32,
    /// Top, right, bottom, left.
    pub padding: [f32; 4],
    pub width: Length,
    pub height: Length,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    pub grow: f32,
    pub no_shrink: bool,
    pub align_items: Option<Align>,
    pub align_self: Option<Align>,
    pub justify: Option<Justify>,
    pub wrap: bool,
    pub absolute: bool,
    /// Top, right, bottom, left offsets when absolute.
    pub inset: [Option<f32>; 4],
    pub background: Option<Color>,
    /// A vertical gradient from the first color at the top to the second.
    pub gradient: Option<(Color, Color)>,
    /// Corner radius; `f32::INFINITY` for a capsule.
    pub radius: f32,
    pub border: Option<(f32, Color)>,
    pub glass: Option<Glass>,
    pub scroll_y: bool,
    /// Draws above everything else, including glass, which it frosts.
    pub overlay: bool,
    /// Presses on it, or on children without their own handler, go nowhere
    /// instead of reaching an ancestor's `on_press`.
    pub capture: bool,
    pub font_size: Option<f32>,
    pub font_weight: Option<u16>,
    pub text_color: Option<Color>,
    pub text_align: Option<TextAlign>,
    pub line_height: Option<f32>,
}

/// RGBA8 pixels shown by [`image()`]. Cheap to clone; the GPU copy is made
/// once per image.
#[derive(Clone, Debug)]
pub struct Image {
    pub(crate) id: u64,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Arc<[u8]>,
}

impl Image {
    /// Tightly packed sRGB RGBA8 pixels, row by row.
    ///
    /// # Panics
    /// If `rgba.len() != width * height * 4` or either side is zero.
    pub fn from_rgba(width: u32, height: u32, rgba: impl Into<Arc<[u8]>>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let rgba = rgba.into();
        assert!(width > 0 && height > 0, "image must not be empty");
        assert_eq!(rgba.len(), width as usize * height as usize * 4, "image must be width * height * 4 bytes");
        Self { id: NEXT.fetch_add(1, Ordering::Relaxed), width, height, rgba }
    }

    /// Decodes a PNG or JPEG from memory, for example `include_bytes!`.
    /// JPEG orientation metadata is ignored.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ImageError> {
        let decoded = image::load_from_memory(bytes).map_err(|e| ImageError(e.to_string()))?.into_rgba8();
        let (width, height) = decoded.dimensions();
        if width == 0 || height == 0 {
            return Err(ImageError("the image is empty".into()));
        }
        Ok(Self::from_rgba(width, height, decoded.into_raw()))
    }

    /// Reads and decodes a PNG or JPEG file.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, ImageError> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(|e| ImageError(format!("{}: {e}", path.display())))?;
        Self::from_bytes(&bytes).map_err(|ImageError(e)| ImageError(format!("{}: {e}", path.display())))
    }

    pub fn size(&self) -> [u32; 2] {
        [self.width, self.height]
    }
}

/// Why an image could not be loaded: the file is missing, is not a PNG or
/// JPEG, or is damaged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageError(String);

impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot load image: {}", self.0)
    }
}

impl std::error::Error for ImageError {}

fn node<M>(kind: Kind) -> Element<M> {
    Element { kind, style: Style::default(), children: Vec::new(), on_press: None, key: None, role: Role::Generic, label: None, on_input: None, on_drag: None, checked: None, value: None, list: None }
}

/// A single-line text field. It shows `value`; when the user edits it,
/// `on_change` turns the new text into a message for your `update`, which
/// should store it so the next `view` passes it back.
///
/// Styled like a macOS text field: 28 px tall and 200 px wide unless you set
/// `w`, `h` or `grow`.
pub fn text_input<M: 'static>(value: impl Into<String>, on_change: impl Fn(String) -> M + 'static) -> Element<M> {
    let mut e = node(Kind::Input { value: value.into(), placeholder: String::new(), multiline: false });
    e.on_input = Some(Box::new(on_change));
    e.role = Role::TextField;
    e.style.width = Length::Px(200.0);
    e.style.height = Length::Px(28.0);
    e.style.padding = [0.0, 8.0, 0.0, 8.0];
    e.style.radius = 7.0;
    e
}

/// A multi-line text field. Return starts a new line, the text wraps at the
/// field's width, and Up and Down move between lines. It shows `value` and
/// reports edits like [`text_input`]; it is 240 px wide and 96 px tall unless
/// you set `w` and `h`, and scrolls to keep the caret in view.
pub fn text_area<M: 'static>(value: impl Into<String>, on_change: impl Fn(String) -> M + 'static) -> Element<M> {
    let mut e = text_input(value, on_change);
    if let Kind::Input { multiline, .. } = &mut e.kind {
        *multiline = true;
    }
    e.style.width = Length::Px(240.0);
    e.style.height = Length::Px(96.0);
    e.style.padding = [6.0, 8.0, 6.0, 8.0];
    e
}

/// A vertically scrolling list of `count` rows, each `row_height` logical
/// pixels tall, that builds and lays out only the rows on screen.
/// `row(index)` is called for those rows each time the view changes or the
/// list scrolls, so keep it cheap and make it depend only on `index` and your
/// app's state. Give the list a height with `h`, `h_full` or `grow`.
///
/// The list ignores `gap`; put spacing inside the rows. Heights are exact to
/// a pixel up to about a million rows of 16 px.
pub fn list<M: 'static>(count: usize, row_height: f32, row: impl Fn(usize) -> Element<M> + 'static) -> Element<M> {
    let mut e = node(Kind::Div);
    e.style.scroll_y = true;
    e.list = Some(std::rc::Rc::new(ListSpec { count, row_height, build: Box::new(row) }));
    e
}

/// A box that stacks its children top to bottom.
pub fn div<M>() -> Element<M> {
    node(Kind::Div)
}

/// A box that lays its children out left to right.
pub fn row<M>() -> Element<M> {
    let mut e = node(Kind::Div);
    e.style.direction = Direction::Row;
    e
}

/// A box that stacks its children top to bottom. Same as [`div`].
pub fn column<M>() -> Element<M> {
    node(Kind::Div)
}

/// A run of text. Size, weight and color are inherited from the parent
/// unless set here.
pub fn text<M>(content: impl Into<String>) -> Element<M> {
    node(Kind::Text(content.into()))
}

/// An image, by default at its pixel size in logical pixels. Set `w` or `h`
/// to scale it; it keeps its aspect ratio when only one is set.
pub fn image<M>(image: Image) -> Element<M> {
    let mut e = node(Kind::Image(image));
    e.role = Role::Image;
    e
}

/// Empty space that grows to fill the row or column.
pub fn spacer<M>() -> Element<M> {
    let mut e = node(Kind::Div);
    e.style.grow = 1.0;
    e
}

macro_rules! spacing {
    ($($n:literal => $p:ident $px:ident $py:ident $pt:ident $pr:ident $pb:ident $pl:ident $gap:ident;)*) => {
        $(
            #[doc = concat!("Padding of ", stringify!($n), " × 4 px on every side.")]
            pub fn $p(self) -> Self { self.padding($n as f32 * 4.0) }
            #[doc = concat!("Padding of ", stringify!($n), " × 4 px left and right.")]
            pub fn $px(self) -> Self { self.padding_x($n as f32 * 4.0) }
            #[doc = concat!("Padding of ", stringify!($n), " × 4 px top and bottom.")]
            pub fn $py(self) -> Self { self.padding_y($n as f32 * 4.0) }
            #[doc = concat!("Padding of ", stringify!($n), " × 4 px at the top.")]
            pub fn $pt(mut self) -> Self { self.style.padding[0] = $n as f32 * 4.0; self }
            #[doc = concat!("Padding of ", stringify!($n), " × 4 px on the right.")]
            pub fn $pr(mut self) -> Self { self.style.padding[1] = $n as f32 * 4.0; self }
            #[doc = concat!("Padding of ", stringify!($n), " × 4 px at the bottom.")]
            pub fn $pb(mut self) -> Self { self.style.padding[2] = $n as f32 * 4.0; self }
            #[doc = concat!("Padding of ", stringify!($n), " × 4 px on the left.")]
            pub fn $pl(mut self) -> Self { self.style.padding[3] = $n as f32 * 4.0; self }
            #[doc = concat!("A gap of ", stringify!($n), " × 4 px between children.")]
            pub fn $gap(self) -> Self { self.gap($n as f32 * 4.0) }
        )*
    };
}

impl<M> Element<M> {
    // Children.

    pub fn child(mut self, child: impl Into<Element<M>>) -> Self {
        self.children.push(child.into());
        self
    }

    pub fn children<E: Into<Element<M>>>(mut self, children: impl IntoIterator<Item = E>) -> Self {
        self.children.extend(children.into_iter().map(Into::into));
        self
    }

    // Behavior.

    /// Sends `message` to the app's `update` when the element is clicked or,
    /// once focused, activated with Space or Return. Makes it a button for
    /// keyboard focus and screen readers unless another role is set.
    pub fn on_press(mut self, message: M) -> Self {
        self.on_press = Some(message);
        if self.role == Role::Generic {
            self.role = Role::Button;
        }
        self
    }

    /// A stable identity for this element. Hover, press and scroll state
    /// follow the key when siblings are added or removed before it.
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn role(mut self, role: Role) -> Self {
        self.role = role;
        self
    }

    /// Gray text a [`text_input`] or [`text_area`] shows while empty.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        if let Kind::Input { placeholder: p, .. } = &mut self.kind {
            *p = placeholder.into();
        }
        self
    }

    /// The name a screen reader announces, when the visible text is not enough.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    // Layout.

    pub fn flex_row(mut self) -> Self {
        self.style.direction = Direction::Row;
        self
    }

    pub fn flex_col(mut self) -> Self {
        self.style.direction = Direction::Column;
        self
    }

    pub fn flex_wrap(mut self) -> Self {
        self.style.wrap = true;
        self
    }

    pub fn gap(mut self, px: f32) -> Self {
        self.style.gap = px;
        self
    }

    pub fn padding(mut self, px: f32) -> Self {
        self.style.padding = [px; 4];
        self
    }

    pub fn padding_x(mut self, px: f32) -> Self {
        self.style.padding[1] = px;
        self.style.padding[3] = px;
        self
    }

    pub fn padding_y(mut self, px: f32) -> Self {
        self.style.padding[0] = px;
        self.style.padding[2] = px;
        self
    }

    /// Padding at the top, in logical pixels.
    pub fn pt(mut self, px: f32) -> Self {
        self.style.padding[0] = px;
        self
    }

    /// Padding on the right, in logical pixels.
    pub fn pr(mut self, px: f32) -> Self {
        self.style.padding[1] = px;
        self
    }

    /// Padding at the bottom, in logical pixels.
    pub fn pb(mut self, px: f32) -> Self {
        self.style.padding[2] = px;
        self
    }

    /// Padding on the left, in logical pixels.
    pub fn pl(mut self, px: f32) -> Self {
        self.style.padding[3] = px;
        self
    }

    spacing! {
        0 => p_0 px_0 py_0 pt_0 pr_0 pb_0 pl_0 gap_0;
        1 => p_1 px_1 py_1 pt_1 pr_1 pb_1 pl_1 gap_1;
        2 => p_2 px_2 py_2 pt_2 pr_2 pb_2 pl_2 gap_2;
        3 => p_3 px_3 py_3 pt_3 pr_3 pb_3 pl_3 gap_3;
        4 => p_4 px_4 py_4 pt_4 pr_4 pb_4 pl_4 gap_4;
        5 => p_5 px_5 py_5 pt_5 pr_5 pb_5 pl_5 gap_5;
        6 => p_6 px_6 py_6 pt_6 pr_6 pb_6 pl_6 gap_6;
        8 => p_8 px_8 py_8 pt_8 pr_8 pb_8 pl_8 gap_8;
        10 => p_10 px_10 py_10 pt_10 pr_10 pb_10 pl_10 gap_10;
        12 => p_12 px_12 py_12 pt_12 pr_12 pb_12 pl_12 gap_12;
        16 => p_16 px_16 py_16 pt_16 pr_16 pb_16 pl_16 gap_16;
    }

    /// Width in logical pixels.
    pub fn w(mut self, px: f32) -> Self {
        self.style.width = Length::Px(px);
        self
    }

    /// Height in logical pixels.
    pub fn h(mut self, px: f32) -> Self {
        self.style.height = Length::Px(px);
        self
    }

    /// Width and height in logical pixels.
    pub fn size(self, width: f32, height: f32) -> Self {
        self.w(width).h(height)
    }

    pub fn w_full(mut self) -> Self {
        self.style.width = Length::Fraction(1.0);
        self
    }

    pub fn h_full(mut self) -> Self {
        self.style.height = Length::Fraction(1.0);
        self
    }

    /// Width as a fraction of the parent, 0 to 1.
    pub fn w_fraction(mut self, fraction: f32) -> Self {
        self.style.width = Length::Fraction(fraction);
        self
    }

    pub fn min_w(mut self, px: f32) -> Self {
        self.style.min_width = Some(px);
        self
    }

    pub fn min_h(mut self, px: f32) -> Self {
        self.style.min_height = Some(px);
        self
    }

    pub fn max_w(mut self, px: f32) -> Self {
        self.style.max_width = Some(px);
        self
    }

    pub fn max_h(mut self, px: f32) -> Self {
        self.style.max_height = Some(px);
        self
    }

    /// Takes a share of the free space in its row or column.
    pub fn grow(mut self) -> Self {
        self.style.grow = 1.0;
        self
    }

    /// Takes `share` of the free space in its row or column, relative to the
    /// other growing children: `grow_by(1.0)` next to `grow_by(3.0)` gets a quarter.
    pub fn grow_by(mut self, share: f32) -> Self {
        self.style.grow = share.max(0.0);
        self
    }

    /// Whether a switch or checkbox is on, or a tab is selected. Screen
    /// readers announce it.
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Makes the element a slider track at `value` (0 to 1). Pressing or
    /// dragging on it, or the arrow keys when it has focus, send
    /// `on_change(new_value)`. The element must be at least 28 logical pixels
    /// wide, the width of the thumb.
    pub fn on_drag(mut self, value: f32, on_change: impl Fn(f32) -> M + 'static) -> Self
    where
        M: 'static,
    {
        self.value = Some(value.clamp(0.0, 1.0));
        self.on_drag = Some(Box::new(on_change));
        self
    }

    /// Stops presses on this element, and on children that handle none, from
    /// reaching an ancestor's `on_press`. Glass does this already; use it for
    /// solid panels inside a [`popover`](crate::popover) or dialog.
    pub fn capture_presses(mut self) -> Self {
        self.style.capture = true;
        self
    }

    /// Keeps its size when its row or column runs out of space.
    pub fn shrink_0(mut self) -> Self {
        self.style.no_shrink = true;
        self
    }

    pub fn items_start(mut self) -> Self {
        self.style.align_items = Some(Align::Start);
        self
    }

    pub fn items_center(mut self) -> Self {
        self.style.align_items = Some(Align::Center);
        self
    }

    pub fn items_end(mut self) -> Self {
        self.style.align_items = Some(Align::End);
        self
    }

    pub fn items_stretch(mut self) -> Self {
        self.style.align_items = Some(Align::Stretch);
        self
    }

    pub fn self_center(mut self) -> Self {
        self.style.align_self = Some(Align::Center);
        self
    }

    pub fn self_start(mut self) -> Self {
        self.style.align_self = Some(Align::Start);
        self
    }

    pub fn self_end(mut self) -> Self {
        self.style.align_self = Some(Align::End);
        self
    }

    pub fn justify_start(mut self) -> Self {
        self.style.justify = Some(Justify::Start);
        self
    }

    pub fn justify_center(mut self) -> Self {
        self.style.justify = Some(Justify::Center);
        self
    }

    pub fn justify_end(mut self) -> Self {
        self.style.justify = Some(Justify::End);
        self
    }

    pub fn justify_between(mut self) -> Self {
        self.style.justify = Some(Justify::Between);
        self
    }

    /// Takes the element out of the flow and places it relative to its
    /// parent with [`top`](Self::top), [`left`](Self::left) and friends.
    /// Floating toolbars and sidebars sit over the content this way.
    pub fn absolute(mut self) -> Self {
        self.style.absolute = true;
        self
    }

    pub fn top(mut self, px: f32) -> Self {
        self.style.inset[0] = Some(px);
        self
    }

    pub fn right(mut self, px: f32) -> Self {
        self.style.inset[1] = Some(px);
        self
    }

    pub fn bottom(mut self, px: f32) -> Self {
        self.style.inset[2] = Some(px);
        self
    }

    pub fn left(mut self, px: f32) -> Self {
        self.style.inset[3] = Some(px);
        self
    }

    /// The same offset on all four sides.
    pub fn inset(mut self, px: f32) -> Self {
        self.style.inset = [Some(px); 4];
        self
    }

    /// Lifts the element and its children above everything else, glass
    /// included: glass inside it frosts the finished window beneath, so a
    /// menu over a toolbar shows the toolbar through it. Also makes the
    /// element absolute; place it with `top`, `left` and friends.
    pub fn overlay(mut self) -> Self {
        self.style.overlay = true;
        self.style.absolute = true;
        self
    }

    /// Scrolls the children vertically when they are taller than the element.
    pub fn scroll_y(mut self) -> Self {
        self.style.scroll_y = true;
        self
    }

    // Look.

    pub fn bg(mut self, color: Color) -> Self {
        self.style.background = Some(color);
        self
    }

    /// A vertical gradient, `top` color to `bottom` color.
    pub fn bg_gradient(mut self, top: Color, bottom: Color) -> Self {
        self.style.gradient = Some((top, bottom));
        self
    }

    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.style.border = Some((width, color));
        self
    }

    pub fn rounded(mut self, px: f32) -> Self {
        self.style.radius = px;
        self
    }

    pub fn rounded_sm(self) -> Self {
        self.rounded(2.0)
    }

    pub fn rounded_md(self) -> Self {
        self.rounded(6.0)
    }

    pub fn rounded_lg(self) -> Self {
        self.rounded(8.0)
    }

    pub fn rounded_xl(self) -> Self {
        self.rounded(12.0)
    }

    pub fn rounded_2xl(self) -> Self {
        self.rounded(16.0)
    }

    pub fn rounded_3xl(self) -> Self {
        self.rounded(24.0)
    }

    /// A capsule, or a circle when width and height are equal.
    pub fn rounded_full(self) -> Self {
        self.rounded(f32::INFINITY)
    }

    /// Draws the element as glass. Its children float on top of the glass.
    pub fn glass(mut self) -> Self {
        self.style.glass = Some(Glass { tint: None, accent: false, group: None, interactive: false });
        self
    }

    /// Glass tinted with `color`, like a prominent button.
    pub fn glass_tint(mut self, color: Color) -> Self {
        let glass = self.style.glass.get_or_insert(Glass { tint: None, accent: false, group: None, interactive: false });
        glass.tint = Some(color);
        self
    }

    /// Glass tinted with the theme's accent color: a prominent button.
    pub fn glass_prominent(mut self) -> Self {
        let glass = self.style.glass.get_or_insert(Glass { tint: None, accent: false, group: None, interactive: false });
        glass.accent = true;
        self
    }

    /// Glass in merge group `group`: glass elements in the same group melt
    /// together when they come within about 19 px. Without a group each
    /// glass element only touches its neighbors.
    pub fn glass_group(mut self, group: u32) -> Self {
        let glass = self.style.glass.get_or_insert(Glass { tint: None, accent: false, group: None, interactive: false });
        glass.group = Some(group);
        self
    }

    /// Glass that grows and brightens while pressed.
    pub fn glass_interactive(mut self) -> Self {
        let glass = self.style.glass.get_or_insert(Glass { tint: None, accent: false, group: None, interactive: false });
        glass.interactive = true;
        self
    }

    // Text.

    /// Font size in logical pixels.
    pub fn text_size(mut self, px: f32) -> Self {
        self.style.font_size = Some(px);
        self
    }

    pub fn text_xs(self) -> Self {
        self.text_size(11.0)
    }

    pub fn text_sm(self) -> Self {
        self.text_size(12.0)
    }

    pub fn text_base(self) -> Self {
        self.text_size(13.0)
    }

    pub fn text_lg(self) -> Self {
        self.text_size(15.0)
    }

    pub fn text_xl(self) -> Self {
        self.text_size(17.0)
    }

    pub fn text_2xl(self) -> Self {
        self.text_size(22.0)
    }

    pub fn text_3xl(self) -> Self {
        self.text_size(28.0)
    }

    pub fn text_4xl(self) -> Self {
        self.text_size(34.0)
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.style.text_color = Some(color);
        self
    }

    /// Font weight from 100 to 900; 400 is regular.
    pub fn font_weight(mut self, weight: u16) -> Self {
        self.style.font_weight = Some(weight);
        self
    }

    pub fn font_medium(self) -> Self {
        self.font_weight(500)
    }

    pub fn font_semibold(self) -> Self {
        self.font_weight(600)
    }

    pub fn font_bold(self) -> Self {
        self.font_weight(700)
    }

    pub fn text_center(mut self) -> Self {
        self.style.text_align = Some(TextAlign::Center);
        self
    }

    pub fn text_right(mut self) -> Self {
        self.style.text_align = Some(TextAlign::Right);
        self
    }

    /// Line height as a multiple of the font size.
    pub fn leading(mut self, multiple: f32) -> Self {
        self.style.line_height = Some(multiple);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(format: image::ImageFormat) -> Vec<u8> {
        // 3 x 2: red, green, blue on top; white, black, half-transparent gray below.
        let pixels = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255], [0, 0, 0, 255], [128, 128, 128, 128]];
        let rgba: Vec<u8> = pixels.iter().flatten().copied().collect();
        let img = image::RgbaImage::from_raw(3, 2, rgba).expect("pixels");
        let mut out = std::io::Cursor::new(Vec::new());
        match format {
            // JPEG has no alpha.
            image::ImageFormat::Jpeg => image::DynamicImage::ImageRgba8(img).to_rgb8().write_to(&mut out, format),
            _ => img.write_to(&mut out, format),
        }
        .expect("encode");
        out.into_inner()
    }

    #[test]
    fn png_bytes_decode_to_the_same_pixels() {
        let image = Image::from_bytes(&encode(image::ImageFormat::Png)).expect("png");
        assert_eq!(image.size(), [3, 2]);
        assert_eq!(&image.rgba[..4], [255, 0, 0, 255]);
        assert_eq!(&image.rgba[20..24], [128, 128, 128, 128], "alpha survives");
    }

    #[test]
    fn jpeg_bytes_decode_close_to_the_original_colors() {
        let image = Image::from_bytes(&encode(image::ImageFormat::Jpeg)).expect("jpeg");
        assert_eq!(image.size(), [3, 2]);
        assert_eq!(image.rgba[3], 255, "opaque");
        assert!(image.rgba[0] > 150 && image.rgba[1] < 120, "the first pixel is still reddish: {:?}", &image.rgba[..4]);
    }

    #[test]
    fn open_reads_a_file_and_errors_name_the_path_and_the_problem() {
        let dir = std::env::temp_dir().join(format!("embun-image-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let good = dir.join("a.png");
        std::fs::write(&good, encode(image::ImageFormat::Png)).expect("write");
        assert_eq!(Image::open(&good).expect("open").size(), [3, 2]);
        let missing = Image::open(dir.join("missing.png")).expect_err("a missing file");
        assert!(missing.to_string().contains("missing.png"), "{missing}");
        let bad = dir.join("b.png");
        std::fs::write(&bad, b"this is not an image").expect("write");
        let err = Image::open(&bad).expect_err("not an image");
        assert!(err.to_string().contains("b.png"), "{err}");
        assert!(Image::from_bytes(&[]).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
