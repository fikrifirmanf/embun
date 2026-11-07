use std::collections::HashMap;

use embun_motion::{Animated, Spring};

use crate::element::{Element, Role};
use crate::layout::{self, Rect4, ScrollInfo};
use crate::render::Renderer;
use crate::scene::{self, Highlight, Scene};
use crate::text::TextSystem;
use crate::Theme;

/// An Embun app: state, a way to change it, and a view of it.
///
/// `view` describes the whole UI for the current state; Embun calls it again
/// after every `update`. Interactions send the messages you attach with
/// [`Element::on_press`].
pub trait App {
    type Message: Clone;

    fn update(&mut self, message: Self::Message);

    fn view(&self, theme: &Theme) -> Element<Self::Message>;
}

/// A key press Embun handles itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Moves keyboard focus to the next control.
    Tab,
    /// Moves keyboard focus to the previous control.
    ShiftTab,
    /// Space or Return: activates the focused control.
    Activate,
    /// Clears keyboard focus.
    Escape,
}

/// What the pointer and focus can reach in the last layout.
#[derive(Clone, Debug)]
struct Target {
    id: u64,
    rect: Rect4,
    clip: Rect4,
    /// Glass and what sits on it draw over the content, so they are hit first.
    on_top: bool,
    press: Option<u64>,
    scroll: Option<u64>,
}

const PRESS: Spring = Spring { response: 0.2, damping_fraction: 0.8 };
const HOVER: Spring = Spring { response: 0.25, damping_fraction: 1.0 };

/// The window-independent heart of an Embun app: runs `view`, lays it out,
/// handles input and builds what to draw. A windowing shell (see
/// [`crate::run`]) feeds it events; tests drive it directly.
pub struct Ui<A: App> {
    app: A,
    theme: Theme,
    viewport: [f32; 2],
    text: TextSystem,
    view: Option<Element<A::Message>>,
    scroll: HashMap<u64, f32>,
    scroll_info: HashMap<u64, ScrollInfo>,
    targets: Vec<Target>,
    messages: HashMap<u64, A::Message>,
    focus_order: Vec<u64>,
    pointer: Option<[f32; 2]>,
    hovered: Option<u64>,
    pressed: Option<u64>,
    focused: Option<u64>,
    highlight: HashMap<u64, (Animated, Animated)>,
    scene: Scene,
    dirty: bool,
}

impl<A: App> Ui<A> {
    pub fn new(app: A, theme: Theme, viewport: [f32; 2]) -> Self {
        Self::with_text(app, theme, viewport, TextSystem::new())
    }

    pub fn with_text(app: A, theme: Theme, viewport: [f32; 2], text: TextSystem) -> Self {
        Self {
            app,
            theme,
            viewport,
            text,
            view: None,
            scroll: HashMap::new(),
            scroll_info: HashMap::new(),
            targets: Vec::new(),
            messages: HashMap::new(),
            focus_order: Vec::new(),
            pointer: None,
            hovered: None,
            pressed: None,
            focused: None,
            highlight: HashMap::new(),
            scene: Scene::default(),
            dirty: true,
        }
    }

