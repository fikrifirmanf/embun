//! Small pieces that are not controls: badges, dividers, progress and
//! disclosure groups.

use crate::Theme;
use crate::element::{Element, Motion, Role, column, div, row, text};

/// A small filled pill with a short label, such as an unread count. It is
/// 18 px tall in the accent color; override with `.bg(...)`.
pub fn badge<M>(theme: &Theme, label: impl Into<String>) -> Element<M> {
    div()
        .h(18.0)
        .min_w(18.0)
        .px_2()
        .shrink_0()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(theme.accent())
        .child(text(label).text_size(11.0).font_semibold().text_color(theme.on_accent()))
}

/// A 1 px horizontal line across its parent.
pub fn divider<M>(theme: &Theme) -> Element<M> {
    div().h(1.0).w_full().shrink_0().bg(theme.separator())
}

/// A 1 px vertical line down its parent.
pub fn vertical_divider<M>(theme: &Theme) -> Element<M> {
    div().w(1.0).h_full().shrink_0().bg(theme.separator())
}

/// A progress bar, 200 px wide and 6 px tall unless you set `w`. `value` runs
/// from 0 to 1; anything else is clamped. Screen readers get the value and,
/// with `.label(...)`, a name.
pub fn progress<M>(theme: &Theme, value: f32) -> Element<M> {
    let value = if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 };
    let track = crate::Color::rgba(120, 120, 128, if theme.is_dark() { 0.4 } else { 0.28 });
    let mut e = div()
        .w(200.0)
        .h(6.0)
        .shrink_0()
        .rounded_full()
        .bg(track)
        .child(div().h_full().w_fraction(value).rounded_full().bg(theme.accent()))
        .role(Role::ProgressBar);
    e.value = Some(value);
    e
}

/// A progress bar for work of unknown length: a segment sweeps back and forth.
/// 200 px wide and 6 px tall unless you set `w`. It reports no value to screen
/// readers, so give it a `.label(...)` such as "Loading".
pub fn progress_indeterminate<M>(theme: &Theme) -> Element<M> {
    let track = crate::Color::rgba(120, 120, 128, if theme.is_dark() { 0.4 } else { 0.28 });
    let mut e = div()
        .w(200.0)
        .h(6.0)
        .shrink_0()
        .rounded_full()
        .bg(track)
        .child(div().h_full().w_fraction(0.35).rounded_full().bg(theme.accent()).key("fill"))
        .role(Role::ProgressBar);
    e.motion = Motion::Sweep;
    e
}

/// An activity indicator for work of unknown length: three dots that hop one
/// after another. Give it a `.label(...)` such as "Loading" for screen readers.
pub fn spinner<M>(theme: &Theme) -> Element<M> {
    let dot = |k: usize| div().size(6.0, 6.0).shrink_0().rounded_full().bg(theme.secondary_label()).key(format!("dot{k}"));
    let mut e = row().h(16.0).items_end().gap(5.0).shrink_0().children((0..3).map(dot)).role(Role::ProgressBar);
    e.motion = Motion::Bounce;
    e
}

