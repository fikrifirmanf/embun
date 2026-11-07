//! Glass controls built from elements.
//!
//! Sizes follow macOS 27.0.1 glass buttons as captured in
//! `reference/macos-27.0.1/*/controls-*`: small and regular buttons are
//! rounded rectangles with a radius of a quarter of their height, large and
//! extra-large ones are capsules, and icon buttons are circles.

use crate::element::{Element, Role, div, row, text};

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
}

/// A glass button with a text label.
pub fn button<M>(label: impl Into<String>) -> Button<M> {
    Button { label: label.into(), size: ControlSize::Regular, prominent: false, circle: false, on_press: None, key: None, accessible_label: None }
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
