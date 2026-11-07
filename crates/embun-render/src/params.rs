/// Which parts of the glass effect are drawn.
///
/// The default is what macOS 27 draws for resting glass controls: frost,
/// lensing, color fringe and merging. The outline and the edge highlight are
/// part of the measured look and always drawn. Rim light and the drop shadow are off,
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

/// How the glass recolors the backdrop, per pixel and channel, on
/// gamma-encoded values from 0 to 1:
///
/// `offset + gain * L + curve * L² + chroma * (c - L) + sharp_gain * Ls + sharp_chroma * (s - Ls)`
///
/// where `c` is the frosted (blurred) backdrop with luminance `L`, and `s` is
/// the unblurred backdrop with luminance `Ls`. Focused macOS glass mixes in the
/// sharp backdrop, most at the clear setting; unfocused glass does not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub offset: f32,
    pub gain: f32,
    pub curve: f32,
    /// Scales color away from gray; above `gain` means more saturated.
    pub chroma: f32,
    pub sharp_gain: f32,
    pub sharp_chroma: f32,
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
    /// Width of the band at the rim where the backdrop bends, in logical pixels.
    pub bevel: f32,
    /// Strength of the rim light, 0 to 1.
    pub rim: f32,
    /// Strength of the color fringe at the rim, 0 or 1.
    pub fringe: f32,
    /// Strength of the drop shadow, 0 or 1.
    pub shadow: f32,
    /// Smooth-minimum radius in logical pixels; 0 turns merging off.
    pub merge_k: f32,
    /// How much the one-pixel outline just outside the shape darkens the
    /// backdrop beside its left and right sides, for light and dark glass.
    pub outline: [f32; 2],
    /// The same above and below the shape. Corners blend between the two.
    pub outline_ends: [f32; 2],
    /// Brightening along the inside of the top and bottom edges, for light and
    /// dark glass, at its peak (gamma-encoded, 0 to 1). It fades over about
    /// two device pixels.
    pub highlight: [f32; 2],
}

/// Merge radius in logical pixels. Fitted to the neck macOS draws between two
/// 56 pt circles 6 pt apart in a `GlassEffectContainer` with spacing 22.
const MERGE_K: f32 = 19.0;

/// The macOS 27 default position of the glass slider.
pub const DEFAULT_LEVEL: f32 = 0.5;

/// Columns of the measured tables: blur sigma (pt); offset, gain, curve,
/// chroma, sharp gain, sharp chroma for light glass; the same six for dark
/// glass; outline at the sides (light, dark); outline at the ends (light,
/// dark); highlight (light, dark); lensing; bevel (pt).
const COLUMNS: usize = 21;

/// Focused windows, measured from macOS 27.0.1 captures (tools/capture) at
/// the slider's clear, middle and tinted positions, which map to levels 0,
/// 0.5 and 1. The blur stays at 16 pt; the slider changes how much of the
/// sharp backdrop shows through.
const FOCUSED: [[f32; COLUMNS]; 3] = [
    [
        16.0, 0.412, 0.241, -0.051, 0.465, 0.392, 0.621, 0.139, 0.358, -0.207, 0.323, 0.453, 0.727, 0.273, 0.714,
        0.084, 0.0, 0.053, 0.204, 1.0, 14.0,
    ],
    [
        16.0, 0.520, 0.387, -0.068, 0.660, 0.153, 0.245, 0.133, 0.696, -0.281, 0.750, 0.190, 0.305, 0.273, 0.714,
        0.084, 0.0, 0.053, 0.204, 0.8, 14.0,
    ],
    [
        16.0, 0.701, 0.296, -0.010, 0.555, 0.009, 0.016, 0.125, 0.496, -0.203, 0.524, 0.008, 0.015, 0.273, 0.714,
        0.084, 0.0, 0.053, 0.204, 0.6, 14.0,
    ],
];

/// Unfocused windows, measured the same way from captures of a window
/// without focus. Frosted only, no sharp backdrop and no highlight; the
/// outline at the ends is two thirds of the sides.
const UNFOCUSED: [[f32; COLUMNS]; 3] = [
    [
        6.0, 0.405, 0.598, -0.078, 1.131, 0.0, 0.0, 0.088, 0.939, -0.313, 0.984, 0.0, 0.0, 0.264, 0.536, 0.174,
        0.354, 0.0, 0.0, 1.0, 18.0,
    ],
    [
        12.0, 0.527, 0.462, -0.047, 0.914, 0.0, 0.0, 0.087, 0.950, -0.329, 1.010, 0.0, 0.0, 0.240, 0.536, 0.158,
        0.354, 0.0, 0.0, 0.8, 18.0,
    ],
    [
        16.0, 0.698, 0.281, -0.015, 0.583, 0.0, 0.0, 0.103, 0.521, -0.208, 0.539, 0.0, 0.0, 0.202, 0.536, 0.133,
        0.354, 0.0, 0.0, 0.6, 18.0,
    ],
];

