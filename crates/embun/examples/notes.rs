//! A title and a multi-line note.
//!
//! cargo run -p embun --example notes

#[path = "support/mod.rs"]
mod support;

fn main() -> Result<(), embun::RunError> {
    embun::run(support::notes::Notes::new(), embun::Settings::new("Notes").size(520.0, 360.0))
}
