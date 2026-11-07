//! Glass rendering for wgpu.
//!
//! `embun-render` draws glass shapes over a backdrop texture: the backdrop is
//! frosted, bent near each shape's rim, tinted, and lit along the edge. Shapes
//! that come close merge like water drops.
//!
//! The renderer is a plain library. It draws into any texture view you give it,
//! so it can sit inside a winit app, an iced `shader` widget, an egui paint
//! callback, or a Bevy render node.

pub mod offscreen;
mod params;
mod renderer;

pub use params::{Body, DEFAULT_LEVEL, GlassLayers, GlassParams};
pub use renderer::{Backdrop, Frame, GlassRenderer, GlassShape, MAX_SHAPES};
