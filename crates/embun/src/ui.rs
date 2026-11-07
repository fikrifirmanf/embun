use std::collections::HashMap;

use embun_motion::{Animated, Spring};

use crate::element::{Element, Kind, Role, SLIDER_THUMB};
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

/// A key press Embun handles itself. Typed characters go to
/// [`Ui::insert_text`] instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Key {
    /// Moves keyboard focus to the next control.
    Tab,
    /// Moves keyboard focus to the previous control.
    ShiftTab,
    /// Space or Return: activates the focused control.
    Activate,
    /// Clears keyboard focus.
    Escape,
    Backspace,
    Delete,
    Left,
    Right,
    /// Start of the line.
    Home,
    /// End of the line.
    End,
    /// Left, Right, Home and End with Shift: extend the selection.
    SelectLeft,
    SelectRight,
    SelectHome,
    SelectEnd,
    SelectAll,
    /// Up and Down: between lines of a text area; to the start or end of a single-line field.
    Up,
    Down,
    SelectUp,
    SelectDown,
    /// Option-Left and Option-Right: by word.
    WordLeft,
    WordRight,
    SelectWordLeft,
    SelectWordRight,
    /// Option-Backspace: deletes the word before the caret.
    DeleteWordBack,
    /// Command-Up and Command-Down: start and end of the whole text.
    DocStart,
    DocEnd,
}

/// The text field being edited: cursor and the other end of the selection,
/// as byte offsets into its value.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Edit {
    id: u64,
    cursor: usize,
    anchor: usize,
}

/// One element as a screen reader sees it.
#[derive(Clone, Debug)]
struct AccessNode {
    id: u64,
    role: accesskit::Role,
    label: Option<String>,
    value: Option<String>,
    rect: Rect4,
    children: Vec<u64>,
    clickable: bool,
    focusable: bool,
    toggled: Option<bool>,
    selected: Option<bool>,
    numeric: Option<f32>,
}

/// The id of the window node that holds the app's root element.
const WINDOW_NODE: u64 = 1;

/// A text field's change handler.
type OnChange<M> = dyn Fn(String) -> M;

/// What the pointer and focus can reach in the last layout.
#[derive(Clone, Debug)]
struct Target {
    id: u64,
    rect: Rect4,
    clip: Rect4,
    /// Glass and what sits on it draw over the content, so they are hit first.
    on_top: bool,
    /// Overlays draw over everything, so they are hit before anything else.
    overlay: bool,
    press: Option<u64>,
    scroll: Option<u64>,
    /// The slider this element belongs to, if any.
    drag: Option<u64>,
    input: bool,
}

