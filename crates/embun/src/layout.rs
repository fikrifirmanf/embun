use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use taffy::prelude::*;
use taffy::{Overflow, Point};

use crate::element::{Align, Direction, Element, Justify, Kind, Length, TextAlign};
use crate::text::{TextStyle, TextSystem};
use crate::{Color, Theme};

/// A rectangle in logical pixels: x, y, width, height.
pub type Rect4 = [f32; 4];

/// One element in pre-order, with everything resolved from its ancestors.
pub(crate) struct Node<'a, M> {
    pub el: &'a Element<M>,
    pub id: u64,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub text: TextStyle,
    pub color: Color,
    /// The nearest glass ancestor, whose surface this element sits on.
    pub on_glass: Option<usize>,
    /// The element or an ancestor is an overlay.
    pub in_overlay: bool,
    /// Border box in window coordinates, after scrolling.
    pub rect: Rect4,
    /// What is visible of it: the window, cut by any scrolling ancestors.
    pub clip: Rect4,
}

const DEFAULT_TEXT: TextStyle = TextStyle { size: 13.0, weight: 400, line_height: 1.25, align: TextAlign::Left };

/// Lists the tree in pre-order with ids, inherited text styles and parents.
pub(crate) fn flatten<'a, M>(root: &'a Element<M>, theme: &Theme) -> Vec<Node<'a, M>> {
    let mut nodes = Vec::new();
    visit(root, None, 0, &mut nodes, theme);
    nodes
}

fn visit<'a, M>(el: &'a Element<M>, parent: Option<usize>, index: usize, nodes: &mut Vec<Node<'a, M>>, theme: &Theme) {
    let (text, color, on_glass, parent_id, parent_overlay) = match parent {
        Some(p) => {
            let n = &nodes[p];
            // An overlay starts a new stack: glass under it is not "its" glass.
            let on_glass = if el.style.overlay {
                None
            } else if n.el.style.glass.is_some() {
                Some(p)
            } else {
                n.on_glass
            };
            (n.text.clone(), n.color, on_glass, n.id, n.in_overlay)
        }
        None => (DEFAULT_TEXT, theme.label(), None, 0x656d_6275_6e00, false),
    };
    let s = &el.style;
    let text = TextStyle {
        size: s.font_size.unwrap_or(text.size),
        weight: s.font_weight.unwrap_or(text.weight),
        line_height: s.line_height.unwrap_or(text.line_height),
        align: s.text_align.unwrap_or(text.align),
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    parent_id.hash(&mut hasher);
    match &el.key {
        Some(key) => key.hash(&mut hasher),
        None => index.hash(&mut hasher),
    }
    let me = nodes.len();
    nodes.push(Node {
        el,
        id: hasher.finish(),
        parent,
        children: Vec::new(),
        text,
        color: s.text_color.unwrap_or(color),
        on_glass,
        in_overlay: parent_overlay || el.style.overlay,
        rect: [0.0; 4],
        clip: [0.0; 4],
    });
    if let Some(p) = parent {
        nodes[p].children.push(me);
    }
    for (i, child) in el.children.iter().enumerate() {
        visit(child, Some(me), i, nodes, theme);
    }
}

/// What layout learned about a scroll container.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ScrollInfo {
    pub max: f32,
}

/// Lays the nodes out in a `viewport` (logical pixels) and fills in their
/// rects and clips. `scroll` holds the offset of each scroll container by id.
pub(crate) fn layout<M>(
    nodes: &mut [Node<'_, M>],
    viewport: [f32; 2],
    text: &mut TextSystem,
    scroll: &HashMap<u64, f32>,
) -> HashMap<u64, ScrollInfo> {
    let mut tree: TaffyTree<usize> = TaffyTree::new();
    let ids: Vec<NodeId> = build(&mut tree, nodes, 0);
    let root = tree
        .new_with_children(
            Style {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                size: Size { width: length(viewport[0]), height: length(viewport[1]) },
                ..Default::default()
            },
            &[ids[0]],
        )
        .expect("taffy root");
    tree.compute_layout_with_measure(
        root,
        Size { width: AvailableSpace::Definite(viewport[0]), height: AvailableSpace::Definite(viewport[1]) },
        |inputs, _, context, style| {
            taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |known, available| match context {
                Some(&mut index) => measure(&nodes[index], known, available, text),
                None => Size::ZERO,
            })
        },
    )
    .expect("taffy layout");

    let mut scrolls = HashMap::new();
    let window = [0.0, 0.0, viewport[0], viewport[1]];
    place(nodes, &tree, &ids, 0, [0.0, 0.0], window, scroll, &mut scrolls);
    scrolls
}

