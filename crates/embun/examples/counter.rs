//! The smallest Embun app: a number and two glass buttons.
//!
//! cargo run -p embun --example counter

#[path = "support/mod.rs"]
mod support;

use embun::{RunError, Settings, run};
use support::counter::Counter;

fn main() -> Result<(), RunError> {
    run(Counter { count: 0 }, Settings::new("Counter").size(420.0, 300.0))
}
