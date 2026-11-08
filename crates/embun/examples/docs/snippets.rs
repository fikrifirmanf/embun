//! The code shown on the docs site. Each function here is rendered to a PNG and
//! printed on a page, so what the page shows is what the library draws.
//! The `Msg` enum is the one an app would define.

use embun::*;

#[derive(Clone)]
pub enum Msg {
    Save,
    Cancel,
    Back,
    Forward,
    Share,
    Pick(usize),
    Toggle,
    Volume(f32),
    View(usize),
    Text(String),
    Open(bool),
    Sort(usize),
    Row(usize),
}

pub fn button_basic(_theme: &Theme) -> Element<Msg> {
    row()
        .gap_3()
        .child(button("Cancel").on_press(Msg::Cancel))
        .child(button("Save").prominent().on_press(Msg::Save))
}

pub fn button_sizes(_theme: &Theme) -> Element<Msg> {
    row()
        .gap_3()
        .items_center()
        .child(button("Small").size(ControlSize::Small).on_press(Msg::Save))
        .child(button("Regular").on_press(Msg::Save))
        .child(button("Large").size(ControlSize::Large).on_press(Msg::Save))
        .child(button("Extra large").size(ControlSize::ExtraLarge).on_press(Msg::Save))
}

pub fn icon_buttons(_theme: &Theme) -> Element<Msg> {
    row()
        .gap_3()
        .child(icon_button("+").on_press(Msg::Save).label("Add"))
        .child(icon_button("−").on_press(Msg::Cancel).label("Remove"))
        .child(icon_button("×").prominent().on_press(Msg::Cancel).label("Close"))
}

pub fn toolbar(_theme: &Theme) -> Element<Msg> {
    row()
        .gap_3()
        .items_center()
        .child(
            toolbar_group()
                .child(toolbar_item("‹", Msg::Back).label("Back"))
                .child(toolbar_item("›", Msg::Forward).label("Forward")),
        )
        .child(button("Share").size(ControlSize::Large).prominent().on_press(Msg::Share))
}

pub fn sidebar_list(theme: &Theme) -> Element<Msg> {
    sidebar()
        .w(200.0)
        .child(sidebar_heading("Library").text_color(theme.secondary_label()))
        .child(sidebar_item("All Photos", true, Msg::Pick(0)).key("all"))
        .child(sidebar_item("Favorites", false, Msg::Pick(1)).key("favorites"))
        .child(sidebar_item("Travel", false, Msg::Pick(2)).key("travel"))
}

pub fn switches(theme: &Theme) -> Element<Msg> {
    row()
        .gap_4()
        .items_center()
        .child(switch(theme, true, Msg::Toggle).label("Wi-Fi"))
        .child(switch(theme, false, Msg::Toggle).label("Bluetooth"))
}

pub fn checkboxes(theme: &Theme) -> Element<Msg> {
    column()
        .gap_3()
        .child(checkbox(theme, "I have read the terms", true, Msg::Toggle))
        .child(checkbox(theme, "Remind me tomorrow", false, Msg::Toggle))
}

pub fn radios(theme: &Theme) -> Element<Msg> {
    column()
        .gap_3()
        .child(radio(theme, "Small", false, Msg::Pick(0)).key("small"))
        .child(radio(theme, "Medium", true, Msg::Pick(1)).key("medium"))
        .child(radio(theme, "Large", false, Msg::Pick(2)).key("large"))
}

pub fn sliders(theme: &Theme) -> Element<Msg> {
    slider(theme, 0.35, Msg::Volume).w(240.0).label("Volume")
}

pub fn segmented_control(theme: &Theme) -> Element<Msg> {
    segmented(theme, &["Day", "Week", "Month"], 1, Msg::View)
}

pub fn layout_stack(theme: &Theme) -> Element<Msg> {
    let tile = |label: &str| div().px_4().py_2().rounded_lg().bg(theme.accent()).child(text(label.to_string()).text_color(theme.on_accent()));
    column()
        .gap_3()
        .items_start()
        .child(row().gap_2().child(tile("One")).child(tile("Two")).child(tile("Three")))
        .child(column().gap_2().items_start().child(tile("Four")).child(tile("Five")))
}

