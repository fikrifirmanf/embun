//! A table: a header row over a virtual list of rows.

use std::rc::Rc;

use crate::Theme;
use crate::element::{Element, Role, column, div, list_with, row, text};

/// How wide a table column is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Width {
    Px(f32),
    Share(f32),
}

/// One column of a [`table`]: a title and a width.
#[derive(Clone, Debug)]
pub struct Column {
    title: String,
    width: Width,
    right: bool,
}

impl Column {
    /// A column that shares the leftover width equally with the other growing columns.
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), width: Width::Share(1.0), right: false }
    }

    /// A fixed width in pixels.
    pub fn width(mut self, px: f32) -> Self {
        self.width = Width::Px(px);
        self
    }

    /// Takes `share` parts of the leftover width, where plain columns take 1.
    pub fn grow(mut self, share: f32) -> Self {
        self.width = Width::Share(share);
        self
    }

    /// Aligns the title and the cells to the right, for numbers.
    pub fn right(mut self) -> Self {
        self.right = true;
        self
    }
}

type Cell<M> = Rc<dyn Fn(usize, usize) -> Element<M>>;

/// A table with a header row and `rows` virtual rows. Convert it into an
/// [`Element`] by passing it to `.child(...)`, or with `.into()`.
pub struct Table<M> {
    theme: Theme,
    columns: Vec<Column>,
    rows: usize,
    row_height: f32,
    cell: Cell<M>,
    selected: Option<usize>,
    on_select: Option<Rc<dyn Fn(usize) -> M>>,
    sort: Option<(usize, bool)>,
    on_sort: Option<Rc<dyn Fn(usize) -> M>>,
    width: Option<f32>,
    height: f32,
}

/// A table of `rows` rows. `cell(row, column)` builds one cell; return
/// `text(...).no_wrap()` for text so long values are cut at the column edge.
/// Only the rows on screen are built, so a million rows cost the same as ten.
/// Your app owns the data, the sort order and the selection.
pub fn table<M: Clone + 'static>(theme: &Theme, columns: Vec<Column>, rows: usize, cell: impl Fn(usize, usize) -> Element<M> + 'static) -> Table<M> {
    Table { theme: *theme, columns, rows, row_height: 28.0, cell: Rc::new(cell), selected: None, on_select: None, sort: None, on_sort: None, width: None, height: 240.0 }
}

impl<M: Clone + 'static> Table<M> {
    /// Highlights row `index`. With `on_select`, pressing a row, or Up and
    /// Down on a focused row, sends that row's index.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    /// Turns a row index into the message that selects it.
    pub fn on_select(mut self, on_select: impl Fn(usize) -> M + 'static) -> Self {
        self.on_select = Some(Rc::new(on_select));
        self
    }

    /// Marks column `column` as the sort column, ascending or descending.
    /// The table only shows the arrow; sorting your data is up to you.
    pub fn sorted_by(mut self, column: usize, ascending: bool) -> Self {
        self.sort = Some((column, ascending));
        self
    }

    /// Makes the headers pressable: `on_sort(column)` is sent when one is pressed.
    pub fn on_sort(mut self, on_sort: impl Fn(usize) -> M + 'static) -> Self {
        self.on_sort = Some(Rc::new(on_sort));
        self
    }

    /// Row height in pixels; 28 by default. Every row has the same height.
    pub fn row_height(mut self, px: f32) -> Self {
        self.row_height = px;
        self
    }

    /// Total width in pixels; the table fills its parent's width by default.
    pub fn w(mut self, px: f32) -> Self {
        self.width = Some(px);
        self
    }

    /// Total height in pixels, header included; 240 by default.
    pub fn h(mut self, px: f32) -> Self {
        self.height = px;
        self
    }
}

fn cell_box<M>(width: Width, right: bool) -> Element<M> {
    let e = match width {
        Width::Px(px) => row().w(px).shrink_0(),
        Width::Share(share) => row().w(0.0).grow_by(share.max(0.0)).min_w(0.0),
    };
    let e = e.h_full().px_3().items_center().clip();
    if right { e.justify_end() } else { e.justify_start() }
}