    pub fn app(&self) -> &A {
        &self.app
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Sends a message to the app, as a press would.
    pub fn send(&mut self, message: A::Message) {
        self.app.update(message);
        self.view = None;
        self.dirty = true;
    }

    pub fn set_theme(&mut self, theme: Theme) {
        if theme != self.theme {
            self.theme = theme;
            self.view = None;
            self.dirty = true;
        }
    }

    /// The window size in logical pixels.
    pub fn resize(&mut self, viewport: [f32; 2]) {
        if viewport != self.viewport {
            self.viewport = viewport;
            self.dirty = true;
        }
    }

    pub fn viewport(&self) -> [f32; 2] {
        self.viewport
    }

    /// True when something changed since the last [`Ui::update`].
    pub fn needs_redraw(&self) -> bool {
        self.dirty
    }

    /// Lays out and rebuilds the scene if anything changed. Returns true if
    /// it did, meaning the window should redraw.
    pub fn update(&mut self) -> bool {
        if !self.dirty {
            return false;
        }
        self.dirty = false;
        if self.view.is_none() {
            self.view = Some(self.app.view(&self.theme));
        }
        let view = self.view.as_ref().expect("view");
        let mut nodes = layout::flatten(view, &self.theme);
        self.scroll_info = layout::layout(&mut nodes, self.viewport, &mut self.text, &self.scroll);

        self.targets.clear();
        self.messages.clear();
        self.focus_order.clear();
        let mut press_of: Vec<Option<u64>> = Vec::with_capacity(nodes.len());
        let mut scroll_of: Vec<Option<u64>> = Vec::with_capacity(nodes.len());
        for (i, n) in nodes.iter().enumerate() {
            let inherited = |v: &[Option<u64>]| n.parent.and_then(|p| v[p]);
            let press = if let Some(m) = &n.el.on_press {
                self.messages.insert(n.id, m.clone());
                if n.el.role == Role::Button {
                    self.focus_order.push(n.id);
                }
                Some(n.id)
            } else {
                inherited(&press_of)
            };
            let scroll = if n.el.style.scroll_y { Some(n.id) } else { inherited(&scroll_of) };
            press_of.push(press);
            scroll_of.push(scroll);
            self.targets.push(Target {
                id: n.id,
                rect: n.rect,
                clip: n.clip,
                on_top: n.on_glass.is_some() || n.el.style.glass.is_some(),
                press,
                scroll,
            });
            let _ = i;
        }
        // Forget state of controls that are gone.
        let alive: std::collections::HashSet<u64> = nodes.iter().map(|n| n.id).collect();
        self.highlight.retain(|id, _| alive.contains(id));
        for id in [&mut self.hovered, &mut self.pressed, &mut self.focused] {
            if id.is_some_and(|i| !alive.contains(&i)) {
                *id = None;
            }
        }
        self.scroll.retain(|id, _| alive.contains(id));

        let highlight: HashMap<u64, Highlight> =
            self.highlight.iter().map(|(&id, (press, hover))| (id, Highlight { press: press.value, hover: hover.value })).collect();
        self.scene = scene::build(&nodes, &self.theme, &highlight, self.focused);
        true
    }

    /// Advances animations by `dt` seconds. Returns true while any is moving.
    pub fn tick(&mut self, dt: f32) -> bool {
        let mut moving = false;
        for (press, hover) in self.highlight.values_mut() {
            moving |= press.step(dt);
            moving |= hover.step(dt);
        }
        self.highlight.retain(|_, (p, h)| !(p.is_settled() && h.is_settled() && p.value == 0.0 && h.value == 0.0));
        if moving {
            self.dirty = true;
        }
        moving
    }

    /// The control under `p`, if any.
    fn hit(&self, p: [f32; 2]) -> Option<&Target> {
        self.targets
            .iter()
            .enumerate()
            .filter(|(_, t)| layout::contains(t.rect, p) && layout::contains(t.clip, p))
            .max_by_key(|(i, t)| (t.on_top, *i))
            .map(|(_, t)| t)
    }

    fn animate(&mut self, id: u64, press: Option<f32>, hover: Option<f32>) {
        let (p, h) = self.highlight.entry(id).or_insert_with(|| (Animated::new(0.0, PRESS), Animated::new(0.0, HOVER)));
        if let Some(v) = press {
            p.target = v;
        }
        if let Some(v) = hover {
            h.target = v;
        }
        self.dirty = true;
    }

    /// The pointer moved to `p`, in logical pixels.
    pub fn pointer_moved(&mut self, p: [f32; 2]) {
        self.pointer = Some(p);
        let hovered = self.hit(p).and_then(|t| t.press);
        if hovered != self.hovered {
            if let Some(old) = self.hovered {
                self.animate(old, None, Some(0.0));
            }
            if let Some(new) = hovered {
                self.animate(new, None, Some(1.0));
            }
            self.hovered = hovered;
        }
        // Dragging off a pressed control releases its look, and back on restores it.
        if let Some(pressed) = self.pressed {
            let over = hovered == Some(pressed);
            self.animate(pressed, Some(if over { 1.0 } else { 0.0 }), None);
        }
    }

    pub fn pointer_left(&mut self) {
        self.pointer = None;
        if let Some(old) = self.hovered.take() {
            self.animate(old, Some(0.0), Some(0.0));
        }
    }

    pub fn pointer_pressed(&mut self) {
        let Some(p) = self.pointer else { return };
        self.pressed = self.hit(p).and_then(|t| t.press);
        if let Some(id) = self.pressed {
            self.animate(id, Some(1.0), None);
        }
        if self.focused.take().is_some() {
            self.dirty = true;
        }
    }

    /// Releasing over the control that was pressed activates it.
    pub fn pointer_released(&mut self) {
        let Some(id) = self.pressed.take() else { return };
        self.animate(id, Some(0.0), None);
        let over = self.pointer.and_then(|p| self.hit(p)).and_then(|t| t.press) == Some(id);
        if over && let Some(message) = self.messages.get(&id).cloned() {
            self.send(message);
        }
    }

    /// Scrolls the innermost scroll container under the pointer by `delta`
    /// logical pixels (positive moves the content up).
    pub fn scrolled(&mut self, delta: f32) {
        let Some(p) = self.pointer else { return };
        let Some(id) = self.hit(p).and_then(|t| t.scroll) else { return };
        let max = self.scroll_info.get(&id).map_or(0.0, |s| s.max);
        let offset = self.scroll.entry(id).or_insert(0.0);
        let next = (*offset + delta).clamp(0.0, max);
        if next != *offset {
            *offset = next;
            self.dirty = true;
        }
    }

    pub fn key(&mut self, key: Key) {
        match key {
            Key::Tab | Key::ShiftTab => {
                if self.focus_order.is_empty() {
                    return;
                }
                let n = self.focus_order.len();
                let current = self.focused.and_then(|f| self.focus_order.iter().position(|&id| id == f));
                let next = match (current, key) {
                    (None, Key::Tab) => 0,
                    (None, _) => n - 1,
                    (Some(i), Key::Tab) => (i + 1) % n,
                    (Some(i), _) => (i + n - 1) % n,
                };
                self.focused = Some(self.focus_order[next]);
                self.dirty = true;
            }
            Key::Activate => {
                if let Some(message) = self.focused.and_then(|id| self.messages.get(&id).cloned()) {
                    let id = self.focused.expect("focused");
                    // A short press flash.
                    if let Some((p, _)) = self.highlight.get_mut(&id) {
                        p.value = 1.0;
                    } else {
                        let mut press = Animated::new(1.0, PRESS);
                        press.target = 0.0;
                        self.highlight.insert(id, (press, Animated::new(0.0, HOVER)));
                    }
                    self.send(message);
                }
            }
            Key::Escape => {
                if self.focused.take().is_some() {
                    self.dirty = true;
                }
            }
        }
    }

    /// Lays out if needed and draws into `target` (`size` physical pixels at
    /// `scale_factor`).
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        renderer: &mut Renderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: [u32; 2],
        scale_factor: f32,
    ) {
        self.update();
        renderer.render(device, queue, encoder, target, size, scale_factor, &self.scene, &self.theme, &mut self.text);
    }

