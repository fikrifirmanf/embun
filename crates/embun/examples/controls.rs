//! Every control in one window.
//!
//! cargo run -p embun --example controls

#[path = "support/mod.rs"]
mod support;

fn main() -> Result<(), embun::RunError> {
    embun::run(support::controls::Controls::new(), embun::Settings::new("Controls").size(640.0, 460.0))
}
