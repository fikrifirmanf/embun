//! Embun: a Rust UI toolkit with glass controls that match macOS 27.
//!
//! Implement [`App`] for your state, describe the UI in [`App::view`] with
//! [`column()`], [`row()`], [`text()`], [`button()`] and friends, and start it
//! with [`run()`]. [`Ui`] and [`snapshot()`] run the same app without a window,
//! for tests.

mod color;
mod element;
mod layout;
mod render;
mod run;
mod scene;
mod text;
mod theme;
mod ui;
mod components;
mod table;
mod widgets;

pub use color::Color;
pub use element::{Element, Image, ImageError, Role, column, div, image, row, list, spacer, text, text_area, text_input};
pub use render::{Renderer, SNAPSHOT_FORMAT, snapshot};
pub use run::{RunError, Settings, run};
pub use text::{FONT_FAMILY, TextSystem};
pub use theme::{Appearance, Theme};
pub use ui::{App, Key, Ui};
pub use table::{Column, Table, table};
pub use components::{badge, disclosure, dropdown, divider, progress, progress_indeterminate, spinner, vertical_divider};
pub use widgets::{
    Button, ControlSize, button, checkbox, dialog, dialog_panel, icon_button, radio, segmented, slider, switch, menu, menu_check_item, menu_item, menu_separator, popover, sidebar, sidebar_heading, sidebar_item, toolbar_group,
    toolbar_item,
};

pub use embun_motion as motion;
pub use embun_render as render_glass;

/// Compiles the README example as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
