/// A color with sRGB-encoded channels from 0 to 1 and straight (not
/// premultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const TRANSPARENT: Color = Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };

    /// An opaque color from 8-bit sRGB channels.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }
    }

    /// A color from 8-bit sRGB channels and an alpha from 0 to 1.
    pub const fn rgba(r: u8, g: u8, b: u8, a: f32) -> Self {
        Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a }
    }

    /// An opaque color from `0xRRGGBB`.
    pub const fn hex(rgb: u32) -> Self {
        Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
    }

    pub const fn with_alpha(self, a: f32) -> Self {
        Color { a, ..self }
    }

    /// Linear-light channels, premultiplied by alpha, for blending on the GPU.
    pub(crate) fn linear_premultiplied(self) -> [f32; 4] {
        let a = self.a.clamp(0.0, 1.0);
        [to_linear(self.r) * a, to_linear(self.g) * a, to_linear(self.b) * a, a]
    }
}

/// The sRGB transfer function, from encoded to linear light.
pub(crate) fn to_linear(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_rgb_agree() {
        assert_eq!(Color::hex(0x0085fa), Color::rgb(0, 133, 250));
    }

    #[test]
    fn linear_premultiplied_matches_the_srgb_curve() {
        let [r, g, b, a] = Color::rgba(255, 128, 0, 0.5).linear_premultiplied();
        assert_eq!((r, b, a), (0.5, 0.0, 0.5));
        // sRGB 128 is about 21.6% linear light.
        assert!((g / 0.5 - 0.2158).abs() < 1e-3, "g was {g}");
    }
}
