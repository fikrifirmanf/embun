use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use embun_render::GlassShape;

use crate::element::{Image, Kind};
use crate::layout::{Node, Rect4};
use crate::text::TextStyle;
use crate::{Color, Theme};

/// Something drawn on the content layer or on top of the glass.
#[derive(Clone, Debug)]
pub(crate) enum Prim {
    Rect { rect: Rect4, radius: f32, top: Color, bottom: Color, border: Option<(f32, Color)>, clip: Rect4 },
    Text { text: String, style: TextStyle, width: f32, origin: [f32; 2], color: Color, clip: Rect4 },
    Image { image: Image, rect: Rect4, radius: f32, clip: Rect4 },
}

/// What one frame draws: content behind the glass, the glass shapes, and
/// what sits on the glass.
#[derive(Clone, Debug, Default)]
pub(crate) struct Scene {
    pub background: Option<Color>,
    pub content: Vec<Prim>,
    pub glass: Vec<GlassShape>,
    pub foreground: Vec<Prim>,
    pub caret: Option<Caret>,
}

impl Scene {
    /// How far the text of field `id` is shifted left to keep its caret visible.
    pub(crate) fn caret_scroll(&self, id: u64) -> f32 {
        self.caret.filter(|c| c.id == id).map_or(0.0, |c| c.scroll)
    }
}

/// The caret and selection of the text field being edited, in logical
/// pixels from the start of its text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Caret {
    pub id: u64,
    pub x: f32,
    pub selection: Option<(f32, f32)>,
    pub scroll: f32,
}

/// Press and hover progress of a glass control, each 0 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Highlight {
    pub press: f32,
    pub hover: f32,
}

/// How much a pressed glass control grows, in logical pixels per side, and
/// how much white a hovered or pressed one mixes in.
const PRESS_GROW: f32 = 3.0;
const HOVER_BRIGHTEN: f32 = 0.06;
const PRESS_BRIGHTEN: f32 = 0.14;
/// Width of the keyboard focus ring and its gap from the control.
const FOCUS_RING: f32 = 3.0;

