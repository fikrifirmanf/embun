//! Every control on one screen, over colorful content so the glass shows.

use embun::*;

use super::photos::landscape;

pub struct Controls {
    pub backdrop: Image,
    pub wifi: bool,
    pub dark_mode: bool,
    pub agree: bool,
    pub remind: bool,
    pub volume: f32,
    pub view: usize,
    pub clicks: u32,
    pub size: usize,
    pub dialog: bool,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Wifi,
    DarkMode,
    Agree,
    Remind,
    Volume(f32),
    View(usize),
    Click,
    Size(usize),
    Dialog(bool),
}

impl Controls {
    pub fn new() -> Self {
        Self { backdrop: landscape(3, 640, 400), wifi: true, dark_mode: false, agree: true, remind: false, volume: 0.35, view: 0, clicks: 0, size: 1, dialog: false }
    }
}

impl App for Controls {
    type Message = Msg;

    fn update(&mut self, message: Msg) {
        match message {
            Msg::Wifi => self.wifi = !self.wifi,
            Msg::DarkMode => self.dark_mode = !self.dark_mode,
            Msg::Agree => self.agree = !self.agree,
            Msg::Remind => self.remind = !self.remind,
            Msg::Volume(v) => self.volume = v,
            Msg::View(i) => self.view = i,
            Msg::Click => {
                self.clicks += 1;
                self.dialog = false;
            }
            Msg::Size(i) => self.size = i,
            Msg::Dialog(open) => self.dialog = open,
        }
    }

    fn view(&self, theme: &Theme) -> Element<Msg> {
        let line = |name: &str, control: Element<Msg>| row().items_center().justify_between().w_full().child(text(name.to_string()).text_size(13.0)).child(control);
        let card = column()
            .w(380.0)
            .p_5()
            .gap_4()
            .rounded(22.0)
            .bg(theme.surface())
            .child(text("Controls").text_size(17.0).font_semibold())
            .child(segmented(theme, &["Day", "Week", "Month"], self.view, Msg::View))
            .child(line("Wi-Fi", switch(theme, self.wifi, Msg::Wifi).label("Wi-Fi").key("wifi")))
            .child(line("Dark mode", switch(theme, self.dark_mode, Msg::DarkMode).label("Dark mode").key("dark")))
            .child(line("Volume", slider(theme, self.volume, Msg::Volume).w(180.0).key("volume").label("Volume")))
            .child(checkbox(theme, "I have read the terms", self.agree, Msg::Agree).key("agree"))
            .child(checkbox(theme, "Remind me tomorrow", self.remind, Msg::Remind).key("remind"))
            .child(
                row().gap_4().children(["Small", "Medium", "Large"].iter().enumerate().map(|(i, name)| radio(theme, *name, self.size == i, Msg::Size(i)).key(format!("size{i}")))),
            )
            .child(
                row()
                    .gap_3()
                    .justify_end()
                    .w_full()
                    .child(button("Cancel").on_press(Msg::Dialog(true)).key("cancel"))
                    .child(button(format!("Save ({})", self.clicks)).prominent().on_press(Msg::Click).key("save")),
            );
        let mut window = div().items_center().justify_center().child(image(self.backdrop.clone()).absolute().inset(0.0)).child(card);
        if self.dialog {
            let panel = dialog_panel(theme)
                .child(text("Discard changes?").text_size(15.0).font_semibold())
                .child(text("Your edits to these settings will be lost.").text_size(13.0).text_color(theme.secondary_label()))
                .child(
                    row()
                        .gap_3()
                        .justify_end()
                        .child(button("Keep editing").on_press(Msg::Dialog(false)))
                        .child(button("Discard").prominent().on_press(Msg::Click).key("discard")),
                );
            window = window.child(dialog(Msg::Dialog(false), panel));
        }
        window
    }
}
