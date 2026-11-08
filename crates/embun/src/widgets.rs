//! Glass controls built from elements.
//!
//! Sizes follow macOS 27.0.1 glass buttons as captured in
//! `reference/macos-27.0.1/*/controls-*`: small and regular buttons are
//! rounded rectangles with a radius of a quarter of their height, large and
//! extra-large ones are capsules, and icon buttons are circles.

use crate::Color;
use crate::Theme;
use crate::element::{Element, Role, SLIDER_THUMB, div, row, text};

/// The standard control sizes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ControlSize {
    Small,
    #[default]
    Regular,
    Large,
    ExtraLarge,
}

impl ControlSize {
    /// Button height in logical pixels.
    pub fn height(self) -> f32 {
        match self {
            ControlSize::Small => 20.0,
            ControlSize::Regular => 24.0,
            ControlSize::Large => 28.0,
            ControlSize::ExtraLarge => 36.0,
        }
    }

    fn radius(self) -> f32 {
        match self {
            ControlSize::Small | ControlSize::Regular => self.height() / 4.0,
            ControlSize::Large | ControlSize::ExtraLarge => f32::INFINITY,
        }
    }

    fn padding_x(self) -> f32 {
        match self {
            ControlSize::Small => 7.0,
            ControlSize::Regular => 12.0,
            ControlSize::Large => 14.0,
            ControlSize::ExtraLarge => 18.0,
        }
    }

    fn font_size(self) -> f32 {
        match self {
            ControlSize::Small => 11.0,
            ControlSize::Regular | ControlSize::Large => 13.0,
            ControlSize::ExtraLarge => 15.0,
        }
    }

    /// Diameter of a circular icon button.
    fn circle(self) -> f32 {
        match self {
            ControlSize::Small => 17.0,
            ControlSize::Regular => 21.0,
            ControlSize::Large => 25.0,
            ControlSize::ExtraLarge => 33.0,
        }
    }
}

/// A glass push button. Convert it into an [`Element`] by passing it to
/// `.child(...)`, or with `.into()`.
pub struct Button<M> {
    label: String,
    size: ControlSize,
    prominent: bool,
    circle: bool,
    on_press: Option<M>,
    key: Option<String>,
    accessible_label: Option<String>,
    tooltip: Option<String>,
}

/// A glass button with a text label.
pub fn button<M>(label: impl Into<String>) -> Button<M> {
    Button { label: label.into(), size: ControlSize::Regular, prominent: false, circle: false, on_press: None, key: None, accessible_label: None, tooltip: None }
}

/// A circular glass button holding one short glyph, such as "+" or "×".
/// Give it an accessible name with [`Button::label`].
pub fn icon_button<M>(glyph: impl Into<String>) -> Button<M> {
    Button { circle: true, ..button(glyph) }
}