impl GlassParams {
    /// Maps the glass slider (0 = clear, 1 = tinted) to shader inputs for a
    /// focused window, the look of the app in use.
    ///
    /// Frost, body, outline and highlight follow the measured macOS values,
    /// interpolated between the three captured slider positions.
    /// `reduce_transparency` overrides the level with a nearly opaque surface
    /// (not yet measured).
    pub fn from_level(level: f32, layers: GlassLayers, reduce_transparency: bool) -> Self {
        Self::from_table(&FOCUSED, level, layers, reduce_transparency)
    }

    /// The same for a window without focus, where macOS glass is frosted
    /// more and shows no sharp backdrop.
    pub fn unfocused(level: f32, layers: GlassLayers, reduce_transparency: bool) -> Self {
        Self::from_table(&UNFOCUSED, level, layers, reduce_transparency)
    }

    fn from_table(table: &[[f32; COLUMNS]; 3], level: f32, layers: GlassLayers, reduce: bool) -> Self {
        let l = if level.is_finite() { level.clamp(0.0, 1.0) } else { DEFAULT_LEVEL };
        let m: [f32; COLUMNS] = {
            let (a, b, t) = if l <= 0.5 { (0, 1, l * 2.0) } else { (1, 2, l * 2.0 - 1.0) };
            std::array::from_fn(|i| lerp(table[a][i], table[b][i], t))
        };
        let (light, dark) = if reduce {
            let opaque = |offset| Body { offset, gain: 0.08, curve: 0.0, chroma: 0.08, sharp_gain: 0.0, sharp_chroma: 0.0 };
            (opaque(0.9), opaque(0.11))
        } else {
            let body = |i: usize| Body {
                offset: m[i],
                gain: m[i + 1],
                curve: m[i + 2],
                chroma: m[i + 3],
                sharp_gain: m[i + 4],
                sharp_chroma: m[i + 5],
            };
            (body(1), body(7))
        };
        Self {
            blur_sigma: match (layers.frost, reduce) {
                (false, _) => 0.0,
                (true, true) => 20.0,
                (true, false) => m[0],
            },
            light,
            dark,
            lensing: if layers.lensing && !reduce { m[19] } else { 0.0 },
            bevel: m[20],
            rim: if layers.rim { 1.0 } else { 0.0 },
            fringe: if layers.fringe && !reduce { 1.0 } else { 0.0 },
            shadow: if layers.shadow { 1.0 } else { 0.0 },
            merge_k: if layers.merge { MERGE_K } else { 0.0 },
            outline: [m[13], m[14]],
            outline_ends: [m[15], m[16]],
            highlight: [m[17], m[18]],
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
        let row_body = |row: &[f32; COLUMNS], i: usize| Body {
            offset: row[i],
            gain: row[i + 1],
            curve: row[i + 2],
            chroma: row[i + 3],
            sharp_gain: row[i + 4],
            sharp_chroma: row[i + 5],
        };
        type Make = fn(f32, GlassLayers, bool) -> GlassParams;
        for (table, make) in [(&FOCUSED, GlassParams::from_level as Make), (&UNFOCUSED, GlassParams::unfocused as Make)] {
            for (level, row) in [0.0, 0.5, 1.0].into_iter().zip(table) {
                let p = make(level, GlassLayers::default(), false);
                assert_eq!(p.blur_sigma, row[0]);
                assert_eq!(p.light, row_body(row, 1));
                assert_eq!(p.dark, row_body(row, 7));
                assert_eq!(p.outline, [row[13], row[14]]);
                assert_eq!(p.outline_ends, [row[15], row[16]]);
                assert_eq!(p.highlight, [row[17], row[18]]);
                assert_eq!((p.lensing, p.bevel), (row[19], row[20]));
            }
        }
    }

    #[test]
    fn levels_between_captures_interpolate() {
        let p = GlassParams::unfocused(0.25, GlassLayers::default(), false);
        assert!((p.blur_sigma - 9.0).abs() < 1e-5, "blur was {}", p.blur_sigma);
        assert!((p.light.offset - (0.405 + 0.527) / 2.0).abs() < 1e-5);
        let p = GlassParams::from_level(0.75, GlassLayers::default(), false);
        assert!((p.light.sharp_gain - (0.153 + 0.009) / 2.0).abs() < 1e-5, "sharp gain was {}", p.light.sharp_gain);
    }

    #[test]
    fn focused_glass_shows_more_sharp_backdrop_at_clear_than_tinted() {
        let at = |level| GlassParams::from_level(level, GlassLayers::default(), false);
        assert!(at(0.0).light.sharp_gain > at(0.5).light.sharp_gain);
        assert!(at(0.5).light.sharp_gain > at(1.0).light.sharp_gain);
        assert_eq!(GlassParams::unfocused(0.0, GlassLayers::default(), false).light.sharp_gain, 0.0);
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
            assert_eq!(p.light.sharp_gain, 0.0);
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
