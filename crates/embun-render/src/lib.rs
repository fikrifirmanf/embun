//! Glass rendering for wgpu.
//!
//! `embun-render` draws glass shapes over a backdrop texture: the backdrop is
//! frosted, bent near each shape's rim and tinted, and each shape gets a
//! one-pixel outline. Shapes that come close merge like water drops. A rim
//! light and a drop shadow are available as opt-in layers.
//!
//! The renderer is a plain library. It draws into any texture view you give it,
//! so it can sit inside a winit app, an iced `shader` widget, an egui paint
//! callback, or a Bevy render node.

pub mod offscreen;
mod params;
mod renderer;

pub use params::{Body, DEFAULT_LEVEL, GlassLayers, GlassParams};
pub use renderer::{Backdrop, Frame, GlassRenderer, GlassShape, MAX_SHAPES};

/// Compiles the README example as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