impl<M> Button<M> {
    /// Tinted with the accent color, for the main action of a view.
    pub fn prominent(mut self) -> Self {
        self.prominent = true;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_press(mut self, message: M) -> Self {
        self.on_press = Some(message);
        self
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    /// Shows `text` near the pointer after it rests on the button; see
    /// [`Element::tooltip`].
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// The name a screen reader announces; defaults to the visible label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.accessible_label = Some(label.into());
        self
    }
}

impl<M> From<Button<M>> for Element<M> {
    fn from(b: Button<M>) -> Self {
        let size = b.size;
        let mut label = text(b.label.clone()).text_size(size.font_size());
        if b.prominent {
            // White on the accent color in both appearances.
            label = label.text_color(crate::Color::WHITE).font_medium();
        }
        let mut e = row().items_center().justify_center().glass_interactive().shrink_0().child(label);
        e = if b.circle {
            e.size(size.circle(), size.circle()).rounded_full()
        } else {
            e.h(size.height()).padding_x(size.padding_x()).rounded(size.radius())
        };
        if b.prominent {
            e = e.glass_prominent();
        }
        if let Some(m) = b.on_press {
            e = e.on_press(m);
        }
        if let Some(k) = b.key {
            e = e.key(k);
        }
        if let Some(t) = b.tooltip {
            e = e.tooltip(t);
        }
        e.role(Role::Button).label(b.accessible_label.unwrap_or(b.label))
    }
}

/// A glass capsule holding toolbar items side by side, like the button
/// groups in a macOS 27 window toolbar.
pub fn toolbar_group<M>() -> Element<M> {
    row().items_center().h(36.0).px_1().gap_1().rounded_full().glass().role(Role::Toolbar)
}

/// An item inside a [`toolbar_group`]: text on the group's glass that
/// highlights while hovered or pressed.
pub fn toolbar_item<M>(label: impl Into<String>, message: M) -> Element<M> {
    let label = label.into();
    row()
        .items_center()
        .justify_center()
        .h(28.0)
        .min_w(28.0)
        .px_2()
        .rounded_full()
        .child(text(label.clone()).text_size(13.0).font_medium())
        .on_press(message)
        .label(label)
}

/// A floating glass sidebar: a column inset from the window edge, like the
/// macOS 27 source list. Place it with `.absolute()` or in a row.
pub fn sidebar<M>() -> Element<M> {
    div().w(220.0).p_2().gap_1().rounded(18.0).glass().role(Role::Navigation)
}

/// A row in a [`sidebar`]. The selected row gets a soft fill.
pub fn sidebar_item<M>(label: impl Into<String>, selected: bool, message: M) -> Element<M> {
    let label = label.into();
    let mut e = row()
        .items_center()
        .h(28.0)
        .px_2()
        .rounded(8.0)
        .child(text(label.clone()).text_size(13.0))
        .on_press(message)
        .label(label);
    if selected {
        e = e.bg(crate::Color::rgba(120, 120, 128, 0.2)).font_medium();
    }
    e
}

/// A small gray heading over a group of sidebar rows.
pub fn sidebar_heading<M>(label: impl Into<String>) -> Element<M> {
    div().px_2().pt_3().pb_1().child(text(label).text_size(11.0).font_semibold()).role(Role::Heading)
}

/// A layer over the whole window holding `content` (usually a [`menu`]).
/// Clicking outside `content`, or pressing Escape, sends `dismiss`; your
/// `update` should then stop showing the popover. Place `content` with
/// `.absolute()` and `top`/`left`, for example just under the button that
/// opened it.
pub fn popover<M>(dismiss: M, content: impl Into<Element<M>>) -> Element<M> {
    div().overlay().inset(0.0).on_press(dismiss).role(Role::Generic).label("Dismiss").child(content)
}

/// A modal dialog: dims the window and centers `content`, usually a
/// [`dialog_panel`]. Clicking the dim area, or pressing Escape, sends
/// `dismiss`; your `update` should then stop showing it.
pub fn dialog<M>(dismiss: M, content: impl Into<Element<M>>) -> Element<M> {
    div()
        .overlay()
        .inset(0.0)
        .bg(Color::rgba(0, 0, 0, 0.28))
        .items_center()
        .justify_center()
        .on_press(dismiss)
        .role(Role::Generic)
        .label("Dismiss")
        .child(content)
}

/// The surface of a [`dialog`]: a solid rounded panel 300 px wide with room
/// for a title, a message and buttons. It is solid, not glass, so glass
/// buttons on it have something to frost.
pub fn dialog_panel<M>(theme: &Theme) -> Element<M> {
    div().w(300.0).p_5().gap_3().rounded(24.0).bg(theme.surface()).border(1.0, theme.separator()).capture_presses()
}

/// A glass menu panel: a column of [`menu_item`]s.
pub fn menu<M>() -> Element<M> {
    div().min_w(180.0).p_1().gap_0().rounded(12.0).glass().role(Role::Menu)
}

/// A row in a [`menu`].
pub fn menu_item<M>(label: impl Into<String>, message: M) -> Element<M> {
    let label = label.into();
    row()
        .items_center()
        .h(26.0)
        .px_2()
        .rounded(7.0)
        .child(text(label.clone()).text_size(13.0))
        .on_press(message)
        .role(Role::MenuItem)
        .label(label)
}

/// A menu row with a check mark column: checked rows show ✓, and labels
/// line up whether checked or not.
pub fn menu_check_item<M>(label: impl Into<String>, checked: bool, message: M) -> Element<M> {
    let label = label.into();
    let mark = div().w(18.0).shrink_0().child(text(if checked { "✓" } else { "" }).text_size(13.0));
    row()
        .items_center()
        .h(26.0)
        .px_2()
        .rounded(7.0)
        .child(mark)
        .child(text(label.clone()).text_size(13.0))
        .on_press(message)
        .role(Role::MenuItem)
        .label(if checked { format!("{label}, checked") } else { label })
}

/// A thin line between groups of menu items.
pub fn menu_separator<M>() -> Element<M> {
    div().h(9.0).px_2().justify_center().child(div().h(1.0).bg(crate::Color::rgba(128, 128, 128, 0.25)))
}

/// An on/off switch, 38 by 22 px: a track in the accent color when on, gray
/// when off, with a white knob. Pressing it sends `message`, which should
/// flip the value you pass next time. Name it for screen readers with
/// `.label(...)`.
pub fn switch<M>(theme: &Theme, on: bool, message: M) -> Element<M> {
    let track = if on { theme.accent() } else { Color::rgba(120, 120, 128, if theme.is_dark() { 0.4 } else { 0.32 }) };
    let knob = div().size(18.0, 18.0).rounded_full().bg(Color::WHITE).border(0.5, Color::rgba(0, 0, 0, 0.12));
    let mut e = row().w(38.0).h(22.0).shrink_0().padding(2.0).items_center().rounded_full().bg(track).child(knob).on_press(message);
    e = if on { e.justify_end() } else { e.justify_start() };
    e.role(Role::Switch).checked(on)
}

/// A checkbox with a text label. The whole row is the target; pressing it
/// sends `message`, which should flip the value you pass next time.
pub fn checkbox<M>(theme: &Theme, label: impl Into<String>, checked: bool, message: M) -> Element<M> {
    let label = label.into();
    let mut boxed = div().size(16.0, 16.0).shrink_0().rounded(4.0).items_center().justify_center();
    boxed = if checked {
        boxed.bg(theme.accent()).child(text("\u{2713}").text_size(11.0).font_bold().text_color(Color::WHITE))
    } else {
        boxed.bg(Color::rgba(128, 128, 128, 0.12)).border(1.0, Color::rgba(128, 128, 128, 0.55))
    };
    row()
        .items_center()
        .gap_2()
        .h(22.0)
        .child(boxed)
        .child(text(label.clone()).text_size(13.0))
        .on_press(message)
        .role(Role::Checkbox)
        .checked(checked)
        .label(label)
}

/// A radio button with a text label, for choosing one of several options.
/// Pressing it sends `message`; your `update` should select that option and
/// pass `selected` accordingly. Radio buttons in one group are not linked by
/// the toolkit: you decide which one is selected.
pub fn radio<M>(theme: &Theme, label: impl Into<String>, selected: bool, message: M) -> Element<M> {
    let label = label.into();
    let mut dot = div().size(16.0, 16.0).shrink_0().rounded_full().items_center().justify_center();
    dot = if selected {
        dot.bg(theme.accent()).child(div().size(6.0, 6.0).rounded_full().bg(Color::WHITE))
    } else {
        dot.bg(Color::rgba(128, 128, 128, 0.12)).border(1.0, Color::rgba(128, 128, 128, 0.55))
    };
    row()
        .items_center()
        .gap_2()
        .h(22.0)
        .child(dot)
        .child(text(label.clone()).text_size(13.0))
        .on_press(message)
        .role(Role::Radio)
        .checked(selected)
        .label(label)
}

/// A horizontal slider from 0 to 1, 200 px wide unless you set `w` or `grow`.
/// Pressing or dragging it, or the arrow keys when it has focus, send
/// `on_change` with the new value, which your `update` should store and pass
/// back as `value`. The thumb is a small glass pill over the track.
pub fn slider<M: 'static>(theme: &Theme, value: f32, on_change: impl Fn(f32) -> M + 'static) -> Element<M> {
    let value = if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 };
    let gray = Color::rgba(120, 120, 128, if theme.is_dark() { 0.4 } else { 0.28 });
    let fill = div().h(4.0).grow_by(value).bg(theme.accent());
    let rest = div().h(4.0).grow_by(1.0 - value).bg(gray);
    let thumb = div().size(SLIDER_THUMB, 20.0).shrink_0().rounded_full().glass();
    row()
        .w(200.0)
        .h(28.0)
        .items_center()
        .child(fill)
        .child(thumb)
        .child(rest)
        .on_drag(value, on_change)
        .role(Role::Slider)
}