/// A heading that shows or hides `content` when pressed. Your app owns
/// `open`; `on_toggle` is sent when the heading is pressed and should flip it.
/// The content is indented under the title and left out of the tree while
/// closed.
pub fn disclosure<M>(theme: &Theme, title: impl Into<String>, open: bool, on_toggle: M, content: impl Into<Element<M>>) -> Element<M> {
    let title = title.into();
    let header = row()
        .items_center()
        .gap_2()
        .h(24.0)
        .child(div().w(12.0).shrink_0().child(text(if open { "\u{2304}" } else { "\u{203a}" }).text_size(13.0).font_bold().text_color(theme.secondary_label())))
        .child(text(title.clone()).text_size(13.0).font_medium())
        .on_press(on_toggle)
        .role(Role::Button)
        .label(format!("{title}, {}", if open { "expanded" } else { "collapsed" }));
    let mut group = column().child(header);
    if open {
        group = group.child(div().pl(20.0).child(content));
    }
    group
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{flatten, layout};
    use crate::text::TextSystem;
    use std::collections::HashMap;

    fn laid_out(root: &Element<()>) -> Vec<[f32; 4]> {
        let theme = Theme::default();
        let mut nodes = flatten(root, &theme);
        layout(&mut nodes, [400.0, 300.0], &mut TextSystem::new(), &HashMap::new());
        nodes.iter().map(|n| n.rect).collect()
    }

    #[test]
    fn a_progress_bar_fills_its_value_of_the_track() {
        let theme = Theme::default();
        for (value, expect) in [(0.25, 50.0), (0.0, 0.0), (1.0, 200.0), (1.7, 200.0), (-1.0, 0.0), (f32::NAN, 0.0)] {
            let root: Element<()> = column().items_start().child(progress(&theme, value));
            let rects = laid_out(&root);
            assert_eq!(rects[1][2], 200.0, "track");
            assert_eq!(rects[2][2], expect, "fill for {value}");
        }
    }

    #[test]
    fn a_closed_disclosure_leaves_its_content_out() {
        let theme = Theme::default();
        let content = || text::<()>("Inside");
        let closed = laid_out(&column().items_start().child(disclosure(&theme, "More", false, (), content())));
        let open = laid_out(&column().items_start().child(disclosure(&theme, "More", true, (), content())));
        assert!(open.len() > closed.len(), "open shows the content");
        let content_rect = open.last().copied().expect("content");
        assert!(content_rect[0] >= 20.0, "the content is indented under the title: {content_rect:?}");
    }

    #[test]
    fn a_badge_is_a_pill_that_grows_with_its_label() {
        let theme = Theme::default();
        let short = laid_out(&column().items_start().child(badge::<()>(&theme, "3")));
        let long = laid_out(&column().items_start().child(badge::<()>(&theme, "1,204")));
        assert_eq!(short[1][3], 18.0);
        assert!(long[1][2] > short[1][2], "{} vs {}", long[1][2], short[1][2]);
        assert!(short[1][2] >= 18.0);
    }

    use crate::{App, Key, Ui, button, menu, menu_item, segmented};

    struct Demo {
        tab: usize,
        picked: Vec<String>,
    }

    #[derive(Clone)]
    enum Msg {
        Tab(usize),
        Pick(&'static str),
    }

    impl App for Demo {
        type Message = Msg;

        fn update(&mut self, message: Msg) {
            match message {
                Msg::Tab(i) => self.tab = i,
                Msg::Pick(name) => self.picked.push(name.to_string()),
            }
        }

        fn view(&self, theme: &Theme) -> Element<Msg> {
            column()
                .items_start()
                .gap_3()
                .child(segmented(theme, &["A", "B", "C"], self.tab, Msg::Tab))
                .child(button("Save").tooltip("Saves the file").on_press(Msg::Pick("save")).key("save"))
                .child(spinner(theme).key("spin"))
                .child(progress_indeterminate(theme).key("bar"))
                .child(menu().child(menu_item("One", Msg::Pick("one")).key("one")).child(menu_item("Two", Msg::Pick("two")).key("two")))
        }
    }

    fn demo() -> Ui<Demo> {
        Ui::new(Demo { tab: 0, picked: Vec::new() }, Theme::default(), [400.0, 400.0])
    }

    fn center(r: [f32; 4]) -> [f32; 2] {
        [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
    }

    fn tip_boxes(ui: &mut Ui<Demo>) -> usize {
        ui.root_children()
    }

    #[test]
    fn a_tooltip_appears_after_the_pointer_rests_and_goes_when_it_leaves() {
        let mut ui = demo();
        let save = ui.rect_of_key("save").expect("save");
        let base = tip_boxes(&mut ui);
        ui.pointer_moved(center(save));
        ui.tick(0.3);
        assert_eq!(tip_boxes(&mut ui), base, "not yet after 0.3 s");
        ui.tick(0.4);
        assert_eq!(tip_boxes(&mut ui), base + 1, "shown after 0.7 s");
        let tip = ui.scene().clone();
        assert!(!tip.glass.is_empty() || !tip.content.is_empty());
        ui.pointer_moved([390.0, 390.0]);
        assert_eq!(tip_boxes(&mut ui), base, "gone once the pointer moves off");
        ui.pointer_moved(center(save));
        ui.tick(0.3);
        ui.pointer_left();
        ui.tick(0.5);
        assert_eq!(tip_boxes(&mut ui), base, "leaving the window cancels the wait");
    }

    #[test]
    fn pressing_hides_the_tooltip() {
        let mut ui = demo();
        let save = ui.rect_of_key("save").expect("save");
        let base = tip_boxes(&mut ui);
        ui.pointer_moved(center(save));
        ui.tick(0.7);
        assert_eq!(tip_boxes(&mut ui), base + 1);
        ui.pointer_pressed();
        assert_eq!(tip_boxes(&mut ui), base);
        ui.tick(0.3);
        assert_eq!(tip_boxes(&mut ui), base, "and it does not come straight back");
    }

    #[test]
    fn the_spinner_dots_hop_one_after_another() {
        let mut ui = demo();
        let spin = ui.rect_of_key("spin").expect("spinner");
        let floor = spin[1] + spin[3] - 6.0; // a dot at rest sits on the bottom edge
        let mut tallest = [0.0f32; 3];
        let mut steps = 0;
        for _ in 0..30 {
            assert!(ui.tick(0.05), "a spinner keeps frames coming");
            let ys: Vec<f32> = (0..3).map(|k| ui.rect_of_key(&format!("dot{k}")).expect("dot")[1]).collect();
            for (k, y) in ys.iter().enumerate() {
                assert!(*y <= floor + 0.01 && *y >= floor - 5.01, "dot {k} stays within its hop: {y} vs floor {floor}");
                tallest[k] = tallest[k].max(floor - y);
            }
            if (ys[0] - ys[2]).abs() > 0.5 {
                steps += 1;
            }
        }
        assert!(tallest.iter().all(|&h| h > 4.0), "every dot reaches the top of its hop: {tallest:?}");
        assert!(steps > 10, "the dots are mostly out of step with each other");
    }

    #[test]
    fn an_indeterminate_bar_sweeps_inside_its_track() {
        let mut ui = demo();
        let track = ui.rect_of_key("bar").expect("bar");
        let mut seen = Vec::new();
        for _ in 0..20 {
            ui.tick(0.1);
            let fill = ui.rect_of_key("fill").expect("fill");
            assert!(fill[0] >= track[0] - 0.01 && fill[0] + fill[2] <= track[0] + track[2] + 0.01, "{fill:?} inside {track:?}");
            seen.push(fill[0]);
        }
        let (lo, hi) = seen.iter().fold((f32::MAX, f32::MIN), |(l, h), &x| (l.min(x), h.max(x)));
        assert!(hi - lo > track[2] * 0.4, "it travels along the track: {lo}..{hi}");
    }

    #[test]
    fn without_a_spinner_no_frames_are_requested() {
        let mut ui = Ui::new(Demo2, Theme::default(), [100.0, 100.0]);
        assert!(!ui.tick(0.1));
    }

    struct Demo2;
    impl App for Demo2 {
        type Message = ();
        fn update(&mut self, _: ()) {}
        fn view(&self, _theme: &Theme) -> Element<()> {
            text("still")
        }
    }

    #[test]
    fn arrow_keys_move_between_tabs_and_select_them() {
        let mut ui = demo();
        ui.update();
        ui.key(Key::Tab);
        assert_eq!(ui.app().tab, 0);
        ui.key(Key::Right);
        assert_eq!(ui.app().tab, 1, "Right selects the next tab");
        ui.key(Key::Right);
        assert_eq!(ui.app().tab, 2);
        ui.key(Key::Right);
        assert_eq!(ui.app().tab, 2, "and stops at the last one");
        ui.key(Key::Left);
        assert_eq!(ui.app().tab, 1);
        ui.key(Key::Activate);
        assert_eq!(ui.app().tab, 1, "the focus followed the selection");
    }

    #[test]
    fn up_and_down_move_focus_between_menu_items() {
        let mut ui = demo();
        ui.update();
        for _ in 0..5 {
            ui.key(Key::Tab);
        }
        // Focus order: three tabs, Save, then the menu items.
        ui.key(Key::Activate);
        assert_eq!(ui.app().picked, ["one"], "fifth stop is the first menu item");
        ui.key(Key::Down);
        ui.key(Key::Activate);
        assert_eq!(ui.app().picked, ["one", "two"], "Down moved to the next item");
        ui.key(Key::Up);
        ui.key(Key::Activate);
        assert_eq!(ui.app().picked, ["one", "two", "one"]);
    }

    #[test]
    fn a_progress_bar_tells_screen_readers_its_value_and_offers_no_actions() {
        let theme = Theme::default();
        struct P(Theme);
        impl App for P {
            type Message = ();
            fn update(&mut self, _: ()) {}
            fn view(&self, _t: &Theme) -> Element<()> {
                progress(&self.0, 0.4).label("Upload")
            }
        }
        let mut ui = Ui::new(P(theme), theme, [300.0, 100.0]);
        let tree = ui.accessibility(1.0);
        let (_, bar) = tree.nodes.iter().find(|(_, n)| n.role() == accesskit::Role::ProgressIndicator).expect("a progress node");
        assert_eq!(bar.numeric_value(), Some(0.4f32 as f64));
        assert_eq!(bar.label(), Some("Upload"));
        assert!(!bar.supports_action(accesskit::Action::Increment), "a progress bar cannot be set");
    }
}
