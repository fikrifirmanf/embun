//! A list of a million rows. Only the rows on screen are built.
//!
//! cargo run --release -p embun --example list

use embun::*;

struct Rows {
    selected: Option<usize>,
}

impl App for Rows {
    type Message = usize;

    fn update(&mut self, row: usize) {
        self.selected = Some(row);
    }

    fn view(&self, theme: &Theme) -> Element<usize> {
        let selected = self.selected;
        let accent = theme.accent();
        let title = match selected {
            Some(i) => format!("Row {i} selected"),
            None => "1,000,000 rows".to_string(),
        };
        let rows = list(1_000_000, 32.0, move |i| {
            let row = row().key(format!("row{i}")).h_full().px_4().items_center().justify_between().child(text(format!("Row {i}"))).child(
                text(format!("{:.2}", (i as f32 * 0.37).sin().abs() * 100.0)).text_size(12.0),
            );
            if selected == Some(i) { row.bg(accent.with_alpha(0.25)).on_press(i) } else { row.on_press(i) }
        })
        .key("rows")
        .h_full();
        column().child(div().h(44.0).px_4().items_center().justify_center().child(text(title).font_semibold())).child(div().grow().child(rows))
    }
}

fn main() -> Result<(), RunError> {
    run(Rows { selected: None }, Settings::new("List").size(480.0, 640.0))
}