impl<M: Clone + 'static> From<Table<M>> for Element<M> {
    fn from(t: Table<M>) -> Self {
        let theme = t.theme;
        let header = row().h(28.0).shrink_0().bg(theme.surface()).role(Role::Row).children(t.columns.iter().enumerate().map(|(c, col)| {
            let arrow = match t.sort {
                Some((sorted, ascending)) if sorted == c => Some(if ascending { "\u{2191}" } else { "\u{2193}" }),
                _ => None,
            };
            let mut label = text(col.title.clone()).text_size(12.0).font_semibold().text_color(theme.secondary_label()).no_wrap();
            if arrow.is_some() {
                label = label.text_color(theme.label());
            }
            let mut cell = cell_box(col.width, col.right).role(Role::ColumnHeader).gap_1().child(label);
            if let Some(arrow) = arrow {
                cell = cell.child(text(arrow).text_size(12.0).font_bold().no_wrap());
            }
            let name = match t.sort {
                Some((sorted, ascending)) if sorted == c => format!("{}, sorted {}", col.title, if ascending { "ascending" } else { "descending" }),
                _ => col.title.clone(),
            };
            cell = cell.label(name);
            if let Some(on_sort) = &t.on_sort {
                cell = cell.on_press(on_sort(c)).role(Role::ColumnHeader).key(format!("header{c}"));
            }
            cell
        }));

        let columns = t.columns.clone();
        let cell = t.cell.clone();
        let on_select = t.on_select.clone();
        let selected = t.selected;
        let stripe = crate::Color::rgba(128, 128, 128, if theme.is_dark() { 0.1 } else { 0.07 });
        let body = list_with(
            t.rows,
            t.row_height,
            move |i| {
                let on = selected == Some(i);
                let mut r = row().h_full().items_center().role(Role::Row).checked(on).label(format!("Row {}", i + 1));
                if on {
                    r = r.bg(theme.accent()).text_color(theme.on_accent());
                } else if i % 2 == 1 {
                    r = r.bg(stripe);
                }
                if let Some(on_select) = &on_select {
                    r = r.on_press(on_select(i)).role(Role::Row);
                }
                r.children(columns.iter().enumerate().map(|(c, col)| cell_box(col.width, col.right).role(Role::Cell).child(cell(i, c))))
            },
            t.on_select.clone().map(|f| Box::new(move |i| f(i)) as Box<dyn Fn(usize) -> M>),
        )
        .grow()
        .min_h(0.0);

        let mut e = column().h(t.height).shrink_0().rounded(10.0).border(1.0, theme.separator()).clip().bg(theme.background()).role(Role::Table).child(header).child(
            div().h(1.0).w_full().shrink_0().bg(theme.separator()),
        ).child(body);
        e = match t.width {
            Some(w) => e.w(w),
            None => e.w_full(),
        };
        e
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Prim;
    use crate::{App, Key, Ui};

    struct Files {
        count: usize,
        selected: Option<usize>,
        sort: (usize, bool),
    }

    #[derive(Clone)]
    enum Msg {
        Select(usize),
        Sort(usize),
    }

    impl App for Files {
        type Message = Msg;

        fn update(&mut self, message: Msg) {
            match message {
                Msg::Select(i) => self.selected = Some(i),
                Msg::Sort(c) => self.sort = (c, if self.sort.0 == c { !self.sort.1 } else { true }),
            }
        }

        fn view(&self, theme: &Theme) -> Element<Msg> {
            column().items_start().child(
                table(theme, vec![Column::new("Name").width(80.0), Column::new("Size").right(), Column::new("Kind")], self.count, |r, c| {
                    text(format!("r{r}c{c} has a value far too long for its column")).no_wrap().key(format!("cell{r}-{c}"))
                })
                .selected(self.selected)
                .on_select(Msg::Select)
                .sorted_by(self.sort.0, self.sort.1)
                .on_sort(Msg::Sort)
                .w(400.0)
                .h(200.0),
            )
        }
    }

    fn files(count: usize) -> Ui<Files> {
        Ui::new(Files { count, selected: None, sort: (0, true) }, Theme::default(), [500.0, 400.0])
    }

    fn center(r: [f32; 4]) -> [f32; 2] {
        [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]
    }

    fn click(ui: &mut Ui<Files>, at: [f32; 2]) {
        ui.pointer_moved(at);
        ui.pointer_pressed();
        ui.pointer_released();
    }

    #[test]
    fn columns_take_their_widths_and_rows_stack_below_the_header() {
        let mut ui = files(50);
        let a = ui.rect_of_key("cell0-0").expect("cell");
        let b = ui.rect_of_key("cell0-1").expect("cell");
        let c = ui.rect_of_key("cell0-2").expect("cell");
        let next_row = ui.rect_of_key("cell1-0").expect("cell");
        // Name is 80 wide; Size and Kind share the other 320 equally, 160 each, with 12 px of padding.
        assert_eq!(a[0], 12.0, "the first column's text starts after its padding");
        assert_eq!(c[0], 80.0 + 160.0 + 12.0, "the third column starts after 80 + 160 px");
        assert_eq!(b[0] + b[2], 80.0 + 160.0 - 12.0, "the right-aligned column's text ends at its right padding");
        assert_eq!(next_row[1] - a[1], 28.0, "rows are 28 px apart");
        assert!(a[1] >= 28.0, "the first row sits below the 28 px header: {a:?}");
    }

    #[test]
    fn long_text_is_cut_at_the_column_edge_not_wrapped_or_spilled() {
        let mut ui = files(10);
        let name = ui.rect_of_key("cell0-0").expect("cell");
        assert!(name[2] > 200.0, "the text is as long as it is: {name:?}");
        assert!(name[3] < 20.0, "and stays on one line: {name:?}");
        let scene = ui.scene().clone();
        let clip = scene
            .content
            .iter()
            .find_map(|p| match p {
                Prim::Text { text, clip, .. } if text.starts_with("r0c0") => Some(*clip),
                _ => None,
            })
            .expect("the first cell's text is drawn");
        assert!(clip[2] <= 80.5, "drawn only inside its 80 px column: {clip:?}");
    }

    #[test]
    fn pressing_a_row_selects_it_and_highlights_it() {
        let mut ui = files(50);
        let cell = ui.rect_of_key("cell3-1").expect("cell");
        click(&mut ui, center(cell));
        assert_eq!(ui.app().selected, Some(3));
        let accent = Theme::default().accent();
        let filled = ui.scene().content.iter().any(|p| matches!(p, Prim::Rect { top, .. } if *top == accent));
        assert!(filled, "the selected row is filled with the accent color");
    }

    #[test]
    fn pressing_a_header_sends_its_column_and_the_arrow_follows() {
        let mut ui = files(10);
        let header = ui.rect_of_key("header1").expect("header");
        click(&mut ui, center(header));
        assert_eq!(ui.app().sort, (1, true));
        click(&mut ui, center(header));
        assert_eq!(ui.app().sort, (1, false), "pressing again reverses it");
        let arrows: Vec<String> = ui
            .scene()
            .content
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, .. } if text == "\u{2191}" || text == "\u{2193}" => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(arrows, ["\u{2193}"], "one arrow, pointing down, on the sorted column");
    }

    #[test]
    fn a_million_rows_build_only_the_visible_ones() {
        let mut ui = files(1_000_000);
        assert!(ui.rect_of_key("cell0-0").is_some());
        assert!(ui.rect_of_key("cell40-0").is_none(), "rows far below are not built");
        ui.pointer_moved([100.0, 100.0]);
        ui.scrolled(28.0 * 500_000.0);
        assert!(ui.rect_of_key("cell0-0").is_none());
        assert!(ui.rect_of_key("cell500000-0").is_some() || ui.rect_of_key("cell499999-0").is_some(), "the rows at the new offset are built");
    }

    #[test]
    fn arrow_keys_step_the_selection_and_scroll_it_into_view() {
        let mut ui = files(1000);
        let cell = ui.rect_of_key("cell0-1").expect("cell");
        click(&mut ui, center(cell));
        assert_eq!(ui.app().selected, Some(0));
        // Focus order: the three headers, then the rows. Tab to the first row.
        for _ in 0..4 {
            ui.key(Key::Tab);
        }
        for _ in 0..30 {
            ui.key(Key::Down);
        }
        assert_eq!(ui.app().selected, Some(30), "each Down selected the next row");
        let cell = ui.rect_of_key("cell30-0").expect("row 30 is built, so it was scrolled into view");
        assert!(cell[1] >= 28.0 && cell[1] + cell[3] <= 230.0, "and it lies inside the table's body: {cell:?}");
        ui.key(Key::Up);
        assert_eq!(ui.app().selected, Some(29));
        ui.key(Key::Activate);
        assert_eq!(ui.app().selected, Some(29), "the focus followed the selection");
    }

    #[test]
    fn up_at_the_first_row_and_down_at_the_last_stay_put() {
        let mut ui = files(3);
        for _ in 0..4 {
            ui.key(Key::Tab);
        }
        ui.key(Key::Up);
        assert_eq!(ui.app().selected, None, "nothing above the first row");
        for _ in 0..5 {
            ui.key(Key::Down);
        }
        assert_eq!(ui.app().selected, Some(2), "stops at the last row");
    }

    #[test]
    fn no_wrap_keeps_text_on_one_line_in_a_narrow_column() {
        use crate::layout::{flatten, layout};
        use crate::text::TextSystem;
        use std::collections::HashMap;
        let long = "a sentence that is much wider than sixty pixels";
        let height_of = |e: Element<()>| {
            let root = column().w(60.0).items_start().child(e);
            let theme = Theme::default();
            let mut nodes = flatten(&root, &theme);
            layout(&mut nodes, [400.0, 400.0], &mut TextSystem::new(), &HashMap::new());
            nodes[1].rect
        };
        let wrapped = height_of(text(long));
        let single = height_of(text(long).no_wrap());
        assert!(wrapped[3] > 30.0, "without no_wrap it wraps onto several lines: {wrapped:?}");
        assert!(single[3] < 20.0 && single[2] > 150.0, "with no_wrap it is one long line: {single:?}");
    }
}
