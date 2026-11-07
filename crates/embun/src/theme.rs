use embun_render::{GlassLayers, GlassParams};

use crate::Color;

/// Light or dark mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Appearance {
    #[default]
    Light,
    Dark,
}

/// The system state the glass and the colors follow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub appearance: Appearance,
    /// The glass slider: 0 is clear, 0.5 the macOS default, 1 tinted.
    pub glass_level: f32,
    /// Whether the window has focus. Glass in a window without focus is
    /// frosted more, and prominent buttons lose their color.
    pub focused: bool,
    /// Draws glass as a nearly opaque surface.
    pub reduce_transparency: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self { appearance: Appearance::Light, glass_level: 0.5, focused: true, reduce_transparency: false }
    }
}

impl Theme {
    pub fn is_dark(&self) -> bool {
        self.appearance == Appearance::Dark
    }

    /// Picks the light or the dark value.
    pub fn pick<T>(&self, light: T, dark: T) -> T {
        if self.is_dark() { dark } else { light }
    }

    /// The window background.
    pub fn background(&self) -> Color {
        self.pick(Color::rgb(242, 242, 247), Color::rgb(28, 28, 30))
    }

    /// A raised surface on the background, such as a card.
    pub fn surface(&self) -> Color {
        self.pick(Color::WHITE, Color::rgb(44, 44, 46))
    }

    /// Primary text.
    pub fn label(&self) -> Color {
        self.pick(Color::rgba(0, 0, 0, 0.88), Color::rgba(255, 255, 255, 0.92))
    }

    /// Secondary text, readable on the background and on glass.
    pub fn secondary_label(&self) -> Color {
        self.pick(Color::rgba(60, 60, 67, 0.75), Color::rgba(235, 235, 245, 0.7))
    }

    /// Hairlines between rows and around cards.
    pub fn separator(&self) -> Color {
        self.pick(Color::rgba(60, 60, 67, 0.18), Color::rgba(84, 84, 88, 0.6))
    }

    /// The accent color. System blue as macOS 27.0.1 draws it on prominent
    /// glass buttons; lighter in dark mode.
    pub fn accent(&self) -> Color {
        self.pick(Color::rgb(0, 133, 250), Color::rgb(0, 145, 255))
    }

    /// Text on an accent-colored surface.
    pub fn on_accent(&self) -> Color {
        Color::WHITE
    }

    /// The glass parameters for this theme.
    pub fn glass(&self) -> GlassParams {
        if self.focused {
            GlassParams::from_level(self.glass_level, GlassLayers::default(), self.reduce_transparency)
        } else {
            GlassParams::unfocused(self.glass_level, GlassLayers::default(), self.reduce_transparency)
        }
    }

    /// 0 for light glass, 1 for dark.
    pub(crate) fn glass_appearance(&self) -> f32 {
        self.pick(0.0, 1.0)
    }
}