/// How far an arrow key, or a screen reader's increment, moves a slider.
const SLIDER_STEP: f32 = 0.05;

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
    /// The slider being dragged.
    dragging: Option<u64>,
    focused: Option<u64>,
    edit: Option<Edit>,
    /// Rects and clips from the last layout, in pre-order. Hover, press,
    /// focus and caret changes reuse them; anything that can move elements
    /// clears them.
    layout_cache: Option<(Vec<Rect4>, Vec<Rect4>)>,
    /// The topmost overlay with an `on_press`: Escape and clicks outside it send its message.
    dismiss: Option<u64>,
    access: Vec<AccessNode>,
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
            dragging: None,
            focused: None,
            edit: None,
            layout_cache: None,
            dismiss: None,
            access: Vec::new(),
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
        self.layout_cache = None;
        self.dirty = true;
    }

    pub fn set_theme(&mut self, theme: Theme) {
        if theme != self.theme {
            self.theme = theme;
            self.view = None;
            self.layout_cache = None;
            self.dirty = true;
        }
    }

    /// The window size in logical pixels.
    pub fn resize(&mut self, viewport: [f32; 2]) {
        if viewport != self.viewport {
            self.viewport = viewport;
            self.layout_cache = None;
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
        match &self.layout_cache {
            Some((rects, clips)) if rects.len() == nodes.len() => {
                for ((n, rect), clip) in nodes.iter_mut().zip(rects).zip(clips) {
                    n.rect = *rect;
                    n.clip = *clip;
                }
            }
            _ => {
                self.scroll_info = layout::layout(&mut nodes, self.viewport, &mut self.text, &self.scroll);
                self.layout_cache = Some((nodes.iter().map(|n| n.rect).collect(), nodes.iter().map(|n| n.clip).collect()));
            }
        }

        self.targets.clear();
        self.messages.clear();
        self.dismiss = None;
        self.focus_order.clear();
        let mut press_of: Vec<Option<u64>> = Vec::with_capacity(nodes.len());
        let mut scroll_of: Vec<Option<u64>> = Vec::with_capacity(nodes.len());
        let mut drag_of: Vec<Option<u64>> = Vec::with_capacity(nodes.len());
        for (i, n) in nodes.iter().enumerate() {
            let inherited = |v: &[Option<u64>]| n.parent.and_then(|p| v[p]);
            if n.el.role == Role::TextField || n.el.on_drag.is_some() {
                self.focus_order.push(n.id);
            }
            let press = if let Some(m) = &n.el.on_press {
                self.messages.insert(n.id, m.clone());
                if matches!(n.el.role, Role::Button | Role::MenuItem | Role::Switch | Role::Checkbox | Role::Tab) {
                    self.focus_order.push(n.id);
                }
                if n.el.style.overlay {
                    self.dismiss = Some(n.id);
                }
                Some(n.id)
            } else if n.el.style.glass.is_some() {
                // A click on a glass panel's empty space stays on the panel
                // instead of reaching whatever is behind it.
                None
            } else {
                inherited(&press_of)
            };
            let scroll = if n.el.style.scroll_y { Some(n.id) } else { inherited(&scroll_of) };
            let drag = if n.el.on_drag.is_some() { Some(n.id) } else { inherited(&drag_of) };
            press_of.push(press);
            scroll_of.push(scroll);
            drag_of.push(drag);
            self.targets.push(Target {
                id: n.id,
                rect: n.rect,
                clip: n.clip,
                on_top: n.on_glass.is_some() || n.el.style.glass.is_some(),
                overlay: n.in_overlay,
                press,
                scroll,
                drag,
                input: n.el.on_input.is_some(),
            });
            let _ = i;
        }
        self.access = nodes
            .iter()
            .map(|n| AccessNode { children: n.children.iter().map(|&c| nodes[c].id).collect(), ..access_node(n) })
            .collect();
        // Forget state of controls that are gone.
        let alive: std::collections::HashSet<u64> = nodes.iter().map(|n| n.id).collect();
        self.highlight.retain(|id, _| alive.contains(id));
        for id in [&mut self.hovered, &mut self.pressed, &mut self.focused] {
            if id.is_some_and(|i| !alive.contains(&i)) {
                *id = None;
            }
        }
        if self.edit.is_some_and(|e| Some(e.id) != self.focused) {
            self.edit = None;
        }
        self.scroll.retain(|id, _| alive.contains(id));

        let highlight: HashMap<u64, Highlight> =
            self.highlight.iter().map(|(&id, (press, hover))| (id, Highlight { press: press.value, hover: hover.value })).collect();
        let caret = self.edit.and_then(|edit| {
            let node = nodes.iter().find(|n| n.id == edit.id)?;
            let Kind::Input { value, multiline, .. } = &node.el.kind else { return None };
            let cursor = edit.cursor.min(value.len());
            let anchor = edit.anchor.min(value.len());
            let (_, wrap) = layout::field_geometry(node.rect, &node.el.style, &node.text, *multiline, [0.0; 2]);
            let pos = self.text.caret(value, &node.text, wrap, cursor);
            let selection =
                if cursor != anchor { self.text.selection_rects(value, &node.text, wrap, cursor.min(anchor), cursor.max(anchor)) } else { Vec::new() };
            let [pad_top, pad_right, pad_bottom, pad_left] = node.el.style.padding;
            let (inner_w, inner_h) = (node.rect[2] - pad_left - pad_right, node.rect[3] - pad_top - pad_bottom);
            let line = node.text.size * node.text.line_height;
            // Shift the text so the caret stays inside the field.
            let scroll = if *multiline { [0.0, (pos[1] + line - inner_h).max(0.0)] } else { [(pos[0] - inner_w + 2.0).max(0.0), 0.0] };
            Some(scene::Caret { id: edit.id, pos, selection, scroll })
        });
        self.scene = scene::build(&nodes, &self.theme, &highlight, self.focused, caret.as_ref());
        true
    }

    /// The whole UI as an accessibility tree for screen readers, with bounds
    /// in physical pixels for a window at `scale_factor`.
    pub fn accessibility(&mut self, scale_factor: f32) -> accesskit::TreeUpdate {
        self.update();
        let s = scale_factor as f64;
        let mut nodes = Vec::with_capacity(self.access.len() + 1);
        let mut window = accesskit::Node::new(accesskit::Role::Window);
        if let Some(root) = self.access.first() {
            window.set_children(vec![accesskit::NodeId(root.id)]);
        }
        nodes.push((accesskit::NodeId(WINDOW_NODE), window));
        for a in &self.access {
            let mut node = accesskit::Node::new(a.role);
            if let Some(label) = &a.label {
                node.set_label(label.clone());
            }
            if let Some(value) = &a.value {
                node.set_value(value.clone());
            }
            let [x, y, w, h] = a.rect;
            node.set_bounds(accesskit::Rect { x0: x as f64 * s, y0: y as f64 * s, x1: (x + w) as f64 * s, y1: (y + h) as f64 * s });
            node.set_children(a.children.iter().map(|&c| accesskit::NodeId(c)).collect::<Vec<_>>());
            if a.clickable {
                node.add_action(accesskit::Action::Click);
            }
            if a.focusable {
                node.add_action(accesskit::Action::Focus);
            }
            if let Some(on) = a.toggled {
                node.set_toggled(if on { accesskit::Toggled::True } else { accesskit::Toggled::False });
            }
            if let Some(selected) = a.selected {
                node.set_selected(selected);
            }
            if let Some(v) = a.numeric {
                node.set_numeric_value(v as f64);
                node.set_min_numeric_value(0.0);
                node.set_max_numeric_value(1.0);
                node.set_numeric_value_step(SLIDER_STEP as f64);
                node.add_action(accesskit::Action::SetValue);
                node.add_action(accesskit::Action::Increment);
                node.add_action(accesskit::Action::Decrement);
            }
            if is_text_input(a.role) {
                node.add_action(accesskit::Action::SetValue);
                node.add_action(accesskit::Action::ReplaceSelectedText);
            }
            nodes.push((accesskit::NodeId(a.id), node));
        }
        let mut tree = accesskit::TreeInfo::new(accesskit::NodeId(WINDOW_NODE));
        tree.toolkit_name = Some("Embun".into());
        tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").into());
        accesskit::TreeUpdate {
            nodes,
            tree: Some(tree),
            tree_id: accesskit::TreeId::ROOT,
            focus: accesskit::NodeId(self.focused.unwrap_or(WINDOW_NODE)),
        }
    }

    /// Carries out an action a screen reader asked for: pressing a button,
    /// moving focus, or changing a text field.
    pub fn accessibility_action(&mut self, request: &accesskit::ActionRequest) {
        // Act on the UI as it is now: several increments can arrive between frames.
        self.update();
        let id = request.target_node.0;
        let Some(node) = self.access.iter().find(|a| a.id == id).cloned() else { return };
        match request.action {
            accesskit::Action::Click if node.clickable => {
                if let Some(message) = self.messages.get(&id).cloned() {
                    self.send(message);
                }
            }
            accesskit::Action::Focus if node.focusable => self.focus(Some(id)),
            accesskit::Action::Increment | accesskit::Action::Decrement | accesskit::Action::SetValue if node.numeric.is_some() => {
                let current = node.numeric.unwrap_or(0.0);
                let next = match (&request.action, &request.data) {
                    (accesskit::Action::Increment, _) => current + SLIDER_STEP,
                    (accesskit::Action::Decrement, _) => current - SLIDER_STEP,
                    (_, Some(accesskit::ActionData::NumericValue(v))) => *v as f32,
                    _ => return,
                };
                self.set_slider(id, next);
            }
            accesskit::Action::SetValue | accesskit::Action::ReplaceSelectedText if is_text_input(node.role) => {
                let Some(accesskit::ActionData::Value(text)) = &request.data else { return };
                if self.focused != Some(id) {
                    self.focus(Some(id));
                }
                if request.action == accesskit::Action::SetValue {
                    self.key(Key::SelectAll);
                }
                let text = text.to_string();
                self.insert_text(&text);
            }
            _ => {}
        }
    }

    /// True while a text field has keyboard focus: typed text goes to it.
    pub fn is_editing(&self) -> bool {
        self.edit.is_some()
    }

    /// Runs `view` again if a message invalidated the last one.
    fn ensure_view(&mut self) {
        if self.view.is_none() {
            self.view = Some(self.app.view(&self.theme));
        }
    }

    /// The value and change handler of the text field `id`.
    fn input(&self, id: u64) -> Option<(String, &OnChange<A::Message>, bool)> {
        let view = self.view.as_ref()?;
        let nodes = layout::flatten(view, &self.theme);
        let node = nodes.iter().find(|n| n.id == id)?;
        match (&node.el.kind, node.el.on_input.as_deref()) {
            (Kind::Input { value, multiline, .. }, Some(on_change)) => Some((value.clone(), on_change, *multiline)),
            _ => None,
        }
    }

    /// The selected text in the focused field, for Copy.
    pub fn selected_text(&mut self) -> Option<String> {
        self.ensure_view();
        let edit = self.edit?;
        let (value, ..) = self.input(edit.id)?;
        let (start, end) = (edit.cursor.min(edit.anchor).min(value.len()), edit.cursor.max(edit.anchor).min(value.len()));
        (start != end).then(|| value[start..end].to_owned())
    }

    /// Removes the selection in the focused field and returns it, for Cut.
    pub fn cut(&mut self) -> Option<String> {
        let text = self.selected_text()?;
        self.edit_value(|value, start, end| {
            value.replace_range(start..end, "");
            start
        });
        Some(text)
    }

    /// Replaces the selection in the focused text field with `text`, as
    /// typing, pasting or an input method does.
    pub fn insert_text(&mut self, text: &str) {
        self.ensure_view();
        let multiline = self.edit.and_then(|e| self.input(e.id)).is_some_and(|(_, _, multiline)| multiline);
        // Line breaks stay in a text area and become spaces in a single line;
        // other control characters go.
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let text: String = text.chars().map(|c| if c == '\n' && !multiline { ' ' } else { c }).filter(|c| *c == '\n' || !c.is_control()).collect();
        if text.is_empty() {
            return;
        }
        self.edit_value(|value, start, end| {
            value.replace_range(start..end, &text);
            start + text.len()
        });
    }

    /// Applies `change` to the focused field's value and selection range;
    /// `change` returns the new cursor. Sends the field's message if the
    /// value changed.
    fn edit_value(&mut self, change: impl FnOnce(&mut String, usize, usize) -> usize) {
        self.ensure_view();
        let Some(edit) = self.edit else { return };
        let Some((mut value, on_change, _)) = self.input(edit.id) else { return };
        let (start, end) = (edit.cursor.min(edit.anchor).min(value.len()), edit.cursor.max(edit.anchor).min(value.len()));
        let before = value.clone();
        let cursor = change(&mut value, start, end);
        let message = (value != before).then(|| on_change(value));
        self.edit = Some(Edit { cursor, anchor: cursor, ..edit });
        self.dirty = true;
        if let Some(message) = message {
            self.send(message);
        }
    }

    fn move_cursor(&mut self, key: Key) {
        self.ensure_view();
        let Some(edit) = self.edit else { return };
        let Some((value, _, multiline)) = self.input(edit.id) else { return };
        let len = value.len();
        let cursor = edit.cursor.min(len);
        let prev = value[..cursor].char_indices().next_back().map_or(0, |(i, _)| i);
        let next = value[cursor..].chars().next().map_or(cursor, |c| cursor + c.len_utf8());
        let line_start = value[..cursor].rfind('\n').map_or(0, |i| i + 1);
        let line_end = value[cursor..].find('\n').map_or(len, |i| cursor + i);
        let (start, end) = (edit.cursor.min(edit.anchor), edit.cursor.max(edit.anchor));
        let (cursor, anchor) = match key {
            Key::Left if start != end => (start, start),
            Key::Right if start != end => (end, end),
            Key::Left => (prev, prev),
            Key::Right => (next, next),
            Key::WordLeft if start != end => (start, start),
            Key::WordRight if start != end => (end, end),
            Key::WordLeft => (word_left(&value, cursor), word_left(&value, cursor)),
            Key::WordRight => (word_right(&value, cursor), word_right(&value, cursor)),
            Key::SelectWordLeft => (word_left(&value, cursor), edit.anchor),
            Key::SelectWordRight => (word_right(&value, cursor), edit.anchor),
            Key::Home => (line_start, line_start),
            Key::End => (line_end, line_end),
            Key::DocStart => (0, 0),
            Key::DocEnd => (len, len),
            Key::SelectLeft => (prev, edit.anchor),
            Key::SelectRight => (next, edit.anchor),
            Key::SelectHome => (line_start, edit.anchor),
            Key::SelectEnd => (line_end, edit.anchor),
            Key::SelectAll => (len, 0),
            Key::Up | Key::Down | Key::SelectUp | Key::SelectDown => {
                let down = matches!(key, Key::Down | Key::SelectDown);
                let target = if !multiline {
                    if down { len } else { 0 }
                } else {
                    let Some((style, wrap)) = self.field_text(edit.id) else { return };
                    let [x, y] = self.text.caret(&value, &style, wrap, cursor);
                    let line = style.size * style.line_height;
                    let total = self.text.measure(&value, &style, wrap)[1];
                    if down && y + line * 1.5 > total + 0.5 {
                        len
                    } else if down {
                        self.text.hit(&value, &style, wrap, [x, y + line * 1.5])
                    } else if y < line * 0.5 {
                        0
                    } else {
                        self.text.hit(&value, &style, wrap, [x, y - line * 0.5])
                    }
                };
                (target, if matches!(key, Key::SelectUp | Key::SelectDown) { edit.anchor } else { target })
            }
            _ => return,
        };
        self.edit = Some(Edit { cursor, anchor, ..edit });
        self.dirty = true;
    }

    /// The byte offset in the field `id` closest to the window x position.
    fn offset_at(&mut self, id: u64, p: [f32; 2]) -> usize {
        self.ensure_view();
        let Some((value, _, multiline)) = self.input(id) else { return 0 };
        let Some(rect) = self.targets.iter().find(|t| t.id == id).map(|t| t.rect) else { return value.len() };
        let view = self.view.as_ref().expect("view");
        let nodes = layout::flatten(view, &self.theme);
        let Some(node) = nodes.iter().find(|n| n.id == id) else { return value.len() };
        let (origin, wrap) = layout::field_geometry(rect, &node.el.style, &node.text, multiline, self.scene.caret_scroll(id));
        let style = node.text.clone();
        // A single line is one row, wherever in it the click lands.
        let y = if multiline { p[1] - origin[1] } else { style.size * style.line_height / 2.0 };
        self.text.hit(&value, &style, wrap, [p[0] - origin[0], y])
    }

    /// How the text of field `id` is set and wrapped.
    fn field_text(&mut self, id: u64) -> Option<(crate::text::TextStyle, Option<f32>)> {
        let rect = self.targets.iter().find(|t| t.id == id)?.rect;
        let view = self.view.as_ref()?;
        let nodes = layout::flatten(view, &self.theme);
        let node = nodes.iter().find(|n| n.id == id)?;
        let Kind::Input { multiline, .. } = &node.el.kind else { return None };
        let (_, wrap) = layout::field_geometry(rect, &node.el.style, &node.text, *multiline, [0.0; 2]);
        Some((node.text.clone(), wrap))
    }

    fn focus(&mut self, id: Option<u64>) {
        self.ensure_view();
        self.focused = id;
        self.edit = None;
        if let Some(id) = id
            && let Some((value, ..)) = self.input(id)
        {
            self.edit = Some(Edit { id, cursor: value.len(), anchor: 0 });
        }
        self.dirty = true;
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
            .max_by_key(|(i, t)| (t.overlay, t.on_top, *i))
            .map(|(_, t)| t)
    }

    /// Sends the message for slider `id` at position `value`.
    fn set_slider(&mut self, id: u64, value: f32) {
        self.ensure_view();
        let Some(view) = self.view.as_ref() else { return };
        let nodes = layout::flatten(view, &self.theme);
        let Some(on_drag) = nodes.iter().find(|n| n.id == id).and_then(|n| n.el.on_drag.as_deref()) else { return };
        let message = on_drag(value.clamp(0.0, 1.0));
        self.send(message);
    }

    /// Moves slider `id` to where the pointer is, at logical `x`.
    fn drag_to(&mut self, id: u64, x: f32) {
        let Some(rect) = self.targets.iter().find(|t| t.id == id).map(|t| t.rect) else { return };
        let track = (rect[2] - SLIDER_THUMB).max(1.0);
        self.set_slider(id, (x - rect[0] - SLIDER_THUMB / 2.0) / track);
    }

    /// Arrow keys, Home and End on a focused slider. False if none is focused.
    fn nudge_slider(&mut self, key: Key) -> bool {
        let Some(id) = self.focused else { return false };
        self.ensure_view();
        let Some(view) = self.view.as_ref() else { return false };
        let nodes = layout::flatten(view, &self.theme);
        let Some(value) = nodes.iter().find(|n| n.id == id).filter(|n| n.el.on_drag.is_some()).and_then(|n| n.el.value) else {
            return false;
        };
        let next = match key {
            Key::Left => value - SLIDER_STEP,
            Key::Right => value + SLIDER_STEP,
            Key::Home => 0.0,
            _ => 1.0,
        };
        self.set_slider(id, next);
        true
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
        self.update();
        self.pointer = Some(p);
        if let Some(id) = self.dragging {
            self.drag_to(id, p[0]);
        }
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
        self.update();
        let Some(p) = self.pointer else { return };
        let hit = self.hit(p).cloned();
        if let Some(t) = hit.as_ref().filter(|t| t.input) {
            // Clicking a text field focuses it and puts the cursor there.
            let id = t.id;
            if self.focused != Some(id) {
                self.focus(Some(id));
            }
            let offset = self.offset_at(id, p);
            self.edit = Some(Edit { id, cursor: offset, anchor: offset });
            self.dirty = true;
            return;
        }
        if let Some(id) = hit.as_ref().and_then(|t| t.drag) {
            self.dragging = Some(id);
            self.drag_to(id, p[0]);
            return;
        }
        self.pressed = hit.and_then(|t| t.press);
        if let Some(id) = self.pressed {
            self.animate(id, Some(1.0), None);
        }
        if self.focused.is_some() {
            self.focus(None);
        }
    }

    /// Releasing over the control that was pressed activates it.
    pub fn pointer_released(&mut self) {
        self.update();
        if self.dragging.take().is_some() {
            return;
        }
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
        self.update();
        let Some(p) = self.pointer else { return };
        let Some(id) = self.hit(p).and_then(|t| t.scroll) else { return };
        let max = self.scroll_info.get(&id).map_or(0.0, |s| s.max);
        let offset = self.scroll.entry(id).or_insert(0.0);
        let next = (*offset + delta).clamp(0.0, max);
        if next != *offset {
            *offset = next;
            self.layout_cache = None;
            self.dirty = true;
        }
    }

    pub fn key(&mut self, key: Key) {
        // Act on the UI as it is now, not as it was at the last frame.
        self.update();
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
                self.focus(Some(self.focus_order[next]));
            }
            Key::Activate if self.edit.is_some() => {}
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
                if let Some(message) = self.dismiss.and_then(|id| self.messages.get(&id).cloned()) {
                    self.focus(None);
                    self.send(message);
                } else if self.focused.is_some() {
                    self.focus(None);
                }
            }
            Key::Backspace => self.edit_value(|value, start, end| {
                if start != end {
                    value.replace_range(start..end, "");
                    start
                } else if let Some((prev, _)) = value[..start].char_indices().next_back() {
                    value.replace_range(prev..start, "");
                    prev
                } else {
                    start
                }
            }),
            Key::DeleteWordBack => self.edit_value(|value, start, end| {
                let from = if start != end { start } else { word_left(value, start) };
                value.replace_range(from..end, "");
                from
            }),
            Key::Delete => self.edit_value(|value, start, end| {
                if start != end {
                    value.replace_range(start..end, "");
                } else if let Some(c) = value[start..].chars().next() {
                    value.replace_range(start..start + c.len_utf8(), "");
                }
                start
            }),
            Key::Left | Key::Right | Key::Home | Key::End if self.nudge_slider(key) => {}
            other => self.move_cursor(other),
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