    #[cfg(test)]
    pub(crate) fn scene(&mut self) -> &Scene {
        self.update();
        &self.scene
    }

    /// The rectangle of the element with `key`, after the last update.
    pub fn rect_of_key(&mut self, key: &str) -> Option<Rect4> {
        self.update();
        let view = self.view.as_ref()?;
        let nodes = layout::flatten(view, &self.theme);
        let index = nodes.iter().position(|n| n.el.key.as_deref() == Some(key))?;
        let id = nodes[index].id;
        self.targets.iter().find(|t| t.id == id).map(|t| t.rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{column, div, text};

    struct Counter {
        count: i32,
    }

    #[derive(Clone)]
    enum Msg {
        Add,
        Sub,
    }

    impl App for Counter {
        type Message = Msg;
        fn update(&mut self, m: Msg) {
            match m {
                Msg::Add => self.count += 1,
                Msg::Sub => self.count -= 1,
            }
        }
        fn view(&self, _: &Theme) -> Element<Msg> {
            column()
                .p_4()
                .gap_2()
                .child(text(format!("{}", self.count)))
                .child(div().key("add").glass_interactive().h(30.0).on_press(Msg::Add).child(text("+")))
                .child(div().key("sub").glass_interactive().h(30.0).on_press(Msg::Sub).child(text("-")))
        }
    }

    fn ui() -> Ui<Counter> {
        Ui::new(Counter { count: 0 }, Theme::default(), [200.0, 200.0])
    }

    fn center(r: Rect4) -> [f32; 2] {
        [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
    }

    fn click(ui: &mut Ui<Counter>, p: [f32; 2]) {
        ui.pointer_moved(p);
        ui.pointer_pressed();
        ui.pointer_released();
    }

    #[test]
    fn clicking_a_control_sends_its_message() {
        let mut ui = ui();
        let add = ui.rect_of_key("add").expect("add button");
        click(&mut ui, center(add));
        click(&mut ui, center(add));
        // The text child of the button forwards the click to it.
        assert_eq!(ui.app().count, 2);
        let sub = ui.rect_of_key("sub").expect("sub button");
        click(&mut ui, [sub[0] + 2.0, sub[1] + 2.0]);
        assert_eq!(ui.app().count, 1);
    }

    #[test]
    fn releasing_away_from_the_pressed_control_cancels() {
        let mut ui = ui();
        let add = ui.rect_of_key("add").expect("add button");
        ui.pointer_moved(center(add));
        ui.pointer_pressed();
        ui.pointer_moved([190.0, 190.0]);
        ui.pointer_released();
        assert_eq!(ui.app().count, 0);
    }

    #[test]
    fn tab_focuses_controls_in_order_and_space_activates() {
        let mut ui = ui();
        ui.update();
        ui.key(Key::Tab);
        ui.key(Key::Tab);
        ui.key(Key::Activate);
        assert_eq!(ui.app().count, -1, "second control is Sub");
        ui.key(Key::Tab);
        ui.key(Key::Activate);
        assert_eq!(ui.app().count, 0, "focus wraps to Add");
        ui.key(Key::ShiftTab);
        ui.key(Key::Activate);
        assert_eq!(ui.app().count, -1);
    }

    #[test]
    fn pressing_animates_the_glass_and_settles_back() {
        let mut ui = ui();
        let add = ui.rect_of_key("add").expect("add button");
        let rest = ui.scene().glass[0].half_size;
        ui.pointer_moved(center(add));
        ui.pointer_pressed();
        for _ in 0..60 {
            ui.tick(1.0 / 60.0);
        }
        let down = ui.scene().glass[0].half_size;
        assert!(down[0] > rest[0] + 2.0, "pressed {down:?} vs rest {rest:?}");
        ui.pointer_released();
        ui.pointer_left();
        let mut frames = 0;
        while ui.tick(1.0 / 60.0) {
            frames += 1;
            assert!(frames < 600, "animation never settled");
        }
        assert_eq!(ui.scene().glass[0].half_size, rest);
    }

    #[test]
    fn wheel_scrolls_the_container_under_the_pointer_within_its_range() {
        struct List;
        impl App for List {
            type Message = ();
            fn update(&mut self, _: ()) {}
            fn view(&self, _: &Theme) -> Element<()> {
                column().child(column().key("list").h(100.0).scroll_y().children((0..10).map(|i| div().key(format!("row{i}")).h(30.0))))
            }
        }
        let mut ui = Ui::new(List, Theme::default(), [200.0, 300.0]);
        let first = ui.rect_of_key("row0").expect("row");
        ui.pointer_moved([50.0, 50.0]);
        ui.scrolled(45.0);
        assert_eq!(ui.rect_of_key("row0").expect("row")[1], first[1] - 45.0);
        ui.scrolled(10_000.0);
        assert_eq!(ui.rect_of_key("row0").expect("row")[1], first[1] - 200.0, "300 px of rows in a 100 px box");
        ui.pointer_moved([50.0, 250.0]);
        ui.scrolled(-50.0);
        assert_eq!(ui.rect_of_key("row0").expect("row")[1], first[1] - 200.0, "pointer outside the list");
    }
}