pub(crate) fn build<M>(
    nodes: &[Node<'_, M>],
    theme: &Theme,
    highlight: &HashMap<u64, Highlight>,
    focused: Option<u64>,
    caret: Option<&Caret>,
) -> Scene {
    let mut scene = Scene { background: Some(theme.background()), caret: caret.copied(), ..Default::default() };
    let mut next_group = 1u32 << 16;
    // Borders draw after the element's children, so a child that fills the
    // element (an image in a selected tile) cannot cover them. `end[i]` is
    // the last node of i's subtree; `borders` is a stack of (end, on glass, prim).
    let mut end: Vec<usize> = (0..nodes.len()).collect();
    for i in (0..nodes.len()).rev() {
        if let Some(&last) = nodes[i].children.last() {
            end[i] = end[last];
        }
    }
    let mut borders: Vec<(usize, bool, Prim)> = Vec::new();
    for (i, node) in nodes.iter().enumerate() {
        let s = &node.el.style;
        let layer = if node.on_glass.is_some() { &mut scene.foreground } else { &mut scene.content };
        if let Some(glass) = s.glass {
            let h = highlight.get(&node.id).copied().unwrap_or_default();
            let [x, y, w, hgt] = node.rect;
            let grow = if glass.interactive { PRESS_GROW * h.press } else { 0.0 };
            let mut shape = GlassShape::new([x + w / 2.0, y + hgt / 2.0], [w / 2.0 + grow, hgt / 2.0 + grow], s.radius + grow)
                .appearance(theme.glass_appearance());
            let brighten = if glass.interactive { HOVER_BRIGHTEN * h.hover + PRESS_BRIGHTEN * h.press } else { 0.0 };
            if let Some(tint) = glass.tint.or(glass.accent.then(|| theme.accent())) {
                // Pressing a tinted control lightens its tint a little.
                let lift = |c: f32| c + (1.0 - c) * brighten;
                shape = shape.tint([lift(tint.r), lift(tint.g), lift(tint.b)], tint.a);
            } else if brighten > 0.0 {
                shape = shape.tint([1.0; 3], brighten);
            }
            shape.group = glass.group.unwrap_or_else(|| {
                next_group += 1;
                next_group
            });
            scene.glass.push(shape);
        } else {
            // Pressable items on glass (toolbar items, sidebar rows) show a
            // soft fill while hovered or pressed.
            if node.on_glass.is_some() && node.el.on_press.is_some() {
                let h = highlight.get(&node.id).copied().unwrap_or_default();
                let alpha = 0.07 * h.hover + 0.09 * h.press;
                if alpha > 0.001 {
                    let fill = theme.label().with_alpha(alpha);
                    layer.push(Prim::Rect { rect: node.rect, radius: s.radius, top: fill, bottom: fill, border: None, clip: node.clip });
                }
            }
            if let Some(top) = s.background.or(s.gradient.map(|g| g.0)) {
                let bottom = s.gradient.map_or(top, |g| g.1);
                layer.push(Prim::Rect { rect: node.rect, radius: s.radius, top, bottom, border: None, clip: node.clip });
            }
            if let Some(border) = s.border {
                let clear = Color::TRANSPARENT;
                let ring = Prim::Rect { rect: node.rect, radius: s.radius, top: clear, bottom: clear, border: Some(border), clip: node.clip };
                borders.push((end[i], node.on_glass.is_some(), ring));
            }
        }
        match &node.el.kind {
            Kind::Text(text) => layer.push(Prim::Text {
                text: text.clone(),
                style: node.text.clone(),
                // A little slack so the text does not rewrap at the width it measured.
                width: node.rect[2] + 0.5,
                origin: [node.rect[0], node.rect[1]],
                color: node.color,
                clip: node.clip,
            }),
            Kind::Image(image) => layer.push(Prim::Image { image: image.clone(), rect: node.rect, radius: s.radius, clip: node.clip }),
            Kind::Input { value, placeholder } => {
                let [x, y, w, h] = node.rect;
                let fill = if node.on_glass.is_some() { theme.label().with_alpha(0.06) } else { theme.surface() };
                layer.push(Prim::Rect {
                    rect: node.rect,
                    radius: s.radius,
                    top: fill,
                    bottom: fill,
                    border: Some((1.0, theme.separator())),
                    clip: node.clip,
                });
                let line = node.text.size * node.text.line_height;
                let inner = [x + s.padding[3], y, w - s.padding[1] - s.padding[3], h];
                let clip = crate::layout::intersect(node.clip, inner);
                let caret = caret.filter(|c| c.id == node.id);
                let left = inner[0] - caret.map_or(0.0, |c| c.scroll);
                let top = y + (h - line) / 2.0;
                if let Some((a, b)) = caret.and_then(|c| c.selection) {
                    let sel = theme.accent().with_alpha(0.3);
                    layer.push(Prim::Rect { rect: [left + a, top, b - a, line], radius: 0.0, top: sel, bottom: sel, border: None, clip });
                }
                let (shown, color) = if value.is_empty() { (placeholder, theme.secondary_label()) } else { (value, node.color) };
                if !shown.is_empty() {
                    layer.push(Prim::Text {
                        text: shown.clone(),
                        style: node.text.clone(),
                        width: f32::INFINITY,
                        origin: [left, top],
                        color,
                        clip,
                    });
                }
                if let Some(c) = caret.filter(|c| c.selection.is_none()) {
                    let ink = theme.accent();
                    layer.push(Prim::Rect { rect: [left + c.x, top + 1.0, 1.5, line - 2.0], radius: 0.0, top: ink, bottom: ink, border: None, clip });
                }
            }
            Kind::Div => {}
        }
        if focused == Some(node.id) {
            let [x, y, w, h] = node.rect;
            let out = FOCUS_RING + 1.0;
            let clear = Color::TRANSPARENT;
            scene.foreground.push(Prim::Rect {
                rect: [x - out, y - out, w + 2.0 * out, h + 2.0 * out],
                radius: s.radius + out,
                top: clear,
                bottom: clear,
                border: Some((FOCUS_RING, theme.accent().with_alpha(0.75))),
                clip: [f32::MIN / 4.0, f32::MIN / 4.0, f32::MAX / 2.0, f32::MAX / 2.0],
            });
        }
        while borders.last().is_some_and(|(e, ..)| *e == i) {
            let (_, on_glass, ring) = borders.pop().expect("border");
            if on_glass { scene.foreground.push(ring) } else { scene.content.push(ring) }
        }
    }
    scene
}

/// A hash of the content layer, so the backdrop is redrawn (and re-blurred)
/// only when it changes.
pub(crate) fn content_hash(scene: &Scene) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let f = |h: &mut std::collections::hash_map::DefaultHasher, v: &[f32]| v.iter().for_each(|x| x.to_bits().hash(h));
    let color = |h: &mut std::collections::hash_map::DefaultHasher, c: Color| f(h, &[c.r, c.g, c.b, c.a]);
    if let Some(bg) = scene.background {
        color(&mut h, bg);
    }
    for p in &scene.content {
        match p {
            Prim::Rect { rect, radius, top, bottom, border, clip } => {
                0u8.hash(&mut h);
                f(&mut h, rect);
                f(&mut h, &[*radius]);
                color(&mut h, *top);
                color(&mut h, *bottom);
                if let Some((w, c)) = border {
                    f(&mut h, &[*w]);
                    color(&mut h, *c);
                }
                f(&mut h, clip);
            }
            Prim::Text { text, style, width, origin, color: c, clip } => {
                1u8.hash(&mut h);
                text.hash(&mut h);
                f(&mut h, &[style.size, style.line_height, *width]);
                style.weight.hash(&mut h);
                (style.align as u8).hash(&mut h);
                f(&mut h, origin);
                color(&mut h, *c);
                f(&mut h, clip);
            }
            Prim::Image { image, rect, radius, clip } => {
                2u8.hash(&mut h);
                image.id.hash(&mut h);
                f(&mut h, rect);
                f(&mut h, &[*radius]);
                f(&mut h, clip);
            }
        }
    }
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{Element, column, div, text};
    use crate::layout::{flatten, layout};
    use crate::text::TextSystem;

    fn scene_of(root: &Element<()>, highlight: &HashMap<u64, Highlight>) -> (Scene, Vec<u64>) {
        let theme = Theme::default();
        let mut nodes = flatten(root, &theme);
        layout(&mut nodes, [400.0, 300.0], &mut TextSystem::new(), &HashMap::new());
        (build(&nodes, &theme, highlight, None, None), nodes.iter().map(|n| n.id).collect())
    }

    #[test]
    fn children_of_glass_go_on_top_and_the_rest_behind() {
        let root = column()
            .bg(Color::rgb(10, 20, 30))
            .child(text("behind"))
            .child(div().glass().rounded_full().p_2().child(text("on glass")));
        let (scene, _) = scene_of(&root, &HashMap::new());
        assert_eq!(scene.glass.len(), 1);
        let texts = |prims: &[Prim]| prims.iter().filter_map(|p| if let Prim::Text { text, .. } = p { Some(text.clone()) } else { None }).collect::<Vec<_>>();
        assert_eq!(texts(&scene.content), ["behind"]);
        assert_eq!(texts(&scene.foreground), ["on glass"]);
    }

    #[test]
    fn separate_glass_elements_get_separate_merge_groups() {
        let root = column().child(div().glass().h(20.0)).child(div().glass().h(20.0)).child(div().glass().glass_group(7).h(20.0)).child(div().glass().glass_group(7).h(20.0));
        let (scene, _) = scene_of(&root, &HashMap::new());
        let groups: Vec<u32> = scene.glass.iter().map(|g| g.group).collect();
        assert_ne!(groups[0], groups[1]);
        assert_eq!((groups[2], groups[3]), (7, 7));
    }

    #[test]
    fn pressed_interactive_glass_grows_and_brightens() {
        let root = column().child(div().glass_interactive().w(100.0).h(30.0));
        let (rest, ids) = scene_of(&root, &HashMap::new());
        let pressed = HashMap::from([(ids[1], Highlight { press: 1.0, hover: 1.0 })]);
        let (down, _) = scene_of(&root, &pressed);
        assert_eq!(down.glass[0].half_size[0], rest.glass[0].half_size[0] + PRESS_GROW);
        assert_eq!(rest.glass[0].tint[3], 0.0);
        assert!(down.glass[0].tint[3] > 0.15, "tint {:?}", down.glass[0].tint);
    }

    #[test]
    fn borders_draw_over_the_children_that_fill_them() {
        let root = column().child(div().w(100.0).h(80.0).border(3.0, Color::WHITE).child(div().w(100.0).h(80.0).bg(Color::BLACK)));
        let (scene, _) = scene_of(&root, &HashMap::new());
        let kinds: Vec<bool> = scene.content.iter().map(|p| matches!(p, Prim::Rect { border: Some(_), .. })).collect();
        assert_eq!(kinds, [false, true], "fill first, then the border");
    }

    #[test]
    fn content_hash_ignores_glass_but_not_content() {
        let a = column().child(div().glass().w(100.0).h(30.0)).child(text("x"));
        let b = column().child(div().glass().w(120.0).h(30.0)).child(text("x"));
        let c = column().child(div().glass().w(100.0).h(30.0)).child(text("y"));
        let hash = |e: &Element<()>| content_hash(&scene_of(e, &HashMap::new()).0);
        assert_eq!(hash(&a), hash(&b), "moving glass should not redraw the backdrop");
        assert_ne!(hash(&a), hash(&c));
    }
}
