//! A title field and a multi-line note, with a live count.

use embun::*;

pub struct Notes {
    pub title: String,
    pub body: String,
}

#[derive(Clone, Debug)]
pub enum Msg {
    Title(String),
    Body(String),
}

impl Notes {
    pub fn new() -> Self {
        Self {
            title: "Trip plan".into(),
            body: "Day 1: arrive in Yogyakarta, walk along Malioboro and find dinner.\nDay 2: sunrise at Borobudur, then the market.\n\nPack: camera, rain jacket, a good book.".into(),
        }
    }
}

impl App for Notes {
    type Message = Msg;

    fn update(&mut self, message: Msg) {
        match message {
            Msg::Title(t) => self.title = t,
            Msg::Body(b) => self.body = b,
        }
    }

    fn view(&self, theme: &Theme) -> Element<Msg> {
        let words = self.body.split_whitespace().count();
        column()
            .p_6()
            .gap_3()
            .child(text("Note").text_size(13.0).font_semibold().text_color(theme.secondary_label()))
            .child(text_input(self.title.clone(), Msg::Title).w_full().key("title").placeholder("Title").label("Title"))
            .child(text_area(self.body.clone(), Msg::Body).w_full().grow().key("body").placeholder("Write something").label("Note"))
            .child(text(format!("{words} words, {} characters", self.body.chars().count())).text_size(12.0).text_color(theme.secondary_label()))
    }
}
