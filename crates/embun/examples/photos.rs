//! The sample photo library in a window.
//!
//! cargo run -p embun --example photos

#[path = "support/mod.rs"]
mod support;

fn main() -> Result<(), embun::RunError> {
    embun::run(support::library::Library::new(), embun::Settings::new("Photos").size(1100.0, 700.0))
}