/// A row of mutually exclusive choices on one glass capsule; the selected
/// one gets a soft pill. `on_select` turns the chosen index into a message.
pub fn segmented<M: Clone>(theme: &Theme, options: &[&str], selected: usize, on_select: impl Fn(usize) -> M) -> Element<M> {
    let pill = if theme.is_dark() { Color::rgba(255, 255, 255, 0.2) } else { Color::rgba(120, 120, 128, 0.2) };
    let segments = options.iter().enumerate().map(|(i, name)| {
        let mut e = row()
            .items_center()
            .justify_center()
            .h(24.0)
            .px_3()
            .rounded_full()
            .child(text(*name).text_size(13.0))
            .on_press(on_select(i))
            .role(Role::Tab)
            .checked(i == selected)
            .label(*name);
        if i == selected {
            e = e.bg(pill).font_medium();
        }
        e
    });
    row().items_center().self_start().h(28.0).padding(2.0).shrink_0().rounded_full().glass().role(Role::TabList).children(segments)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Theme;
    use crate::layout::{flatten, layout};
    use crate::scene::build;
    use crate::text::TextSystem;
    use std::collections::HashMap;

    #[test]
    fn button_sizes_match_the_macos_captures() {
        // Heights and corner radii from controls-solid-light-scene.txt.
        for (size, height, radius) in [
            (ControlSize::Small, 20.0, 5.0),
            (ControlSize::Regular, 24.0, 6.0),
            (ControlSize::Large, 28.0, 14.0),
            (ControlSize::ExtraLarge, 36.0, 18.0),
        ] {
            let root: Element<()> = crate::element::column().items_start().child(button("Button").size(size));
            let theme = Theme::default();
            let mut nodes = flatten(&root, &theme);
            layout(&mut nodes, [400.0, 200.0], &mut TextSystem::new(), &HashMap::new());
            let scene = build(&nodes, &theme, &HashMap::new(), None, None);
            let glass = scene.glass[0];
            assert_eq!(glass.half_size[1] * 2.0, height, "{size:?}");
            assert_eq!(glass.radius.min(glass.half_size[1]), radius, "{size:?}");
        }
    }

    #[test]
    fn the_selected_sidebar_row_gets_a_fill_on_the_glass() {
        let root: Element<()> = sidebar().child(sidebar_item("One", true, ())).child(sidebar_item("Two", false, ()));
        let theme = Theme::default();
        let mut nodes = flatten(&root, &theme);
        layout(&mut nodes, [400.0, 300.0], &mut TextSystem::new(), &HashMap::new());
        let scene = build(&nodes, &theme, &HashMap::new(), None, None);
        let fills: Vec<[f32; 4]> = scene.foreground.iter().filter_map(|p| match p {
            crate::scene::Prim::Rect { rect, .. } => Some(*rect),
            _ => None,
        }).collect();
        assert_eq!(fills.len(), 1, "one fill, for the selected row: {:?}", scene.foreground);
        assert_eq!(fills[0], nodes[1].rect);
    }

    #[test]
    fn prominent_buttons_take_the_accent_tint_for_the_appearance() {
        let root: Element<()> = crate::element::column().items_start().child(button("Save").prominent());
        for theme in [Theme::default(), Theme { appearance: crate::Appearance::Dark, ..Theme::default() }] {
            let mut nodes = flatten(&root, &theme);
            layout(&mut nodes, [400.0, 200.0], &mut TextSystem::new(), &HashMap::new());
            let tint = build(&nodes, &theme, &HashMap::new(), None, None).glass[0].tint;
            let accent = theme.accent();
            assert_eq!(tint, [accent.r, accent.g, accent.b, 1.0]);
        }
    }
}
