//! The smallest Embun app: a number and two glass buttons.
//!
//! cargo run -p embun --example counter

use embun::*;

struct Counter {
    count: i32,
}

#[derive(Clone)]
enum Message {
    Increment,
    Decrement,
}

impl App for Counter {
    type Message = Message;

    fn update(&mut self, message: Message) {
        match message {
            Message::Increment => self.count += 1,
            Message::Decrement => self.count -= 1,
        }
    }

    fn view(&self, theme: &Theme) -> Element<Message> {
        column()
            .items_center()
            .justify_center()
            .gap_6()
            .bg_gradient(theme.pick(Color::hex(0xffd6a5), Color::hex(0x2b2d42)), theme.pick(Color::hex(0xbde0fe), Color::hex(0x14213d)))
            .child(text(self.count.to_string()).text_size(64.0).font_bold())
            .child(
                row()
                    .gap_3()
                    .child(button("Decrease").size(ControlSize::Large).on_press(Message::Decrement))
                    .child(button("Increase").size(ControlSize::Large).prominent().on_press(Message::Increment)),
            )
    }
}

fn main() -> Result<(), RunError> {
    run(Counter { count: 0 }, Settings::new("Counter").size(420.0, 300.0))
}
