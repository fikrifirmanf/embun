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