pub fn layout_grow(theme: &Theme) -> Element<Msg> {
    let tile = |label: &str| div().px_4().py_2().rounded_lg().bg(theme.accent()).child(text(label.to_string()).text_color(theme.on_accent()));
    row()
        .w(320.0)
        .gap_2()
        .items_center()
        .child(tile("Fixed"))
        .child(div().grow().h(40.0).rounded_lg().bg(theme.surface()).items_center().justify_center().child(text("Grows")))
        .child(tile("Fixed"))
}

pub fn layout_wrap(theme: &Theme) -> Element<Msg> {
    let tile = |label: &str| div().px_3().py_1().rounded_full().bg(theme.surface()).child(text(label.to_string()));
    row()
        .flex_wrap()
        .gap_2()
        .w(300.0)
        .children(["Rust", "wgpu", "glass", "layout", "taffy", "text", "images"].map(tile))
}

pub fn layout_absolute(theme: &Theme) -> Element<Msg> {
    let tile = |label: &str| div().px_3().py_1().rounded_lg().bg(theme.accent()).child(text(label.to_string()).text_color(theme.on_accent()));
    div()
        .w(300.0)
        .h(140.0)
        .rounded_xl()
        .bg(theme.surface())
        .child(tile("Top left").absolute().top(12.0).left(12.0))
        .child(tile("Bottom right").absolute().bottom(12.0).right(12.0))
}

pub fn style_card(theme: &Theme) -> Element<Msg> {
    column()
        .w(280.0)
        .p_5()
        .gap_1()
        .rounded_2xl()
        .bg(theme.surface())
        .border(1.0, theme.separator())
        .child(text("Storage").font_semibold())
        .child(text("42 GB of 256 GB used").text_sm().text_color(theme.secondary_label()))
}

pub fn style_gradient(_theme: &Theme) -> Element<Msg> {
    div()
        .w(280.0)
        .h(100.0)
        .rounded_2xl()
        .bg_gradient(Color::hex(0xffd6a5), Color::hex(0xbde0fe))
}

pub fn text_styles(theme: &Theme) -> Element<Msg> {
    column()
        .gap_1()
        .items_start()
        .child(text("Large title").text_3xl().font_bold())
        .child(text("Headline").text_xl().font_semibold())
        .child(text("Body text at the base size."))
        .child(text("Caption in secondary color").text_xs().text_color(theme.secondary_label()))
}

pub fn text_wrapping(_theme: &Theme) -> Element<Msg> {
    text("Text wraps at the width you give it, and leading sets the line height as a multiple of the size.")
        .w(260.0)
        .text_center()
        .leading(1.5)
}

pub fn image_scaled(_theme: &Theme) -> Element<Msg> {
    let pixels: Vec<u8> = (0..64 * 64u32).flat_map(|i| [((i % 64) * 4) as u8, ((i / 64) * 4) as u8, 160, 255]).collect();
    image(Image::from_rgba(64, 64, pixels)).w(120.0).h(120.0).rounded(16.0)
}

pub fn field_single(_theme: &Theme) -> Element<Msg> {
    column()
        .gap_3()
        .child(text_input("Embun", Msg::Text).label("Name"))
        .child(text_input("", Msg::Text).placeholder("Search").label("Search"))
}

pub fn field_multi(_theme: &Theme) -> Element<Msg> {
    text_area("Buy milk\nCall the bank about the card that was declined on Friday", Msg::Text)
        .w(300.0)
        .h(110.0)
        .label("Notes")
}

pub fn list_rows(theme: &Theme) -> Element<Msg> {
    let line = theme.separator();
    list(1_000_000, 32.0, move |i| {
        row().h_full().items_center().px_4().border(0.5, line).child(text(format!("Item {}", i + 1)))
    })
    .w(280.0)
    .h(190.0)
    .rounded_xl()
}

pub fn menu_example(_theme: &Theme) -> Element<Msg> {
    menu()
        .child(menu_check_item("Newest First", true, Msg::Pick(0)))
        .child(menu_check_item("Oldest First", false, Msg::Pick(1)))
        .child(menu_separator())
        .child(menu_item("Share", Msg::Share))
}

