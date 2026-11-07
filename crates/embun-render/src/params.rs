/// Which parts of the glass effect are drawn.
///
/// The default is what macOS 27 draws for resting glass controls: frost,
/// lensing, color fringe and merging. Rim light and the drop shadow are off,
/// because macOS draws neither on resting controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlassLayers {
    pub frost: bool,
    pub lensing: bool,
    pub rim: bool,
    pub fringe: bool,
    pub shadow: bool,
    pub merge: bool,
}

impl Default for GlassLayers {
    fn default() -> Self {
        Self { frost: true, lensing: true, rim: false, fringe: true, shadow: false, merge: true }
    }
}

/// How the glass recolors the frosted backdrop, per pixel and channel, on
/// gamma-encoded values from 0 to 1, where `luma` is the pixel's luminance:
/// `offset + gain * luma + curve * luma² + chroma * (color - luma)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub offset: f32,
    pub gain: f32,
    pub curve: f32,
    /// Scales color away from gray; above `gain` means more saturated.
    pub chroma: f32,
}

/// Shader inputs derived from a single glass level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlassParams {
    /// Frost blur, as a Gaussian standard deviation in logical pixels.
    pub blur_sigma: f32,
    /// Body for light glass.
    pub light: Body,
    /// Body for dark glass.
    pub dark: Body,
    /// Strength of the rim lensing, 0 to 1.
    pub lensing: f32,
    /// Strength of the rim light, 0 to 1.
    pub rim: f32,
    /// Strength of the color fringe at the rim, 0 or 1.
    pub fringe: f32,
    /// Strength of the drop shadow, 0 or 1.
    pub shadow: f32,
    /// Smooth-minimum radius in logical pixels; 0 turns merging off.
    pub merge_k: f32,
    /// How much the one-pixel outline just outside the shape darkens the
    /// backdrop on its left and right sides, for light and dark glass. Top and
    /// bottom get two thirds of this, as in macOS.
    pub outline: [f32; 2],
}

/// Merge radius in logical pixels. Fitted to the neck macOS draws between two
/// 56 pt circles 6 pt apart in a `GlassEffectContainer` with spacing 22.
const MERGE_K: f32 = 19.0;

/// The macOS 27 default position of the glass slider.
pub const DEFAULT_LEVEL: f32 = 0.5;

/// Measured from macOS 27.0.1 captures (tools/capture) at the slider's clear,
/// middle and tinted positions, which map to levels 0, 0.5 and 1.
/// Columns: blur sigma (pt), then offset, gain, curve, chroma for light glass,
/// then the same four for dark glass, then outline darkening for light and dark.
const MEASURED: [[f32; 11]; 3] = [
    [6.0, 0.405, 0.598, -0.078, 1.131, 0.088, 0.939, -0.313, 0.984, 0.264, 0.536],
    [12.0, 0.527, 0.462, -0.047, 0.914, 0.087, 0.950, -0.329, 1.010, 0.240, 0.536],
    [16.0, 0.698, 0.281, -0.015, 0.583, 0.103, 0.521, -0.208, 0.539, 0.202, 0.536],
];

impl GlassParams {
    /// Maps the glass slider (0 = clear, 1 = tinted) to shader inputs.
    ///
    /// Frost and body follow the measured macOS values, interpolated between
    /// the three captured slider positions. `reduce_transparency` overrides
    /// the level with a nearly opaque surface (not yet measured).
    pub fn from_level(level: f32, layers: GlassLayers, reduce_transparency: bool) -> Self {
        let l = if level.is_finite() { level.clamp(0.0, 1.0) } else { DEFAULT_LEVEL };
        let m: [f32; 11] = {
            let (a, b, t) = if l <= 0.5 { (0, 1, l * 2.0) } else { (1, 2, l * 2.0 - 1.0) };
            std::array::from_fn(|i| lerp(MEASURED[a][i], MEASURED[b][i], t))
        };
        let reduce = reduce_transparency;
        let (light, dark) = if reduce {
            let opaque = |offset| Body { offset, gain: 0.08, curve: 0.0, chroma: 0.08 };
            (opaque(0.9), opaque(0.11))
        } else {
            let body = |i: usize| Body { offset: m[i], gain: m[i + 1], curve: m[i + 2], chroma: m[i + 3] };
            (body(1), body(5))
        };
        Self {
            blur_sigma: match (layers.frost, reduce) {
                (false, _) => 0.0,
                (true, true) => 20.0,
                (true, false) => m[0],
            },
            light,
            dark,
            lensing: if layers.lensing && !reduce { lerp(1.0, 0.6, l) } else { 0.0 },
            rim: if layers.rim { 1.0 } else { 0.0 },
            fringe: if layers.fringe && !reduce { 1.0 } else { 0.0 },
            shadow: if layers.shadow { 1.0 } else { 0.0 },
            merge_k: if layers.merge { MERGE_K } else { 0.0 },
            outline: [m[9], m[10]],
        }
    }
}

impl Default for GlassParams {
    fn default() -> Self {
        Self::from_level(DEFAULT_LEVEL, GlassLayers::default(), false)
    }
}

/// Exact at both ends, so the captured slider positions return measured values.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_slider_positions_return_the_measured_values() {
        for (level, row) in [0.0, 0.5, 1.0].into_iter().zip(MEASURED) {
            let p = GlassParams::from_level(level, GlassLayers::default(), false);
            assert_eq!(p.blur_sigma, row[0]);
            assert_eq!(p.light, Body { offset: row[1], gain: row[2], curve: row[3], chroma: row[4] });
            assert_eq!(p.dark, Body { offset: row[5], gain: row[6], curve: row[7], chroma: row[8] });
            assert_eq!(p.outline, [row[9], row[10]]);
        }
    }

    #[test]
    fn levels_between_captures_interpolate() {
        let p = GlassParams::from_level(0.25, GlassLayers::default(), false);
        assert!((p.blur_sigma - 9.0).abs() < 1e-5, "blur was {}", p.blur_sigma);
        assert!((p.light.offset - (0.405 + 0.527) / 2.0).abs() < 1e-5);
    }

    #[test]
    fn level_is_clamped_and_nan_falls_back_to_default() {
        let layers = GlassLayers::default();
        assert_eq!(GlassParams::from_level(-3.0, layers, false), GlassParams::from_level(0.0, layers, false));
        assert_eq!(GlassParams::from_level(7.0, layers, false), GlassParams::from_level(1.0, layers, false));
        assert_eq!(GlassParams::from_level(f32::NAN, layers, false), GlassParams::default());
    }

    #[test]
    fn reduce_transparency_wins_over_level_and_layers() {
        for level in [0.0, 0.5, 1.0] {
            let p = GlassParams::from_level(level, GlassLayers::default(), true);
            assert_eq!(p.light.offset, 0.9);
            assert_eq!(p.dark.gain, 0.08);
            assert_eq!(p.lensing, 0.0);
            assert_eq!(p.fringe, 0.0);
        }
    }

    #[test]
    fn disabled_layers_are_zero() {
        let off = GlassLayers { frost: false, lensing: false, rim: false, fringe: false, shadow: false, merge: false };
        let p = GlassParams::from_level(0.5, off, false);
        assert_eq!((p.blur_sigma, p.lensing, p.rim, p.fringe, p.shadow, p.merge_k), (0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
    }
}