#[allow(clippy::too_many_arguments)]
fn place<M>(
    nodes: &mut [Node<'_, M>],
    tree: &TaffyTree<usize>,
    ids: &[NodeId],
    i: usize,
    origin: [f32; 2],
    clip: Rect4,
    scroll: &HashMap<u64, f32>,
    scrolls: &mut HashMap<u64, ScrollInfo>,
) {
    let l = tree.layout(ids[i]).expect("layout");
    let rect = [origin[0] + l.location.x, origin[1] + l.location.y, l.size.width, l.size.height];
    nodes[i].rect = rect;
    nodes[i].clip = clip;
    let mut child_origin = [rect[0], rect[1]];
    let mut child_clip = clip;
    if nodes[i].el.style.scroll_y {
        let max = (l.scrollable_overflow_rect.bottom - l.size.height).max(0.0);
        let offset = scroll.get(&nodes[i].id).copied().unwrap_or(0.0).clamp(0.0, max);
        scrolls.insert(nodes[i].id, ScrollInfo { max });
        child_origin[1] -= offset;
        child_clip = intersect(clip, rect);
    }
    for c in nodes[i].children.clone() {
        place(nodes, tree, ids, c, child_origin, child_clip, scroll, scrolls);
    }
}

pub(crate) fn intersect(a: Rect4, b: Rect4) -> Rect4 {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

pub(crate) fn contains(r: Rect4, p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}

/// Creates taffy nodes for `i` and its subtree; returns the id of every node
/// by index (filled for the whole subtree).
fn build<M>(tree: &mut TaffyTree<usize>, nodes: &[Node<'_, M>], i: usize) -> Vec<NodeId> {
    let mut ids = vec![NodeId::from(0u64); nodes.len()];
    build_into(tree, nodes, i, &mut ids, true);
    ids
}

fn build_into<M>(tree: &mut TaffyTree<usize>, nodes: &[Node<'_, M>], i: usize, ids: &mut [NodeId], is_root: bool) {
    let mut style = taffy_style(&nodes[i]);
    // Content inside a scroll container keeps its size and overflows.
    if nodes[i].parent.is_some_and(|p| nodes[p].el.style.scroll_y) {
        style.flex_shrink = 0.0;
    }
    if is_root {
        // The app's root is the window's size unless it sets its own, and
        // does not grow with its content.
        if nodes[i].el.style.width == Length::Auto {
            style.size.width = percent(1.0);
        }
        if nodes[i].el.style.height == Length::Auto {
            style.size.height = percent(1.0);
        }
        style.min_size = Size { width: length(0.0), height: length(0.0) };
    }
    let id = match nodes[i].el.kind {
        Kind::Text(_) | Kind::Image(_) | Kind::Input { .. } => tree.new_leaf_with_context(style, i).expect("taffy leaf"),
        Kind::Div => {
            for &c in &nodes[i].children {
                build_into(tree, nodes, c, ids, false);
            }
            let children: Vec<NodeId> = nodes[i].children.iter().map(|&c| ids[c]).collect();
            tree.new_with_children(style, &children).expect("taffy node")
        }
    };
    ids[i] = id;
}

fn dimension(l: Length) -> Dimension {
    match l {
        Length::Auto => auto(),
        Length::Px(px) => length(px),
        Length::Fraction(f) => percent(f),
    }
}

fn taffy_style<M>(node: &Node<'_, M>) -> Style {
    let s = &node.el.style;
    let lp = |v: f32| LengthPercentage::length(v);
    let align = |a: Align| match a {
        Align::Start => AlignItems::START,
        Align::Center => AlignItems::CENTER,
        Align::End => AlignItems::END,
        Align::Stretch => AlignItems::STRETCH,
    };
    let inset = |v: Option<f32>| v.map_or(LengthPercentageAuto::auto(), LengthPercentageAuto::length);
    let opt = |v: Option<f32>| v.map_or(LengthPercentageAuto::auto(), LengthPercentageAuto::length);
    Style {
        display: Display::Flex,
        flex_direction: match s.direction {
            Direction::Row => FlexDirection::Row,
            Direction::Column => FlexDirection::Column,
        },
        flex_wrap: if s.wrap { FlexWrap::Wrap } else { FlexWrap::NoWrap },
        gap: Size { width: lp(s.gap), height: lp(s.gap) },
        padding: Rect { top: lp(s.padding[0]), right: lp(s.padding[1]), bottom: lp(s.padding[2]), left: lp(s.padding[3]) },
        size: Size { width: dimension(s.width), height: dimension(s.height) },
        min_size: Size { width: opt(s.min_width), height: opt(s.min_height) },
        max_size: Size { width: opt(s.max_width), height: opt(s.max_height) },
        flex_grow: s.grow,
        flex_shrink: if s.no_shrink { 0.0 } else { 1.0 },
        align_items: s.align_items.map_or(AlignItems::NORMAL, align),
        align_self: s.align_self.map(align),
        justify_content: match s.justify {
            None | Some(Justify::Start) => JustifyContent::START,
            Some(Justify::Center) => JustifyContent::CENTER,
            Some(Justify::End) => JustifyContent::END,
            Some(Justify::Between) => JustifyContent::SPACE_BETWEEN,
        },
        position: if s.absolute { Position::Absolute } else { Position::Relative },
        inset: Rect { top: inset(s.inset[0]), right: inset(s.inset[1]), bottom: inset(s.inset[2]), left: inset(s.inset[3]) },
        overflow: Point {
            x: Overflow::Visible,
            y: if s.scroll_y { Overflow::Scroll } else { Overflow::Visible },
        },
        // Scroll containers reserve no space for a scrollbar.
        scrollbar_width: 0.0,
        ..Default::default()
    }
}

fn measure<M>(
    node: &Node<'_, M>,
    known: Size<Option<f32>>,
    available: Size<AvailableSpace>,
    text: &mut TextSystem,
) -> Size<f32> {
    if let Size { width: Some(width), height: Some(height) } = known {
        return Size { width, height };
    }
    match &node.el.kind {
        Kind::Text(content) => {
            let max_width = known.width.or(match available.width {
                AvailableSpace::Definite(w) => Some(w),
                AvailableSpace::MinContent => Some(0.0),
                AvailableSpace::MaxContent => None,
            });
            let [w, h] = text.measure(content, &node.text, max_width);
            Size { width: known.width.unwrap_or(w), height: known.height.unwrap_or(h) }
        }
        Kind::Image(image) => {
            let aspect = image.height as f32 / image.width as f32;
            match (known.width, known.height) {
                (Some(w), None) => Size { width: w, height: w * aspect },
                (None, Some(h)) => Size { width: h / aspect, height: h },
                _ => Size { width: image.width as f32, height: image.height as f32 },
            }
        }
        Kind::Input { .. } => {
            // Its own size comes from the style; this is only the fallback.
            let line = node.text.size * node.text.line_height;
            Size { width: known.width.unwrap_or(200.0), height: known.height.unwrap_or(line + 8.0) }
        }
        Kind::Div => Size::ZERO,
    }
}

/// Where the text of a field starts (window coordinates, after `scroll`) and
/// the width it wraps at; `None` for a single line. A single line is
/// centered vertically; a text area starts at its top padding.
pub(crate) fn field_geometry(rect: Rect4, style: &crate::element::Style, text: &TextStyle, multiline: bool, scroll: [f32; 2]) -> ([f32; 2], Option<f32>) {
    let [top, right, bottom, left] = style.padding;
    let inner_w = (rect[2] - left - right).max(1.0);
    let inner_h = rect[3] - top - bottom;
    let x = rect[0] + left - scroll[0];
    if multiline {
        ([x, rect[1] + top - scroll[1]], Some(inner_w))
    } else {
        let line = text.size * text.line_height;
        ([x, rect[1] + top + (inner_h - line) / 2.0], None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::{column, div, row, text};

    fn run(root: &Element<()>, viewport: [f32; 2]) -> Vec<Rect4> {
        let theme = Theme::default();
        let mut nodes = flatten(root, &theme);
        layout(&mut nodes, viewport, &mut TextSystem::new(), &HashMap::new());
        nodes.iter().map(|n| n.rect).collect()
    }

    #[test]
    fn root_fills_the_window_and_padding_and_gap_place_children() {
        let root = column().p_4().gap_2().child(div().h(20.0)).child(div().h(30.0));
        let rects = run(&root, [300.0, 200.0]);
        assert_eq!(rects[0], [0.0, 0.0, 300.0, 200.0]);
        assert_eq!(rects[1], [16.0, 16.0, 268.0, 20.0]);
        assert_eq!(rects[2], [16.0, 44.0, 268.0, 30.0]);
    }

    #[test]
    fn rows_share_free_space_with_growing_children() {
        let root = row().child(div().w(50.0)).child(div().grow()).child(div().w(50.0));
        let rects = run(&root, [300.0, 100.0]);
        assert_eq!(rects[2][2], 200.0, "middle child should take the rest");
        assert_eq!(rects[3][0], 250.0);
    }

    #[test]
    fn text_gets_its_measured_size_and_centers() {
        let root = column().items_center().justify_center().child(text("Hello"));
        let rects = run(&root, [300.0, 200.0]);
        let [x, y, w, h] = rects[1];
        assert!(w > 20.0 && h >= 16.0, "text size {w} x {h}");
        assert!((x + w / 2.0 - 150.0).abs() <= 1.0 && (y + h / 2.0 - 100.0).abs() <= 1.0, "text at {:?}", rects[1]);
    }

    #[test]
    fn absolute_children_sit_at_their_insets_over_the_flow() {
        let root = column().child(div().h(500.0)).child(div().absolute().top(8.0).left(8.0).right(8.0).h(52.0));
        let rects = run(&root, [400.0, 300.0]);
        assert_eq!(rects[2], [8.0, 8.0, 384.0, 52.0]);
    }

    #[test]
    fn scrolled_children_move_up_and_are_clipped_to_the_container() {
        let root: Element<()> = column().child(column().h(100.0).scroll_y().child(div().h(40.0)).child(div().h(200.0)));
        let theme = Theme::default();
        let mut nodes = flatten(&root, &theme);
        let scroller = nodes[1].id;
        let scrolls = layout(&mut nodes, [200.0, 300.0], &mut TextSystem::new(), &HashMap::from([(scroller, 30.0)]));
        assert_eq!(scrolls[&scroller].max, 140.0);
        assert_eq!(nodes[2].rect[1], -30.0);
        assert_eq!(nodes[3].clip, [0.0, 0.0, 200.0, 100.0]);
    }

    #[test]
    fn keys_keep_ids_when_siblings_are_inserted_before() {
        let theme = Theme::default();
        let before: Element<()> = column().child(div().key("save"));
        let after: Element<()> = column().child(div()).child(div().key("save"));
        let id = |root: &Element<()>, i: usize| flatten(root, &theme)[i].id;
        assert_eq!(id(&before, 1), id(&after, 2));
        let unkeyed_before: Element<()> = column().child(div());
        let unkeyed_after: Element<()> = column().child(div()).child(div());
        assert_ne!(id(&unkeyed_before, 1), id(&unkeyed_after, 2));
    }
}