fn is_text_input(role: accesskit::Role) -> bool {
    matches!(role, accesskit::Role::TextInput | accesskit::Role::MultilineTextInput)
}

/// What a word is for Option-arrow movement: spaces, letters and digits, or punctuation.
fn char_class(c: char) -> u8 {
    if c.is_whitespace() {
        0
    } else if c.is_alphanumeric() || c == '_' {
        1
    } else {
        2
    }
}

/// The start of the word before byte `at`, skipping spaces first.
fn word_left(text: &str, at: usize) -> usize {
    let mut i = at.min(text.len());
    let before = |i: usize| text[..i].chars().next_back();
    while let Some(c) = before(i).filter(|&c| char_class(c) == 0) {
        i -= c.len_utf8();
    }
    if let Some(class) = before(i).map(char_class) {
        while let Some(c) = before(i).filter(|&c| char_class(c) == class) {
            i -= c.len_utf8();
        }
    }
    i
}

/// The end of the word after byte `at`, skipping spaces first.
fn word_right(text: &str, at: usize) -> usize {
    let mut i = at.min(text.len());
    let after = |i: usize| text[i..].chars().next();
    while let Some(c) = after(i).filter(|&c| char_class(c) == 0) {
        i += c.len_utf8();
    }
    if let Some(class) = after(i).map(char_class) {
        while let Some(c) = after(i).filter(|&c| char_class(c) == class) {
            i += c.len_utf8();
        }
    }
    i
}