pub fn dialog_example(theme: &Theme) -> Element<Msg> {
    dialog(
        Msg::Cancel,
        dialog_panel(theme)
            .child(text("Discard changes?").text_size(15.0).font_semibold())
            .child(text("Your edits will be lost.").text_size(13.0).text_color(theme.secondary_label()))
            .child(
                row()
                    .gap_3()
                    .justify_end()
                    .child(button("Keep editing").on_press(Msg::Cancel))
                    .child(button("Discard").prominent().on_press(Msg::Save)),
            ),
    )
}

pub fn theme_palette(theme: &Theme) -> Element<Msg> {
    let swatch = |name: &str, color: Color| {
        column()
            .items_center()
            .gap_1()
            .child(div().w(52.0).h(52.0).rounded_lg().bg(color).border(1.0, theme.separator()))
            .child(text(name.to_string()).text_xs())
    };
    row()
        .gap_3()
        .child(swatch("background", theme.background()))
        .child(swatch("surface", theme.surface()))
        .child(swatch("label", theme.label()))
        .child(swatch("separator", theme.separator()))
        .child(swatch("accent", theme.accent()))
}

pub fn progress_bars(theme: &Theme) -> Element<Msg> {
    column()
        .gap_4()
        .items_start()
        .child(progress(theme, 0.65).label("Upload"))
        .child(progress(theme, 0.25).label("Download"))
}

pub fn progress_unknown(theme: &Theme) -> Element<Msg> {
    column()
        .gap_4()
        .items_start()
        .child(progress_indeterminate(theme).label("Loading"))
        .child(spinner(theme).label("Loading"))
}

pub fn badges(theme: &Theme) -> Element<Msg> {
    let entry = |name: &str, count: &str| row().items_center().gap_2().child(text(name.to_string())).child(badge(theme, count.to_string()));
    row().gap_6().child(entry("Inbox", "12")).child(entry("Mentions", "3")).child(entry("Drafts", "1,204"))
}

pub fn dividers(theme: &Theme) -> Element<Msg> {
    column()
        .w(220.0)
        .gap_2()
        .child(text("Above"))
        .child(divider(theme))
        .child(text("Below"))
}

pub fn tooltip_example(_theme: &Theme) -> Element<Msg> {
    button("Save").tooltip("Save the document").on_press(Msg::Save).into()
}

pub fn disclosure_example(theme: &Theme) -> Element<Msg> {
    column()
        .w(260.0)
        .gap_2()
        .child(disclosure(
            theme,
            "Advanced",
            true,
            Msg::Toggle,
            column().gap_2().child(checkbox(theme, "Show hidden files", false, Msg::Toggle)).child(checkbox(theme, "Use the cache", true, Msg::Toggle)),
        ))
        .child(divider(theme))
        .child(disclosure(theme, "About", false, Msg::Toggle, text("Version 0.3")))
}

pub fn dropdown_closed(theme: &Theme) -> Element<Msg> {
    div().w(220.0).child(dropdown(theme, &["Small", "Medium", "Large"], 1, false, Msg::Open, Msg::Pick))
}

pub fn dropdown_open(theme: &Theme) -> Element<Msg> {
    div().w(220.0).child(dropdown(theme, &["Small", "Medium", "Large"], 1, true, Msg::Open, Msg::Pick))
}

pub fn table_files(theme: &Theme) -> Element<Msg> {
    const FILES: [(&str, &str, &str); 5] = [
        ("Notes.txt", "Text", "2 KB"),
        ("Photo.jpg", "Image", "3.1 MB"),
        ("Budget.csv", "Spreadsheet", "48 KB"),
        ("Archive.zip", "Archive", "12 MB"),
        ("Readme.md", "Markdown", "6 KB"),
    ];
    table(theme, vec![Column::new("Name").grow(2.0), Column::new("Kind").width(110.0), Column::new("Size").width(80.0).right()], FILES.len(), |row, column| {
        let file = FILES[row];
        text([file.0, file.1, file.2][column]).no_wrap()
    })
    .selected(Some(1))
    .on_select(Msg::Row)
    .sorted_by(0, true)
    .on_sort(Msg::Sort)
    .w(460.0)
    .h(190.0)
    .into()
}