fn access_node<M>(n: &layout::Node<'_, M>) -> AccessNode {
    use accesskit::Role as R;
    let el = n.el;
    let (role, label, value) = match &el.kind {
        Kind::Text(text) => (R::Label, Some(text.clone()), None),
        Kind::Image(_) => (R::Image, el.label.clone(), None),
        Kind::Input { value, placeholder, multiline } => {
            let label = el.label.clone().or_else(|| (!placeholder.is_empty()).then(|| placeholder.clone()));
            (if *multiline { R::MultilineTextInput } else { R::TextInput }, label, Some(value.clone()))
        }
        Kind::Div => {
            let role = match el.role {
                Role::Button => R::Button,
                Role::Toolbar => R::Toolbar,
                Role::Navigation => R::Navigation,
                Role::Heading => R::Heading,
                Role::Image => R::Image,
                Role::TextField => R::TextInput,
                Role::Menu => R::Menu,
                Role::MenuItem => R::MenuItem,
                Role::Switch => R::Switch,
                Role::Checkbox => R::CheckBox,
                Role::Tab => R::Tab,
                Role::TabList => R::TabList,
                Role::Slider => R::Slider,
                Role::Generic if el.style.scroll_y => R::ScrollView,
                Role::Generic => R::GenericContainer,
            };
            (role, el.label.clone(), None)
        }
    };
    AccessNode {
        id: n.id,
        role,
        label,
        value,
        rect: n.rect,
        children: Vec::new(),
        clickable: el.on_press.is_some(),
        focusable: el.on_press.is_some() || el.on_input.is_some() || el.on_drag.is_some(),
        toggled: matches!(el.role, Role::Switch | Role::Checkbox).then_some(el.checked).flatten(),
        selected: (el.role == Role::Tab).then_some(el.checked).flatten(),
        numeric: el.on_drag.as_ref().and(el.value),
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
    fn hover_reuses_the_layout_but_resizing_redoes_it() {
        let mut ui = ui();
        let before = ui.rect_of_key("add").expect("add");
        ui.pointer_moved(center(before));
        ui.update();
        assert_eq!(ui.rect_of_key("add"), Some(before), "hovering does not move anything");
        ui.resize([400.0, 200.0]);
        let after = ui.rect_of_key("add").expect("add");
        assert_eq!(after[2], before[2] + 200.0, "the stretched button follows the new width");
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

    struct Form {
        name: String,
        saved: u32,
    }

    #[derive(Clone)]
    enum FormMsg {
        Name(String),
        Save,
    }

    impl App for Form {
        type Message = FormMsg;
        fn update(&mut self, m: FormMsg) {
            match m {
                FormMsg::Name(n) => self.name = n,
                FormMsg::Save => self.saved += 1,
            }
        }
        fn view(&self, _: &Theme) -> Element<FormMsg> {
            column()
                .p_4()
                .gap_2()
                .child(crate::text_input(self.name.clone(), FormMsg::Name).key("name").placeholder("Name"))
                .child(div().key("save").h(30.0).on_press(FormMsg::Save))
        }
    }

    fn form(name: &str) -> Ui<Form> {
        Ui::new(Form { name: name.into(), saved: 0 }, Theme::default(), [300.0, 200.0])
    }

    #[test]
    fn typing_into_a_focused_field_updates_the_app() {
        let mut ui = form("");
        ui.update();
        ui.key(Key::Tab);
        assert!(ui.is_editing());
        ui.insert_text("Ada");
        ui.insert_text(" L");
        assert_eq!(ui.app().name, "Ada L");
        ui.key(Key::Backspace);
        ui.key(Key::Left);
        ui.key(Key::Left);
        ui.insert_text("!");
        assert_eq!(ui.app().name, "Ad!a ");
        ui.key(Key::End);
        ui.key(Key::Delete);
        assert_eq!(ui.app().name, "Ad!a ", "Delete at the end does nothing");
    }

    #[test]
    fn selections_are_replaced_by_typing_and_removed_by_backspace() {
        let mut ui = form("hello world");
        ui.update();
        ui.key(Key::Tab);
        // Focusing selects everything, so typing replaces it.
        ui.insert_text("bye");
        assert_eq!(ui.app().name, "bye");
        ui.key(Key::SelectHome);
        ui.key(Key::Backspace);
        assert_eq!(ui.app().name, "");
        ui.insert_text("añb");
        ui.key(Key::Left);
        ui.key(Key::SelectLeft);
        ui.key(Key::Backspace);
        assert_eq!(ui.app().name, "ab", "multi-byte characters move and delete whole");
    }

    #[test]
    fn clicking_in_a_field_places_the_cursor_and_space_types_instead_of_pressing() {
        let mut ui = form("abc");
        let field = ui.rect_of_key("name").expect("field");
        // Just inside the left padding: before the first character.
        ui.pointer_moved([field[0] + 9.0, field[1] + field[3] / 2.0]);
        ui.pointer_pressed();
        ui.pointer_released();
        ui.insert_text("x");
        assert_eq!(ui.app().name, "xabc");
        ui.key(Key::Activate);
        assert_eq!(ui.app().saved, 0, "Activate while editing must not press other controls");
        ui.key(Key::SelectAll);
        assert_eq!(ui.cut().as_deref(), Some("xabc"));
        assert_eq!(ui.app().name, "");
        ui.insert_text("one\ntwo");
        assert_eq!(ui.app().name, "one two", "pasted line breaks become spaces");
        ui.key(Key::SelectLeft);
        ui.key(Key::SelectLeft);
        assert_eq!(ui.selected_text().as_deref(), Some("wo"));
        // Clicking elsewhere ends editing.
        ui.pointer_moved([field[0] + field[2] + 40.0, field[1] + 1.0]);
        ui.pointer_pressed();
        ui.pointer_released();
        assert!(!ui.is_editing());
    }

    #[test]
    fn the_caret_and_placeholder_are_drawn() {
        let mut ui = form("");
        let texts = |scene: &Scene| -> Vec<String> {
            scene.content.iter().filter_map(|p| if let crate::scene::Prim::Text { text, .. } = p { Some(text.clone()) } else { None }).collect()
        };
        assert_eq!(texts(ui.scene()), ["Name"]);
        assert!(ui.scene().caret.is_none());
        ui.key(Key::Tab);
        let caret = ui.scene().caret.clone().expect("caret while editing");
        assert_eq!((caret.pos[0], caret.selection.len()), (0.0, 0));
        ui.insert_text("Hi");
        let caret = ui.scene().caret.clone().expect("caret");
        assert!(caret.pos[0] > 8.0, "caret should move past the text, at {}", caret.pos[0]);
        assert_eq!(texts(ui.scene()), ["Hi"]);
    }

    #[test]
    fn the_accessibility_tree_names_controls_and_follows_focus() {
        use accesskit::{Action, NodeId, Role as R};
        let mut ui = Ui::new(
            Library::default(),
            Theme::default(),
            [400.0, 300.0],
        );
        let tree = ui.accessibility(2.0);
        let find = |tree: &accesskit::TreeUpdate, role: R| tree.nodes.iter().find(|(_, n)| n.role() == role).map(|(id, n)| (*id, n.clone()));
        let (_, window) = find(&tree, R::Window).expect("window node");
        assert_eq!(window.children().len(), 1);
        let (button_id, button) = find(&tree, R::Button).expect("button");
        assert_eq!(button.label(), Some("Save"));
        assert!(button.supports_action(Action::Click) && button.supports_action(Action::Focus));
        let bounds = button.bounds().expect("bounds");
        assert!(bounds.x1 > bounds.x0 && bounds.y1 > bounds.y0, "bounds {bounds:?}");
        let (_, field) = find(&tree, R::TextInput).expect("text field");
        assert_eq!((field.label(), field.value()), (Some("Title"), Some("Draft")));
        assert_eq!(tree.focus, NodeId(WINDOW_NODE));
        ui.key(Key::Tab);
        assert_eq!(ui.accessibility(2.0).focus, button_id);
    }

    #[test]
    fn screen_reader_actions_press_buttons_and_set_text() {
        use accesskit::{Action, ActionData, ActionRequest, Role as R, TreeId};
        let mut ui = Ui::new(Library::default(), Theme::default(), [400.0, 300.0]);
        let tree = ui.accessibility(1.0);
        let id_of = |role: R| tree.nodes.iter().find(|(_, n)| n.role() == role).map(|(id, _)| *id).expect("node");
        let request = |action, target_node, data| ActionRequest { action, target_tree: TreeId::ROOT, target_node, data };
        ui.accessibility_action(&request(Action::Click, id_of(R::Button), None));
        assert_eq!(ui.app().saves, 1);
        ui.accessibility_action(&request(Action::SetValue, id_of(R::TextInput), Some(ActionData::Value("Final".into()))));
        assert_eq!(ui.app().title, "Final");
        // A Click on a node that cannot be clicked does nothing.
        ui.accessibility_action(&request(Action::Click, id_of(R::TextInput), None));
        assert_eq!(ui.app().saves, 1);
    }

    struct Library {
        title: String,
        saves: u32,
    }

    impl Default for Library {
        fn default() -> Self {
            Self { title: "Draft".into(), saves: 0 }
        }
    }

    #[derive(Clone)]
    enum LibraryMsg {
        Title(String),
        Save,
    }

    impl App for Library {
        type Message = LibraryMsg;
        fn update(&mut self, m: LibraryMsg) {
            match m {
                LibraryMsg::Title(t) => self.title = t,
                LibraryMsg::Save => self.saves += 1,
            }
        }
        fn view(&self, _: &Theme) -> Element<LibraryMsg> {
            column()
                .p_4()
                .gap_2()
                .child(crate::button("Save").on_press(LibraryMsg::Save))
                .child(crate::text_input(self.title.clone(), LibraryMsg::Title).placeholder("Title"))
        }
    }

    struct Menus {
        open: bool,
        picked: Vec<&'static str>,
        under: u32,
    }

    #[derive(Clone)]
    enum MenuMsg {
        Open,
        Close,
        Pick(&'static str),
        Under,
    }

    impl App for Menus {
        type Message = MenuMsg;
        fn update(&mut self, m: MenuMsg) {
            match m {
                MenuMsg::Open => self.open = true,
                MenuMsg::Close => self.open = false,
                MenuMsg::Pick(p) => {
                    self.picked.push(p);
                    self.open = false;
                }
                MenuMsg::Under => self.under += 1,
            }
        }
        fn view(&self, _: &Theme) -> Element<MenuMsg> {
            let mut root = column()
                .p_4()
                .gap_2()
                .child(div().key("open").h(30.0).on_press(MenuMsg::Open))
                .child(div().key("under").h(60.0).on_press(MenuMsg::Under));
            if self.open {
                let panel = crate::menu()
                    .key("panel")
                    .absolute()
                    .top(50.0)
                    .left(16.0)
                    .child(crate::menu_item("Copy", MenuMsg::Pick("copy")).key("copy"))
                    .child(crate::menu_item("Paste", MenuMsg::Pick("paste")).key("paste"));
                root = root.child(crate::popover(MenuMsg::Close, panel));
            }
            root
        }
    }

    fn menus() -> Ui<Menus> {
        let mut ui = Ui::new(Menus { open: false, picked: vec![], under: 0 }, Theme::default(), [300.0, 200.0]);
        let open = ui.rect_of_key("open").expect("open");
        let p = center(open);
        ui.pointer_moved(p);
        ui.pointer_pressed();
        ui.pointer_released();
        assert!(ui.app().open);
        ui
    }

    fn click_at<A: App>(ui: &mut Ui<A>, p: [f32; 2]) {
        ui.pointer_moved(p);
        ui.pointer_pressed();
        ui.pointer_released();
    }

    #[test]
    fn menu_items_take_clicks_before_controls_under_them() {
        let mut ui = menus();
        assert!(ui.scene().overlay.as_ref().is_some_and(|o| o.glass.len() == 1), "the menu is overlay glass");
        let copy = ui.rect_of_key("copy").expect("copy item");
        let under = ui.rect_of_key("under").expect("control under the menu");
        assert!(crate::layout::contains(under, center(copy)), "the item should sit over the other control");
        click_at(&mut ui, center(copy));
        assert_eq!(ui.app().picked, ["copy"]);
        assert_eq!(ui.app().under, 0);
        assert!(!ui.app().open);
        assert!(ui.scene().overlay.is_none());
    }

    #[test]
    fn popovers_close_on_outside_clicks_and_escape_but_not_on_the_panel() {
        let mut ui = menus();
        let panel = ui.rect_of_key("panel").expect("panel");
        // The panel's padding, between items: stays open.
        click_at(&mut ui, [panel[0] + 2.0, panel[1] + 2.0]);
        assert!(ui.app().open, "clicking the panel itself must not dismiss");
        click_at(&mut ui, [290.0, 190.0]);
        assert!(!ui.app().open, "clicking outside dismisses");
        assert_eq!(ui.app().under, 0, "the outside click does not reach the controls");
        let mut ui = menus();
        ui.key(Key::Escape);
        assert!(!ui.app().open, "Escape dismisses");
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

    struct Settings {
        wifi: bool,
        agree: bool,
        volume: f32,
        tab: usize,
    }

    #[derive(Clone)]
    enum SettingsMsg {
        Wifi,
        Agree,
        Volume(f32),
        Tab(usize),
    }

    impl App for Settings {
        type Message = SettingsMsg;
        fn update(&mut self, m: SettingsMsg) {
            match m {
                SettingsMsg::Wifi => self.wifi = !self.wifi,
                SettingsMsg::Agree => self.agree = !self.agree,
                SettingsMsg::Volume(v) => self.volume = v,
                SettingsMsg::Tab(i) => self.tab = i,
            }
        }
        fn view(&self, theme: &Theme) -> Element<SettingsMsg> {
            use crate::widgets::{checkbox, segmented, slider, switch};
            column()
                .p_4()
                .gap_2()
                .child(switch(theme, self.wifi, SettingsMsg::Wifi).key("wifi").label("Wi-Fi"))
                .child(checkbox(theme, "Agree", self.agree, SettingsMsg::Agree).key("agree"))
                .child(slider(theme, self.volume, SettingsMsg::Volume).w(228.0).key("volume").label("Volume"))
                .child(segmented(theme, &["A", "B", "C"], self.tab, SettingsMsg::Tab).key("tabs"))
        }
    }

    fn settings() -> Ui<Settings> {
        Ui::new(Settings { wifi: false, agree: false, volume: 0.5, tab: 0 }, Theme::default(), [300.0, 300.0])
    }

    fn tap(ui: &mut Ui<Settings>, at: [f32; 2]) {
        ui.pointer_moved(at);
        ui.pointer_pressed();
        ui.pointer_released();
    }

    fn middle(ui: &mut Ui<Settings>, key: &str) -> [f32; 2] {
        let r = ui.rect_of_key(key).expect(key);
        [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
    }

    #[test]
    fn pressing_a_switch_or_checkbox_flips_it() {
        let mut ui = settings();
        let at = middle(&mut ui, "wifi");
        tap(&mut ui, at);
        assert!(ui.app().wifi);
        let at = middle(&mut ui, "agree");
        tap(&mut ui, at);
        assert!(ui.app().agree);
        tap(&mut ui, at);
        assert!(!ui.app().agree);
    }

    #[test]
    fn pressing_a_slider_sets_its_value_from_the_pointer_and_dragging_follows() {
        let mut ui = settings();
        let r = ui.rect_of_key("volume").expect("slider");
        // The thumb's center spans the width minus the thumb, so the left
        // and right ends of that span are 0 and 1.
        let (left, right) = (r[0] + SLIDER_THUMB / 2.0, r[0] + r[2] - SLIDER_THUMB / 2.0);
        let y = r[1] + r[3] / 2.0;
        ui.pointer_moved([left + (right - left) * 0.25, y]);
        ui.pointer_pressed();
        assert!((ui.app().volume - 0.25).abs() < 0.001, "pressed at a quarter: {}", ui.app().volume);
        ui.pointer_moved([left + (right - left) * 0.8, y]);
        assert!((ui.app().volume - 0.8).abs() < 0.001, "dragged to 0.8: {}", ui.app().volume);
        // Dragging far past either end, and off the slider vertically, clamps.
        ui.pointer_moved([right + 500.0, y + 200.0]);
        assert_eq!(ui.app().volume, 1.0);
        ui.pointer_moved([left - 500.0, y]);
        assert_eq!(ui.app().volume, 0.0);
        ui.pointer_released();
        // After release, moving does nothing.
        ui.pointer_moved([right, y]);
        assert_eq!(ui.app().volume, 0.0);
    }

    #[test]
    fn pressing_the_glass_thumb_drags_the_slider_too() {
        let mut ui = settings();
        let r = ui.rect_of_key("volume").expect("slider");
        // At 0.5 the thumb is in the middle of the slider.
        ui.pointer_moved([r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]);
        ui.pointer_pressed();
        ui.pointer_moved([r[0] + r[2] / 2.0 + 40.0, r[1] + r[3] / 2.0]);
        assert!(ui.app().volume > 0.5, "dragging from the thumb moves the value: {}", ui.app().volume);
        ui.pointer_released();
    }

    #[test]
    fn a_focused_slider_moves_with_the_arrow_keys() {
        let mut ui = settings();
        ui.update();
        // Switch, checkbox, slider, then the three tabs.
        for _ in 0..3 {
            ui.key(Key::Tab);
        }
        ui.key(Key::Right);
        assert!((ui.app().volume - 0.55).abs() < 0.001, "{}", ui.app().volume);
        ui.key(Key::Left);
        ui.key(Key::Left);
        assert!((ui.app().volume - 0.45).abs() < 0.001, "{}", ui.app().volume);
        ui.key(Key::End);
        assert_eq!(ui.app().volume, 1.0);
        ui.key(Key::Right);
        assert_eq!(ui.app().volume, 1.0, "stays at the end");
        ui.key(Key::Home);
        assert_eq!(ui.app().volume, 0.0);
    }

    #[test]
    fn space_presses_the_focused_switch_and_tab_reaches_the_segments() {
        let mut ui = settings();
        ui.update();
        ui.key(Key::Tab);
        ui.key(Key::Activate);
        assert!(ui.app().wifi);
        for _ in 0..4 {
            ui.key(Key::Tab);
        }
        // Past the checkbox and slider, now on the second segment.
        ui.key(Key::Activate);
        assert_eq!(ui.app().tab, 1);
    }

    #[test]
    fn switches_sliders_and_tabs_tell_screen_readers_their_state() {
        use accesskit::{Action, ActionData, ActionRequest, Role as R, Toggled, TreeId};
        let mut ui = settings();
        let tree = ui.accessibility(1.0);
        let find = |role: R| tree.nodes.iter().filter(|(_, n)| n.role() == role).collect::<Vec<_>>();
        let switch = find(R::Switch);
        assert_eq!(switch.len(), 1);
        assert_eq!(switch[0].1.toggled(), Some(Toggled::False));
        assert_eq!(switch[0].1.label(), Some("Wi-Fi"));
        assert_eq!(find(R::CheckBox)[0].1.toggled(), Some(Toggled::False));
        let tabs = find(R::Tab);
        assert_eq!(tabs.iter().map(|(_, n)| n.is_selected()).collect::<Vec<_>>(), [Some(true), Some(false), Some(false)]);
        assert_eq!(find(R::TabList).len(), 1);
        let slider = find(R::Slider);
        assert_eq!(slider[0].1.numeric_value(), Some(0.5));
        let id = slider[0].0;
        let request = |action, data| ActionRequest { action, target_tree: TreeId::ROOT, target_node: id, data };
        ui.accessibility_action(&request(Action::Increment, None));
        assert!((ui.app().volume - 0.55).abs() < 0.001);
        ui.accessibility_action(&request(Action::SetValue, Some(ActionData::NumericValue(0.9))));
        assert!((ui.app().volume - 0.9).abs() < 0.001);
        ui.accessibility_action(&request(Action::Decrement, None));
        assert!((ui.app().volume - 0.85).abs() < 0.001);
        // Pressing the second tab selects it, and the tree follows.
        let second = tabs[1].0;
        ui.accessibility_action(&ActionRequest { action: Action::Click, target_tree: TreeId::ROOT, target_node: second, data: None });
        assert_eq!(ui.app().tab, 1);
        let tree = ui.accessibility(1.0);
        let selected: Vec<_> = tree.nodes.iter().filter(|(_, n)| n.role() == R::Tab).map(|(_, n)| n.is_selected()).collect();
        assert_eq!(selected, [Some(false), Some(true), Some(false)]);
    }

    #[test]
    fn a_slider_given_nan_still_builds() {
        struct Bad;
        impl App for Bad {
            type Message = f32;
            fn update(&mut self, _: f32) {}
            fn view(&self, theme: &Theme) -> Element<f32> {
                crate::widgets::slider(theme, f32::NAN, |v| v)
            }
        }
        let mut ui = Ui::new(Bad, Theme::default(), [300.0, 100.0]);
        assert!(!ui.scene().glass.is_empty(), "the thumb is still drawn");
    }

    struct Notes {
        text: String,
    }

    impl App for Notes {
        type Message = String;
        fn update(&mut self, m: String) {
            self.text = m;
        }
        fn view(&self, _: &Theme) -> Element<String> {
            column().p_4().child(crate::text_area(self.text.clone(), |t| t).key("notes").w(200.0).h(96.0))
        }
    }

    /// A focused text area holding `text`, with the caret at the end.
    fn notes(text: &str) -> Ui<Notes> {
        let mut ui = Ui::new(Notes { text: text.into() }, Theme::default(), [300.0, 200.0]);
        ui.update();
        ui.key(Key::Tab);
        ui.key(Key::DocEnd);
        ui
    }

    /// Types `marker` at the caret and returns the text with it, to see where the caret is.
    fn caret_in(ui: &mut Ui<Notes>, marker: &str) -> String {
        ui.insert_text(marker);
        let text = ui.app().text.clone();
        ui.key(Key::Backspace);
        text
    }

    #[test]
    fn return_starts_a_new_line_in_a_text_area_but_not_in_a_single_line_field() {
        let mut ui = notes("ab");
        ui.insert_text("\ncd\r\nef\rgh");
        assert_eq!(ui.app().text, "ab\ncd\nef\ngh");
        let mut ui = form("");
        ui.update();
        ui.key(Key::Tab);
        ui.insert_text("a\nb");
        assert_eq!(ui.app().name, "a b");
    }

    #[test]
    fn up_and_down_move_between_lines_keeping_the_column() {
        let mut ui = notes("abcd\nef\nghijkl");
        // The caret is at the end of "ghijkl", column 6. Up lands on the end of the shorter "ef".
        ui.key(Key::Up);
        assert_eq!(caret_in(&mut ui, "|"), "abcd\nef|\nghijkl");
        // Up again: column 2 of "abcd", roughly under where it was.
        ui.key(Key::Up);
        assert_eq!(caret_in(&mut ui, "|"), "ab|cd\nef\nghijkl");
        // Up on the first line goes to the start, Down on the last goes to the end.
        ui.key(Key::Up);
        assert_eq!(caret_in(&mut ui, "|"), "|abcd\nef\nghijkl");
        ui.key(Key::Down);
        assert_eq!(caret_in(&mut ui, "|"), "abcd\n|ef\nghijkl");
        ui.key(Key::Down);
        assert_eq!(caret_in(&mut ui, "|"), "abcd\nef\n|ghijkl");
        ui.key(Key::Down);
        assert_eq!(caret_in(&mut ui, "|"), "abcd\nef\nghijkl|");
    }

    #[test]
    fn down_keeps_the_column_too() {
        // Identical lines, so the same column is the same x in a proportional font.
        let mut ui = notes("abcd\nabcd\nabcd");
        ui.key(Key::DocStart);
        ui.key(Key::Right);
        ui.key(Key::Right);
        ui.key(Key::Down);
        assert_eq!(caret_in(&mut ui, "|"), "abcd\nab|cd\nabcd");
        ui.key(Key::Down);
        assert_eq!(caret_in(&mut ui, "|"), "abcd\nabcd\nab|cd");
    }

    #[test]
    fn selecting_with_up_and_down_covers_the_lines_between() {
        let mut ui = notes("one\ntwo\nthree");
        ui.key(Key::SelectUp);
        ui.key(Key::SelectUp);
        assert_eq!(ui.scene().caret.as_ref().expect("caret").selection.len(), 3, "a selection over three lines has three rectangles");
        // The caret went up to the end of "one" (column 3 of "three" clamps there); typing replaces the rest.
        ui.insert_text("X");
        assert_eq!(ui.app().text, "oneX");
    }

    #[test]
    fn home_and_end_go_to_the_ends_of_the_line_and_command_up_down_to_the_ends_of_the_text() {
        let mut ui = notes("abc\ndef\nghi");
        ui.key(Key::Up);
        ui.key(Key::End);
        assert_eq!(caret_in(&mut ui, "|"), "abc\ndef|\nghi");
        ui.key(Key::Home);
        assert_eq!(caret_in(&mut ui, "|"), "abc\n|def\nghi");
        ui.key(Key::DocStart);
        assert_eq!(caret_in(&mut ui, "|"), "|abc\ndef\nghi");
        ui.key(Key::DocEnd);
        assert_eq!(caret_in(&mut ui, "|"), "abc\ndef\nghi|");
    }

    #[test]
    fn option_arrows_move_by_word_and_option_backspace_deletes_one() {
        let mut ui = notes("hello big-world  again");
        ui.key(Key::WordLeft);
        assert_eq!(caret_in(&mut ui, "|"), "hello big-world  |again");
        ui.key(Key::WordLeft);
        assert_eq!(caret_in(&mut ui, "|"), "hello big-|world  again", "punctuation is a word of its own: spaces then the word");
        ui.key(Key::WordRight);
        assert_eq!(caret_in(&mut ui, "|"), "hello big-world|  again");
        ui.key(Key::SelectWordLeft);
        ui.insert_text("W");
        assert_eq!(ui.app().text, "hello big-W  again");
        ui.key(Key::DeleteWordBack);
        assert_eq!(ui.app().text, "hello big-  again");
        // Punctuation is a word of its own, so this removes only the hyphen.
        ui.key(Key::DeleteWordBack);
        assert_eq!(ui.app().text, "hello big  again");
    }

    #[test]
    fn clicking_a_line_of_a_text_area_puts_the_caret_on_that_line() {
        let mut ui = notes("abc\ndef\nghi");
        let field = ui.rect_of_key("notes").expect("field");
        let line = ui.scene().caret.as_ref().expect("caret").pos[1] / 2.0;
        // Just inside the left padding, at the middle of the second line.
        ui.pointer_moved([field[0] + 9.0, field[1] + 6.0 + line * 1.5]);
        ui.pointer_pressed();
        assert_eq!(caret_in(&mut ui, "|"), "abc\n|def\nghi");
        // Far right on the first line: its end.
        ui.pointer_moved([field[0] + 190.0, field[1] + 6.0 + line * 0.5]);
        ui.pointer_pressed();
        assert_eq!(caret_in(&mut ui, "|"), "abc|\ndef\nghi");
    }

    #[test]
    fn long_lines_wrap_and_the_view_scrolls_to_keep_the_caret_visible() {
        let long = "word ".repeat(60);
        let mut ui = notes(&long);
        let caret = ui.scene().caret.clone().expect("caret");
        assert!(caret.pos[1] > 96.0, "the end of 60 words in a 200 px field is below the 96 px field: {:?}", caret.pos);
        assert!(caret.scroll[1] > 0.0 && caret.scroll[0] == 0.0, "scrolled down, not sideways: {:?}", caret.scroll);
        ui.key(Key::DocStart);
        assert_eq!(ui.scene().caret.as_ref().expect("caret").scroll, [0.0, 0.0]);
    }

    #[test]
    fn a_text_area_is_a_multiline_text_input_for_screen_readers() {
        use accesskit::Role as R;
        let mut ui = notes("a\nb");
        let tree = ui.accessibility(1.0);
        let node = tree.nodes.iter().find(|(_, n)| n.role() == R::MultilineTextInput).expect("a multiline input");
        assert_eq!(node.1.value(), Some("a\nb"));
        assert!(tree.nodes.iter().all(|(_, n)| n.role() != R::TextInput));
    }
}
